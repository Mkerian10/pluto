use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;

use crate::diagnostics::CompileError;
use crate::parser::ast::{
    Block, ClassDecl, Expr, Function, Param, Program, Stmt, TypeExpr,
};
use crate::span::{Span, Spanned};
use crate::typeck::env::TypeEnv;

/// Generate marshal/unmarshal functions for types crossing stage boundaries.
///
/// Two-phase generation:
/// - Phase A (before typeck): Non-generic classes and enums
/// - Phase B (after monomorphize): Generic instantiations (e.g., Box__int)
///
/// For each serializable type T that crosses a stage boundary, generates:
/// - `__marshal_T(value: T, enc: Encoder)` — encodes value using encoder methods
/// - `__unmarshal_T(dec: Decoder) T` — decodes value using decoder methods (fallible)
///
/// Arrays, maps, sets, nullable: marshaling is inlined at use sites, not separate functions.

/// Phase A: Generate marshalers for non-generic types before typeck.
///
/// Walks all stage pub method signatures, collects all non-generic classes and enums
/// that cross stage boundaries, and generates marshal/unmarshal functions.
/// These are injected into the Program AST before typeck.
pub fn generate_marshalers_phase_a(program: &mut Program) -> Result<(), CompileError> {
    if program.stages.is_empty() {
        return Ok(()); // No stages, no marshaling needed
    }

    // Skip marshaler generation if wire module isn't available
    // Check for wire.wire_value_encoder function (proves wire module was imported and flattened)
    let has_wire = program.functions.iter().any(|f| {
        f.node.name.node == "wire.wire_value_encoder"
    });
    if !has_wire {
        return Ok(());
    }

    // Collect all types that need marshalers
    let types_to_marshal = collect_types_from_stage_methods(program)?;

    // Generate marshal and unmarshal functions for each type
    let mut generated_functions = Vec::new();
    let mut instantiated_classes = Vec::new();
    let mut instantiated_enums = Vec::new();

    for type_name in &types_to_marshal {
        // Handle mangled generic instantiations (Box$$int, etc.)
        if type_name.contains("$$") {
            // Parse mangled name: Box$$int → Box, [int]
            let parts: Vec<&str> = type_name.split("$$").collect();
            if parts.len() != 2 {
                continue; // Skip complex generics for now (e.g., Pair$$int$string)
            }
            let base_name = parts[0];
            let type_arg_str = parts[1];

            // Find the generic template class
            if let Some(generic_class) = program.classes.iter().find(|c| &c.node.name.node == base_name && !c.node.type_params.is_empty()) {
                // Create instantiated class declaration
                let instantiated_class = instantiate_generic_class(&generic_class.node, type_name, type_arg_str)?;
                let marshal_fn = generate_marshal_class(&instantiated_class)?;
                let unmarshal_fn = generate_unmarshal_class(&instantiated_class)?;
                generated_functions.push(marshal_fn);
                generated_functions.push(unmarshal_fn);
                // Add instantiated class to program so type checking knows about Box$$int
                instantiated_classes.push(Spanned {
                    node: instantiated_class,
                    span: generic_class.span,
                });
                continue;
            }

            // Find the generic template enum
            if let Some(generic_enum) = program.enums.iter().find(|e| &e.node.name.node == base_name && !e.node.type_params.is_empty()) {
                // Create instantiated enum declaration
                let instantiated_enum = instantiate_generic_enum(&generic_enum.node, type_name, type_arg_str)?;
                let marshal_fn = generate_marshal_enum(&instantiated_enum)?;
                let unmarshal_fn = generate_unmarshal_enum(&instantiated_enum)?;
                generated_functions.push(marshal_fn);
                generated_functions.push(unmarshal_fn);
                // Add instantiated enum to program so type checking knows about Result__int
                instantiated_enums.push(Spanned {
                    node: instantiated_enum,
                    span: generic_enum.span,
                });
                continue;
            }
        }

        // Handle non-generic classes and enums
        if let Some(class_decl) = program.classes.iter().find(|c| &c.node.name.node == type_name) {
            // Skip generic classes (they'll be handled above)
            if !class_decl.node.type_params.is_empty() {
                continue;
            }

            let marshal_fn = generate_marshal_class(&class_decl.node)?;
            let unmarshal_fn = generate_unmarshal_class(&class_decl.node)?;

            // // Debug: print generated functions
            // if type_name == "Item" || type_name == "Data" {
            //     eprintln!("\n=== Generated __marshal_{} ===", type_name);
            //     eprintln!("{}", crate::pretty::pretty_print_function(&marshal_fn.node));
            //     eprintln!("\n=== Generated __unmarshal_{} ===", type_name);
            //     eprintln!("{}", crate::pretty::pretty_print_function(&unmarshal_fn.node));
            // }

            generated_functions.push(marshal_fn);
            generated_functions.push(unmarshal_fn);
        } else if let Some(enum_decl) = program.enums.iter().find(|e| &e.node.name.node == type_name) {
            // Skip generic enums (they'll be handled above)
            if !enum_decl.node.type_params.is_empty() {
                continue;
            }

            generated_functions.push(generate_marshal_enum(&enum_decl.node)?);
            generated_functions.push(generate_unmarshal_enum(&enum_decl.node)?);
        }
    }

    // Inject generated functions and instantiated types into the program
    program.functions.extend(generated_functions);
    program.classes.extend(instantiated_classes);
    program.enums.extend(instantiated_enums);

    Ok(())
}

/// Phase B: Generate marshalers for generic instantiations after monomorphize.
///
/// After monomorphization creates concrete types (Box__int, Pair__string_float),
/// generates marshal/unmarshal functions for each instantiation.
/// These skip typeck (their types are known to be correct).
pub fn generate_marshalers_phase_b(
    program: &mut Program,
    _env: &TypeEnv,
) -> Result<(), CompileError> {
    if program.stages.is_empty() {
        return Ok(());
    }

    // Skip marshaler generation if wire module isn't available
    // Check for wire.wire_value_encoder function (proves wire module was imported and flattened)
    let has_wire = program.functions.iter().any(|f| {
        f.node.name.node == "wire.wire_value_encoder"
    });
    if !has_wire {
        return Ok(());
    }

    // Collect types from stage methods (now includes monomorphized names like Box$$int)
    let types_to_marshal = collect_types_from_stage_methods(program)?;

    eprintln!("Phase B: Collected types: {:?}", types_to_marshal);

    let mut generated_functions = Vec::new();

    for type_name in &types_to_marshal {
        // Check if it's a monomorphized type (contains $$) that doesn't have a marshaler yet
        if type_name.contains("$$") {
            // Convert $$ to __ in function name for valid identifier syntax
            let marshal_fn_name = format!("__marshal_{}", type_name.replace("$$", "__"));
            let has_marshal = program.functions.iter()
                .any(|f| f.node.name.node == marshal_fn_name);

            if has_marshal {
                continue; // Already generated
            }

            // Look for the class in program.classes (monomorphize adds concrete classes)
            if let Some(class_decl) = program.classes.iter().find(|c| &c.node.name.node == type_name) {
                generated_functions.push(generate_marshal_class(&class_decl.node)?);
                generated_functions.push(generate_unmarshal_class(&class_decl.node)?);
                continue;
            }

            // Look for the enum in program.enums (monomorphize adds concrete enums)
            if let Some(enum_decl) = program.enums.iter().find(|e| &e.node.name.node == type_name) {
                generated_functions.push(generate_marshal_enum(&enum_decl.node)?);
                generated_functions.push(generate_unmarshal_enum(&enum_decl.node)?);
                continue;
            }
        }
    }

    program.functions.extend(generated_functions);

    Ok(())
}

/// Collects all types that cross stage boundaries (stage pub method parameters and returns).
/// Returns a set of type names (classes and enums) that need marshalers.
/// Uses fixed-point expansion to recursively collect nested types from class fields and enum variants.
fn collect_types_from_stage_methods(program: &Program) -> Result<HashSet<String>, CompileError> {
    let mut types = HashSet::new();

    // Initial collection from stage method signatures
    for stage in &program.stages {
        for method in &stage.node.methods {
            if !method.node.is_pub {
                continue; // Only pub methods cross boundaries
            }

            // Collect from parameters
            for param in &method.node.params {
                if param.name.node == "self" {
                    continue;
                }
                collect_types_from_type_expr(&param.ty.node, &mut types);
            }

            // Collect from return type
            if let Some(ref ret_type) = method.node.return_type {
                collect_types_from_type_expr(&ret_type.node, &mut types);
            }
        }
    }

    // Fixed-point expansion: recursively collect nested types from class fields and enum variants
    loop {
        let mut new_types = HashSet::new();

        for type_name in &types {
            // Expand class fields
            if let Some(class_decl) = program.classes.iter().find(|c| &c.node.name.node == type_name) {
                for field in &class_decl.node.fields {
                    if field.is_injected {
                        continue; // Skip DI fields
                    }
                    collect_types_from_type_expr(&field.ty.node, &mut new_types);
                }
            }

            // Expand enum variant fields
            if let Some(enum_decl) = program.enums.iter().find(|e| &e.node.name.node == type_name) {
                for variant in &enum_decl.node.variants {
                    for field in &variant.fields {
                        collect_types_from_type_expr(&field.ty.node, &mut new_types);
                    }
                }
            }
        }

        // Check if we found any new types (fixed point reached when no new types added)
        let added = new_types.difference(&types).count();
        if added == 0 {
            break;
        }

        types.extend(new_types);
    }

    Ok(types)
}

/// Instantiates a generic class template with concrete type arguments.
/// Example: Box<T> with type_arg="int" → Box__int with all T replaced by int
fn instantiate_generic_class(template: &ClassDecl, mangled_name: &str, type_arg_str: &str) -> Result<ClassDecl, CompileError> {
    // For now, only support single type parameter
    if template.type_params.len() != 1 {
        return Err(CompileError::codegen(
            format!("Marshal generation for multi-param generics not yet supported: {}", template.name.node)
        ));
    }

    let type_param = &template.type_params[0].node;
    let concrete_type = TypeExpr::Named(type_arg_str.to_string());

    // Substitute type parameter in all field types
    let mut instantiated_fields = Vec::new();
    for field in &template.fields {
        let instantiated_ty = substitute_type_in_type_expr(&field.ty.node, type_param, &concrete_type);
        instantiated_fields.push(crate::parser::ast::Field {
            id: field.id,
            name: field.name.clone(),
            ty: Spanned { node: instantiated_ty, span: field.ty.span },
            is_injected: field.is_injected,
            is_ambient: field.is_ambient,
        });
    }

    Ok(ClassDecl {
        id: template.id,
        name: Spanned { node: mangled_name.to_string(), span: template.name.span },
        type_params: vec![], // Instantiated classes have no type params
        type_param_bounds: std::collections::HashMap::new(),
        fields: instantiated_fields,
        methods: template.methods.clone(), // Methods not used in marshal generation
        invariants: template.invariants.clone(),
        impl_traits: template.impl_traits.clone(),
        uses: template.uses.clone(),
        is_pub: template.is_pub,
        lifecycle: template.lifecycle,
    })
}

/// Instantiates a generic enum template with concrete type arguments.
fn instantiate_generic_enum(template: &crate::parser::ast::EnumDecl, mangled_name: &str, type_arg_str: &str) -> Result<crate::parser::ast::EnumDecl, CompileError> {
    use crate::parser::ast::EnumVariant;

    if template.type_params.len() != 1 {
        return Err(CompileError::codegen(
            format!("Marshal generation for multi-param generics not yet supported: {}", template.name.node)
        ));
    }

    let type_param = &template.type_params[0].node;
    let concrete_type = TypeExpr::Named(type_arg_str.to_string());

    // Substitute type parameter in all variant field types
    let mut instantiated_variants = Vec::new();
    for variant in &template.variants {
        let mut instantiated_fields = Vec::new();
        for field in &variant.fields {
            let instantiated_ty = substitute_type_in_type_expr(&field.ty.node, type_param, &concrete_type);
            instantiated_fields.push(crate::parser::ast::Field {
                id: field.id,
                name: field.name.clone(),
                ty: Spanned { node: instantiated_ty, span: field.ty.span },
                is_injected: field.is_injected,
                is_ambient: field.is_ambient,
            });
        }
        instantiated_variants.push(EnumVariant {
            id: variant.id,
            name: variant.name.clone(),
            fields: instantiated_fields,
        });
    }

    Ok(crate::parser::ast::EnumDecl {
        id: template.id,
        name: Spanned { node: mangled_name.to_string(), span: template.name.span },
        type_params: vec![],
        type_param_bounds: std::collections::HashMap::new(),
        variants: instantiated_variants,
        is_pub: template.is_pub,
    })
}

/// Substitutes a type parameter with a concrete type in a TypeExpr.
fn substitute_type_in_type_expr(ty: &TypeExpr, type_param: &str, concrete: &TypeExpr) -> TypeExpr {
    match ty {
        TypeExpr::Named(name) if name == type_param => concrete.clone(),
        TypeExpr::Named(name) => TypeExpr::Named(name.clone()),
        TypeExpr::Array(elem) => {
            TypeExpr::Array(Box::new(Spanned {
                node: substitute_type_in_type_expr(&elem.node, type_param, concrete),
                span: elem.span,
            }))
        }
        TypeExpr::Nullable(inner) => {
            TypeExpr::Nullable(Box::new(Spanned {
                node: substitute_type_in_type_expr(&inner.node, type_param, concrete),
                span: inner.span,
            }))
        }
        TypeExpr::Generic { name, type_args } => {
            let substituted_args: Vec<Spanned<TypeExpr>> = type_args.iter().map(|arg| {
                Spanned {
                    node: substitute_type_in_type_expr(&arg.node, type_param, concrete),
                    span: arg.span,
                }
            }).collect();
            TypeExpr::Generic { name: name.clone(), type_args: substituted_args }
        }
        other => other.clone(),
    }
}

/// Recursively collects type names from a TypeExpr.
/// Adds class and enum names to the set. Recursively descends into arrays, nullable, etc.
fn collect_types_from_type_expr(ty: &TypeExpr, types: &mut HashSet<String>) {
    match ty {
        TypeExpr::Named(name) => {
            // Add if it's not a primitive
            if !matches!(name.as_str(), "int" | "float" | "bool" | "string" | "byte" | "void") {
                types.insert(name.clone());
            }
        }
        TypeExpr::Array(elem) => {
            collect_types_from_type_expr(&elem.node, types);
        }
        TypeExpr::Nullable(inner) => {
            collect_types_from_type_expr(&inner.node, types);
        }
        TypeExpr::Generic { name, type_args } => {
            // Handle built-in generics (Map, Set)
            match name.as_str() {
                "Map" => {
                    for arg in type_args {
                        collect_types_from_type_expr(&arg.node, types);
                    }
                }
                "Set" => {
                    for arg in type_args {
                        collect_types_from_type_expr(&arg.node, types);
                    }
                }
                _ => {
                    // User-defined generic (will be monomorphized to Box__int, etc.)
                    // Collect the mangled name that will exist after monomorphization
                    let mangled = mangle_generic_name(name, type_args);
                    types.insert(mangled);

                    // Also recursively collect type arguments
                    for arg in type_args {
                        collect_types_from_type_expr(&arg.node, types);
                    }
                }
            }
        }
        TypeExpr::Qualified { module, name } => {
            // Module-qualified type (e.g., math.Vector)
            let flattened = format!("{}.{}", module, name);
            types.insert(flattened);
        }
        TypeExpr::Fn { .. } => {
            // Closures are not serializable (caught by validation)
        }
        TypeExpr::Stream(_) => {
            // Streams are not yet supported (caught by validation)
        }
    }
}

/// Generates __marshal_ClassName function for a class.
fn generate_marshal_class(class_decl: &ClassDecl) -> Result<Spanned<Function>, CompileError> {
    let class_name = &class_decl.name.node;
    // Convert $$ to __ in function name for valid identifier syntax
    let fn_name = format!("__marshal_{}", class_name.replace("$$", "__"));

    // Count non-injected fields
    let data_fields: Vec<_> = class_decl.fields.iter()
        .filter(|f| !f.is_injected)
        .collect();

    let num_fields = data_fields.len();

    // Build function body
    let mut stmts = Vec::new();

    // enc.encode_record_start("ClassName", num_fields)
    stmts.push(mk_stmt_expr(mk_call(
        "enc.encode_record_start",
        vec![
            mk_string_lit(class_name),
            mk_int_lit(num_fields as i64),
        ],
    )));

    // For each field: encode_field + encode the value
    for (index, field) in data_fields.iter().enumerate() {
        // enc.encode_field("field_name", index)
        stmts.push(mk_stmt_expr(mk_call(
            "enc.encode_field",
            vec![
                mk_string_lit(&field.name.node),
                mk_int_lit(index as i64),
            ],
        )));

        // Encode the field value
        stmts.extend(mk_encode_value(
            &field.ty.node,
            mk_field_access("value", &field.name.node),
        )?);
    }

    // enc.encode_record_end()
    stmts.push(mk_stmt_expr(mk_method_call("enc", "encode_record_end", vec![])));

    let body = Spanned {
        node: Block { stmts },
        span: Span { start: 0, end: 0, file_id: 0 },
    };

    let function = Function {
        id: Uuid::new_v4(),
        name: Spanned {
            node: fn_name,
            span: Span { start: 0, end: 0, file_id: 0 },
        },
        type_params: vec![],
        type_param_bounds: HashMap::new(),
        params: vec![
            Param {
                id: Uuid::new_v4(),
                name: Spanned {
                    node: "value".to_string(),
                    span: Span { start: 0, end: 0, file_id: 0 },
                },
                ty: Spanned {
                    node: TypeExpr::Named(class_name.clone()),
                    span: Span { start: 0, end: 0, file_id: 0 },
                },
                is_mut: false,
            },
            Param {
                id: Uuid::new_v4(),
                name: Spanned {
                    node: "enc".to_string(),
                    span: Span { start: 0, end: 0, file_id: 0 },
                },
                ty: Spanned {
                    node: TypeExpr::Named("wire.WireValueEncoder".to_string()),
                    span: Span { start: 0, end: 0, file_id: 0 },
                },
                is_mut: false,
            },
        ],
        return_type: None, // void
        contracts: vec![],
        body,
        is_pub: false,
        is_override: false,
        is_generator: false,
    };

    Ok(Spanned {
        node: function,
        span: Span { start: 0, end: 0, file_id: 0 },
    })
}

/// Generates __unmarshal_ClassName function for a class.
fn generate_unmarshal_class(class_decl: &ClassDecl) -> Result<Spanned<Function>, CompileError> {
    let class_name = &class_decl.name.node;
    // Convert $$ to __ in function name for valid identifier syntax
    let fn_name = format!("__unmarshal_{}", class_name.replace("$$", "__"));

    let data_fields: Vec<_> = class_decl.fields.iter()
        .filter(|f| !f.is_injected)
        .collect();

    let num_fields = data_fields.len();

    let mut stmts = Vec::new();

    // dec.decode_record_start("ClassName", num_fields)!
    stmts.push(mk_stmt_expr(mk_propagate(mk_method_call(
        "dec",
        "decode_record_start",
        vec![
            mk_string_lit(class_name),
            mk_int_lit(num_fields as i64),
        ],
    ))));

    // For each field: decode_field + decode the value
    for (index, field) in data_fields.iter().enumerate() {
        // dec.decode_field("field_name", index)!
        stmts.push(mk_stmt_expr(mk_propagate(mk_method_call(
            "dec",
            "decode_field",
            vec![
                mk_string_lit(&field.name.node),
                mk_int_lit(index as i64),
            ],
        ))));

        // let field_name = decode_type(dec)
        stmts.extend(mk_let_decode(&field.name.node, &field.ty.node)?);
    }

    // dec.decode_record_end()
    stmts.push(mk_stmt_expr(mk_method_call("dec", "decode_record_end", vec![])));

    // return ClassName { field1: field1, field2: field2, ... }
    let field_inits: Vec<_> = data_fields.iter()
        .map(|f| (f.name.node.clone(), mk_var(&f.name.node)))
        .collect();
    stmts.push(mk_return(mk_struct_lit(class_name, field_inits)));

    let body = Spanned {
        node: Block { stmts },
        span: Span { start: 0, end: 0, file_id: 0 },
    };

    let function = Function {
        id: Uuid::new_v4(),
        name: Spanned {
            node: fn_name,
            span: Span { start: 0, end: 0, file_id: 0 },
        },
        type_params: vec![],
        type_param_bounds: HashMap::new(),
        params: vec![Param {
            id: Uuid::new_v4(),
            name: Spanned {
                node: "dec".to_string(),
                span: Span { start: 0, end: 0, file_id: 0 },
            },
            ty: Spanned {
                node: TypeExpr::Named("wire.WireValueDecoder".to_string()),
                span: Span { start: 0, end: 0, file_id: 0 },
            },
            is_mut: false,
        }],
        return_type: Some(Spanned {
            node: TypeExpr::Named(class_name.clone()),
            span: Span { start: 0, end: 0, file_id: 0 },
        }),
        contracts: vec![],
        body,
        is_pub: false,
        is_override: false,
        is_generator: false,
    };

    Ok(Spanned {
        node: function,
        span: Span { start: 0, end: 0, file_id: 0 },
    })
}

/// Generates __marshal_EnumName function for an enum.
fn generate_marshal_enum(enum_decl: &crate::parser::ast::EnumDecl) -> Result<Spanned<Function>, CompileError> {
    use crate::parser::ast::MatchArm;

    let enum_name = &enum_decl.name.node;
    // Convert $$ to __ in function name for valid identifier syntax
    let fn_name = format!("__marshal_{}", enum_name.replace("$$", "__"));

    // Build match arms for each variant
    let mut match_arms = Vec::new();

    for (variant_index, variant) in enum_decl.variants.iter().enumerate() {
        let variant_name = &variant.name.node;
        let num_fields = variant.fields.len();

        // Create bindings for variant fields (field_name, None for no rename)
        let bindings: Vec<_> = variant.fields.iter()
            .map(|f| (
                Spanned { node: f.name.node.clone(), span: mk_span() },
                None
            ))
            .collect();

        // Build match arm body
        let mut stmts = Vec::new();

        // enc.encode_variant_start("EnumName", "VariantName", variant_index, num_fields)
        stmts.push(mk_stmt_expr(mk_call(
            "enc.encode_variant_start",
            vec![
                mk_string_lit(enum_name),
                mk_string_lit(variant_name),
                mk_int_lit(variant_index as i64),
                mk_int_lit(num_fields as i64),
            ],
        )));

        // For each field: encode_field + encode value
        for (field_index, field) in variant.fields.iter().enumerate() {
            // enc.encode_field("field_name", field_index)
            stmts.push(mk_stmt_expr(mk_call(
                "enc.encode_field",
                vec![
                    mk_string_lit(&field.name.node),
                    mk_int_lit(field_index as i64),
                ],
            )));

            // Encode the field value using the bound variable
            stmts.extend(mk_encode_value(
                &field.ty.node,
                mk_var(&field.name.node),
            )?);
        }

        // enc.encode_variant_end()
        stmts.push(mk_stmt_expr(mk_method_call("enc", "encode_variant_end", vec![])));

        let arm = MatchArm {
            enum_name: Spanned { node: enum_name.clone(), span: mk_span() },
            variant_name: Spanned { node: variant_name.clone(), span: mk_span() },
            type_args: vec![],
            bindings,
            body: Spanned { node: Block { stmts }, span: mk_span() },
            enum_id: Some(enum_decl.id),
            variant_id: Some(variant.id),
        };

        match_arms.push(arm);
    }

    // Create function body: match value { ... }
    let match_stmt = Stmt::Match {
        expr: Spanned { node: mk_var("value"), span: mk_span() },
        arms: match_arms,
    };

    let body = Spanned {
        node: Block {
            stmts: vec![Spanned { node: match_stmt, span: mk_span() }],
        },
        span: mk_span(),
    };

    let function = Function {
        id: Uuid::new_v4(),
        name: Spanned { node: fn_name, span: mk_span() },
        type_params: vec![],
        type_param_bounds: HashMap::new(),
        params: vec![
            Param {
                id: Uuid::new_v4(),
                name: Spanned { node: "value".to_string(), span: mk_span() },
                ty: Spanned {
                    node: TypeExpr::Named(enum_name.clone()),
                    span: mk_span(),
                },
                is_mut: false,
            },
            Param {
                id: Uuid::new_v4(),
                name: Spanned { node: "enc".to_string(), span: mk_span() },
                ty: Spanned {
                    node: TypeExpr::Named("wire.WireValueEncoder".to_string()),
                    span: mk_span(),
                },
                is_mut: false,
            },
        ],
        return_type: None, // void
        contracts: vec![],
        body,
        is_pub: false,
        is_override: false,
        is_generator: false,
    };

    Ok(Spanned { node: function, span: mk_span() })
}

/// Generates __unmarshal_enum function for an enum.
fn generate_unmarshal_enum(enum_decl: &crate::parser::ast::EnumDecl) -> Result<Spanned<Function>, CompileError> {
    let enum_name = &enum_decl.name.node;
    // Convert $$ to __ in function name for valid identifier syntax
    let fn_name = format!("__unmarshal_{}", enum_name.replace("$$", "__"));

    let mut stmts = Vec::new();

    // Build variant names array: let names: [string] = ["Variant1", "Variant2", ...]
    let variant_names: Vec<Expr> = enum_decl.variants.iter()
        .map(|v| mk_string_lit(&v.name.node))
        .collect();

    let names_array = Expr::ArrayLit {
        elements: variant_names.into_iter()
            .map(|e| Spanned { node: e, span: mk_span() })
            .collect(),
    };

    stmts.push(Spanned {
        node: Stmt::Let {
            name: Spanned { node: "names".to_string(), span: mk_span() },
            ty: Some(Spanned {
                node: TypeExpr::Array(Box::new(Spanned {
                    node: TypeExpr::Named("string".to_string()),
                    span: mk_span(),
                })),
                span: mk_span(),
            }),
            value: Spanned { node: names_array, span: mk_span() },
            is_mut: false,
        },
        span: mk_span(),
    });

    // let idx = dec.decode_variant("EnumName", names)!
    stmts.push(Spanned {
        node: Stmt::Let {
            name: Spanned { node: "idx".to_string(), span: mk_span() },
            ty: Some(Spanned {
                node: TypeExpr::Named("int".to_string()),
                span: mk_span(),
            }),
            value: Spanned {
                node: mk_propagate(mk_method_call("dec", "decode_variant", vec![
                    mk_string_lit(enum_name),
                    mk_var("names"),
                ])),
                span: mk_span(),
            },
            is_mut: false,
        },
        span: mk_span(),
    });

    // Build if/else-if chain for each variant
    let mut current_else_block: Option<Spanned<Block>> = None;

    for (variant_index, variant) in enum_decl.variants.iter().enumerate().rev() {
        let variant_name = &variant.name.node;

        // Build body for this variant
        let mut variant_stmts = Vec::new();

        // Decode each field
        for (field_index, field) in variant.fields.iter().enumerate() {
            // dec.decode_field("field_name", field_index)!
            variant_stmts.push(mk_stmt_expr(mk_propagate(mk_method_call(
                "dec",
                "decode_field",
                vec![
                    mk_string_lit(&field.name.node),
                    mk_int_lit(field_index as i64),
                ],
            ))));

            // let field_name = decode_type(dec)!
            variant_stmts.extend(mk_let_decode(&field.name.node, &field.ty.node)?);
        }

        // dec.decode_variant_end()
        variant_stmts.push(mk_stmt_expr(mk_method_call("dec", "decode_variant_end", vec![])));

        // Build enum variant literal
        let variant_expr = if variant.fields.is_empty() {
            // Unit variant: EnumName.VariantName
            Expr::EnumUnit {
                enum_name: Spanned { node: enum_name.clone(), span: mk_span() },
                variant: Spanned { node: variant_name.clone(), span: mk_span() },
                type_args: vec![],
                enum_id: Some(enum_decl.id),
                variant_id: Some(variant.id),
            }
        } else {
            // Data variant: EnumName.VariantName { field1: field1, ... }
            Expr::EnumData {
                enum_name: Spanned { node: enum_name.clone(), span: mk_span() },
                variant: Spanned { node: variant_name.clone(), span: mk_span() },
                type_args: vec![],
                fields: variant.fields.iter()
                    .map(|f| (
                        Spanned { node: f.name.node.clone(), span: mk_span() },
                        Spanned { node: mk_var(&f.name.node), span: mk_span() },
                    ))
                    .collect(),
                enum_id: Some(enum_decl.id),
                variant_id: Some(variant.id),
            }
        };

        variant_stmts.push(mk_return(variant_expr));

        let variant_block = Spanned {
            node: Block { stmts: variant_stmts },
            span: mk_span(),
        };

        // Build condition: idx == variant_index
        let condition = Expr::BinOp {
            lhs: Box::new(Spanned { node: mk_var("idx"), span: mk_span() }),
            op: crate::parser::ast::BinOp::Eq,
            rhs: Box::new(Spanned { node: mk_int_lit(variant_index as i64), span: mk_span() }),
        };

        // Build if statement with the current else block
        let if_stmt = Stmt::If {
            condition: Spanned { node: condition, span: mk_span() },
            then_block: variant_block,
            else_block: current_else_block,
        };

        // Wrap the if statement in a block for the next iteration's else
        current_else_block = Some(Spanned {
            node: Block {
                stmts: vec![Spanned { node: if_stmt, span: mk_span() }],
            },
            span: mk_span(),
        });
    }

    // Add the final if/else chain to statements
    if let Some(else_block) = current_else_block {
        // Unwrap the outer block and add its statements directly
        stmts.extend(else_block.node.stmts);
    }

    // After all if/else, raise WireError
    // TODO: When Pluto supports error enums, use WireError.UnknownVariant { type_name, index }
    // For now, use a message string
    stmts.push(Spanned {
        node: Stmt::Raise {
            error_name: Spanned { node: "wire.WireError".to_string(), span: mk_span() },
            fields: vec![
                (
                    Spanned { node: "message".to_string(), span: mk_span() },
                    Spanned {
                        node: mk_string_lit(&format!("Unknown variant for enum {}", enum_name)),
                        span: mk_span(),
                    },
                ),
            ],
            error_id: None,
        },
        span: mk_span(),
    });

    let body = Spanned {
        node: Block { stmts },
        span: mk_span(),
    };

    let function = Function {
        id: Uuid::new_v4(),
        name: Spanned { node: fn_name, span: mk_span() },
        type_params: vec![],
        type_param_bounds: HashMap::new(),
        params: vec![Param {
            id: Uuid::new_v4(),
            name: Spanned { node: "dec".to_string(), span: mk_span() },
            ty: Spanned {
                node: TypeExpr::Named("wire.WireValueDecoder".to_string()),
                span: mk_span(),
            },
            is_mut: false,
        }],
        return_type: Some(Spanned {
            node: TypeExpr::Named(enum_name.clone()),
            span: mk_span(),
        }),
        contracts: vec![],
        body,
        is_pub: false,
        is_override: false,
        is_generator: false,
    };

    Ok(Spanned { node: function, span: mk_span() })
}

// ══════════════════════════════════════════════════════════════════════════════
// AST Helper Functions
// ══════════════════════════════════════════════════════════════════════════════

// Global span counter for generated AST nodes to avoid HashMap collisions in type checking.
// Uses atomic to be thread-safe (though currently single-threaded).
static MARSHAL_SPAN_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn mk_span() -> Span {
    // Generate unique spans for each AST node to avoid method resolution collisions.
    // Start at 20_000_000 to avoid colliding with real source spans.
    let offset = MARSHAL_SPAN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let start = 20_000_000 + offset;
    Span { start, end: start + 1, file_id: 0 }
}

fn mk_stmt_expr(expr: Expr) -> Spanned<Stmt> {
    Spanned {
        node: Stmt::Expr(Spanned { node: expr, span: mk_span() }),
        span: mk_span(),
    }
}

fn mk_call(name: &str, args: Vec<Expr>) -> Expr {
    // Handle method calls (name contains .)
    if name.contains('.') {
        let parts: Vec<&str> = name.split('.').collect();
        Expr::MethodCall {
            object: Box::new(Spanned {
                node: mk_var(parts[0]),
                span: mk_span(),
            }),
            method: Spanned {
                node: parts[1].to_string(),
                span: mk_span(),
            },
            args: args.into_iter().map(|e| Spanned { node: e, span: mk_span() }).collect(),
        }
    } else {
        Expr::Call {
            name: Spanned { node: name.to_string(), span: mk_span() },
            type_args: vec![],
            args: args.into_iter().map(|e| Spanned { node: e, span: mk_span() }).collect(),
            target_id: None,
        }
    }
}

fn mk_var(name: &str) -> Expr {
    Expr::Ident(name.to_string())
}

fn mk_int_lit(value: i64) -> Expr {
    Expr::IntLit(value)
}

fn mk_string_lit(value: &str) -> Expr {
    Expr::StringLit(value.to_string())
}

fn mk_field_access(object: &str, field: &str) -> Expr {
    Expr::FieldAccess {
        object: Box::new(Spanned { node: mk_var(object), span: mk_span() }),
        field: Spanned { node: field.to_string(), span: mk_span() },
    }
}

fn mk_propagate(expr: Expr) -> Expr {
    Expr::Propagate {
        expr: Box::new(Spanned { node: expr, span: mk_span() }),
    }
}

fn mk_method_call(object_name: &str, method: &str, args: Vec<Expr>) -> Expr {
    Expr::MethodCall {
        object: Box::new(Spanned { node: mk_var(object_name), span: mk_span() }),
        method: Spanned { node: method.to_string(), span: mk_span() },
        args: args.into_iter().map(|e| Spanned { node: e, span: mk_span() }).collect(),
    }
}

fn mk_method_call_on_expr(object_expr: Expr, method: &str, args: Vec<Expr>) -> Expr {
    Expr::MethodCall {
        object: Box::new(Spanned { node: object_expr, span: mk_span() }),
        method: Spanned { node: method.to_string(), span: mk_span() },
        args: args.into_iter().map(|e| Spanned { node: e, span: mk_span() }).collect(),
    }
}

fn mk_return(expr: Expr) -> Spanned<Stmt> {
    Spanned {
        node: Stmt::Return(Some(Spanned { node: expr, span: mk_span() })),
        span: mk_span(),
    }
}

fn mk_struct_lit(type_name: &str, fields: Vec<(String, Expr)>) -> Expr {
    Expr::StructLit {
        name: Spanned { node: type_name.to_string(), span: mk_span() },
        type_args: vec![],
        fields: fields.into_iter().map(|(name, expr)| {
            (
                Spanned { node: name, span: mk_span() },
                Spanned { node: expr, span: mk_span() },
            )
        }).collect(),
        target_id: None,
    }
}

/// Generates statement(s) to encode a value of the given type.
/// Returns a vector of statements (may be multiple for complex types like arrays).
fn mk_encode_value(ty: &TypeExpr, value_expr: Expr) -> Result<Vec<Spanned<Stmt>>, CompileError> {
    match ty {
        TypeExpr::Named(name) => {
            let encode_expr = match name.as_str() {
                "int" => mk_method_call("enc", "encode_int", vec![value_expr]),
                "float" => mk_method_call("enc", "encode_float", vec![value_expr]),
                "bool" => mk_method_call("enc", "encode_bool", vec![value_expr]),
                "string" => mk_method_call("enc", "encode_string", vec![value_expr]),
                "byte" => {
                    // byte → encode_int(value as int)
                    mk_method_call("enc", "encode_int", vec![
                        Expr::Cast {
                            expr: Box::new(Spanned { node: value_expr, span: mk_span() }),
                            target_type: Spanned {
                                node: TypeExpr::Named("int".to_string()),
                                span: mk_span(),
                            },
                        }
                    ])
                }
                "void" => {
                    // void → no encoding needed
                    return Ok(vec![Spanned {
                        node: Stmt::Expr(Spanned { node: Expr::IntLit(0), span: mk_span() }),
                        span: mk_span(),
                    }]);
                }
                _ => {
                    // User-defined class or enum → call __marshal_T(value, enc)
                    mk_call(&format!("__marshal_{}", name), vec![value_expr, mk_var("enc")])
                }
            };
            Ok(vec![mk_stmt_expr(encode_expr)])
        }

        TypeExpr::Array(elem_ty) => {
            // enc.encode_array_start(arr.len())
            // let mut i = 0
            // while i < arr.len() {
            //     __marshal_T(arr[i], enc)
            //     i = i + 1
            // }
            // enc.encode_array_end()

            let mut stmts = Vec::new();

            // enc.encode_array_start(value.len())
            stmts.push(mk_stmt_expr(mk_method_call(
                "enc",
                "encode_array_start",
                vec![mk_method_call_on_expr(value_expr.clone(), "len", vec![])]
            )));

            // let mut __i = 0
            stmts.push(Spanned {
                node: Stmt::Let {
                    name: Spanned { node: "__i".to_string(), span: mk_span() },
                    ty: Some(Spanned {
                        node: TypeExpr::Named("int".to_string()),
                        span: mk_span(),
                    }),
                    value: Spanned { node: mk_int_lit(0), span: mk_span() },
                    is_mut: true,
                },
                span: mk_span(),
            });

            // while __i < value.len() { ... }
            let mut loop_body = Vec::new();

            // __marshal_T(value[__i], enc) or enc.encode_primitive(value[__i])
            let index_expr = Expr::Index {
                object: Box::new(Spanned { node: value_expr.clone(), span: mk_span() }),
                index: Box::new(Spanned { node: mk_var("__i"), span: mk_span() }),
            };
            loop_body.extend(mk_encode_value(&elem_ty.node, index_expr)?);

            // __i = __i + 1
            loop_body.push(Spanned {
                node: Stmt::Assign {
                    target: Spanned { node: "__i".to_string(), span: mk_span() },
                    value: Spanned {
                        node: Expr::BinOp {
                            lhs: Box::new(Spanned { node: mk_var("__i"), span: mk_span() }),
                            op: crate::parser::ast::BinOp::Add,
                            rhs: Box::new(Spanned { node: mk_int_lit(1), span: mk_span() }),
                        },
                        span: mk_span(),
                    },
                },
                span: mk_span(),
            });

            let while_stmt = Stmt::While {
                condition: Spanned {
                    node: Expr::BinOp {
                        lhs: Box::new(Spanned { node: mk_var("__i"), span: mk_span() }),
                        op: crate::parser::ast::BinOp::Lt,
                        rhs: Box::new(Spanned {
                            node: mk_method_call_on_expr(value_expr.clone(), "len", vec![]),
                            span: mk_span(),
                        }),
                    },
                    span: mk_span(),
                },
                body: Spanned {
                    node: Block { stmts: loop_body },
                    span: mk_span(),
                },
            };
            stmts.push(Spanned { node: while_stmt, span: mk_span() });

            // enc.encode_array_end()
            stmts.push(mk_stmt_expr(mk_method_call("enc", "encode_array_end", vec![])));

            Ok(stmts)
        }

        TypeExpr::Nullable(inner_ty) => {
            // if value == none {
            //     enc.encode_null()
            // } else {
            //     __marshal_T(value?, enc)
            // }

            let condition = Expr::BinOp {
                lhs: Box::new(Spanned { node: value_expr.clone(), span: mk_span() }),
                op: crate::parser::ast::BinOp::Eq,
                rhs: Box::new(Spanned { node: Expr::NoneLit, span: mk_span() }),
            };

            let then_block = Spanned {
                node: Block {
                    stmts: vec![mk_stmt_expr(mk_method_call("enc", "encode_null", vec![]))],
                },
                span: mk_span(),
            };

            // Unwrap with ? operator
            let unwrapped = Expr::NullPropagate {
                expr: Box::new(Spanned { node: value_expr, span: mk_span() }),
            };

            let else_block = Spanned {
                node: Block {
                    stmts: mk_encode_value(&inner_ty.node, unwrapped)?,
                },
                span: mk_span(),
            };

            let if_stmt = Stmt::If {
                condition: Spanned { node: condition, span: mk_span() },
                then_block,
                else_block: Some(else_block),
            };

            Ok(vec![Spanned { node: if_stmt, span: mk_span() }])
        }

        TypeExpr::Generic { name, type_args } => {
            match name.as_str() {
                "Map" => {
                    // enc.encode_map_start(map.len())
                    // let keys = map.keys()
                    // let mut __i = 0
                    // while __i < keys.len() {
                    //     let key = keys[__i]
                    //     __marshal_K(key, enc)
                    //     __marshal_V(map[key], enc)
                    //     __i = __i + 1
                    // }
                    // enc.encode_map_end()

                    if type_args.len() != 2 {
                        return Err(CompileError::codegen(
                            "Map type must have exactly 2 type arguments"
                        ));
                    }

                    let key_ty = &type_args[0].node;
                    let val_ty = &type_args[1].node;
                    let mut stmts = Vec::new();

                    // enc.encode_map_start(value.len())
                    stmts.push(mk_stmt_expr(mk_method_call(
                        "enc",
                        "encode_map_start",
                        vec![mk_method_call_on_expr(value_expr.clone(), "len", vec![])]
                    )));

                    // let keys = value.keys()
                    stmts.push(Spanned {
                        node: Stmt::Let {
                            name: Spanned { node: "__keys".to_string(), span: mk_span() },
                            ty: None, // Type inference
                            value: Spanned {
                                node: mk_method_call_on_expr(value_expr.clone(), "keys", vec![]),
                                span: mk_span(),
                            },
                            is_mut: false,
                        },
                        span: mk_span(),
                    });

                    // let mut __i = 0
                    stmts.push(Spanned {
                        node: Stmt::Let {
                            name: Spanned { node: "__i".to_string(), span: mk_span() },
                            ty: Some(Spanned {
                                node: TypeExpr::Named("int".to_string()),
                                span: mk_span(),
                            }),
                            value: Spanned { node: mk_int_lit(0), span: mk_span() },
                            is_mut: false,
                        },
                        span: mk_span(),
                    });

                    // while __i < keys.len() { ... }
                    let mut loop_body = Vec::new();

                    // let key = keys[__i]
                    loop_body.push(Spanned {
                        node: Stmt::Let {
                            name: Spanned { node: "__key".to_string(), span: mk_span() },
                            ty: None,
                            value: Spanned {
                                node: Expr::Index {
                                    object: Box::new(Spanned { node: mk_var("__keys"), span: mk_span() }),
                                    index: Box::new(Spanned { node: mk_var("__i"), span: mk_span() }),
                                },
                                span: mk_span(),
                            },
                            is_mut: false,
                        },
                        span: mk_span(),
                    });

                    // Encode key
                    loop_body.extend(mk_encode_value(key_ty, mk_var("__key"))?);

                    // Encode value: map[key]
                    let map_index_expr = Expr::Index {
                        object: Box::new(Spanned { node: value_expr.clone(), span: mk_span() }),
                        index: Box::new(Spanned { node: mk_var("__key"), span: mk_span() }),
                    };
                    loop_body.extend(mk_encode_value(val_ty, map_index_expr)?);

                    // __i = __i + 1
                    loop_body.push(Spanned {
                        node: Stmt::Assign {
                            target: Spanned { node: "__i".to_string(), span: mk_span() },
                            value: Spanned {
                                node: Expr::BinOp {
                                    lhs: Box::new(Spanned { node: mk_var("__i"), span: mk_span() }),
                                    op: crate::parser::ast::BinOp::Add,
                                    rhs: Box::new(Spanned { node: mk_int_lit(1), span: mk_span() }),
                                },
                                span: mk_span(),
                            },
                        },
                        span: mk_span(),
                    });

                    let while_stmt = Stmt::While {
                        condition: Spanned {
                            node: Expr::BinOp {
                                lhs: Box::new(Spanned { node: mk_var("__i"), span: mk_span() }),
                                op: crate::parser::ast::BinOp::Lt,
                                rhs: Box::new(Spanned {
                                    node: mk_call("__keys.len", vec![]),
                                    span: mk_span(),
                                }),
                            },
                            span: mk_span(),
                        },
                        body: Spanned {
                            node: Block { stmts: loop_body },
                            span: mk_span(),
                        },
                    };
                    stmts.push(Spanned { node: while_stmt, span: mk_span() });

                    // enc.encode_map_end()
                    stmts.push(mk_stmt_expr(mk_method_call("enc", "encode_map_end", vec![])));

                    Ok(stmts)
                }
                "Set" => {
                    // Set encoded as array
                    // enc.encode_array_start(set.len())
                    // let arr = set.to_array()
                    // let mut __i = 0
                    // while __i < arr.len() {
                    //     __marshal_T(arr[__i], enc)
                    //     __i = __i + 1
                    // }
                    // enc.encode_array_end()

                    if type_args.len() != 1 {
                        return Err(CompileError::codegen(
                            "Set type must have exactly 1 type argument"
                        ));
                    }

                    let elem_ty = &type_args[0].node;
                    let mut stmts = Vec::new();

                    // enc.encode_array_start(value.len())
                    stmts.push(mk_stmt_expr(mk_method_call(
                        "enc",
                        "encode_array_start",
                        vec![mk_method_call_on_expr(value_expr.clone(), "len", vec![])]
                    )));

                    // let arr = value.to_array()
                    stmts.push(Spanned {
                        node: Stmt::Let {
                            name: Spanned { node: "__arr".to_string(), span: mk_span() },
                            ty: None,
                            value: Spanned {
                                node: mk_method_call_on_expr(value_expr.clone(), "to_array", vec![]),
                                span: mk_span(),
                            },
                            is_mut: false,
                        },
                        span: mk_span(),
                    });

                    // let mut __i = 0
                    stmts.push(Spanned {
                        node: Stmt::Let {
                            name: Spanned { node: "__i".to_string(), span: mk_span() },
                            ty: Some(Spanned {
                                node: TypeExpr::Named("int".to_string()),
                                span: mk_span(),
                            }),
                            value: Spanned { node: mk_int_lit(0), span: mk_span() },
                            is_mut: false,
                        },
                        span: mk_span(),
                    });

                    // while __i < arr.len() { ... }
                    let mut loop_body = Vec::new();

                    // Encode element: arr[__i]
                    let index_expr = Expr::Index {
                        object: Box::new(Spanned { node: mk_var("__arr"), span: mk_span() }),
                        index: Box::new(Spanned { node: mk_var("__i"), span: mk_span() }),
                    };
                    loop_body.extend(mk_encode_value(elem_ty, index_expr)?);

                    // __i = __i + 1
                    loop_body.push(Spanned {
                        node: Stmt::Assign {
                            target: Spanned { node: "__i".to_string(), span: mk_span() },
                            value: Spanned {
                                node: Expr::BinOp {
                                    lhs: Box::new(Spanned { node: mk_var("__i"), span: mk_span() }),
                                    op: crate::parser::ast::BinOp::Add,
                                    rhs: Box::new(Spanned { node: mk_int_lit(1), span: mk_span() }),
                                },
                                span: mk_span(),
                            },
                        },
                        span: mk_span(),
                    });

                    let while_stmt = Stmt::While {
                        condition: Spanned {
                            node: Expr::BinOp {
                                lhs: Box::new(Spanned { node: mk_var("__i"), span: mk_span() }),
                                op: crate::parser::ast::BinOp::Lt,
                                rhs: Box::new(Spanned {
                                    node: mk_call("__arr.len", vec![]),
                                    span: mk_span(),
                                }),
                            },
                            span: mk_span(),
                        },
                        body: Spanned {
                            node: Block { stmts: loop_body },
                            span: mk_span(),
                        },
                    };
                    stmts.push(Spanned { node: while_stmt, span: mk_span() });

                    // enc.encode_array_end()
                    stmts.push(mk_stmt_expr(mk_method_call("enc", "encode_array_end", vec![])));

                    Ok(stmts)
                }
                _ => {
                    // Generic user type like Box<T> → call __marshal_Box__int(value, enc)
                    let mangled_name = mangle_generic_name(name, type_args);
                    let encode_expr = mk_call(&format!("__marshal_{}", mangled_name), vec![value_expr, mk_var("enc")]);
                    Ok(vec![mk_stmt_expr(encode_expr)])
                }
            }
        }

        _ => {
            // Other types: Qualified, Fn, Stream
            unimplemented!("Encoding for this type not yet implemented")
        }
    }
}

/// Helper to convert simple expressions to strings for method calls
fn value_expr_to_string(expr: &Expr) -> Result<String, CompileError> {
    match expr {
        Expr::Ident(name) => Ok(name.clone()),
        Expr::FieldAccess { object, field } => {
            let obj_str = value_expr_to_string(&object.node)?;
            Ok(format!("{}.{}", obj_str, field.node))
        }
        _ => Err(CompileError::codegen(
            "Complex expression not supported in array length call"
        )),
    }
}

/// Helper to mangle generic type names
fn mangle_generic_name(base_name: &str, type_args: &[Spanned<TypeExpr>]) -> String {
    let mut result = base_name.to_string();
    result.push_str("$$");
    for (i, arg) in type_args.iter().enumerate() {
        if i > 0 {
            result.push('$');
        }
        result.push_str(&type_expr_to_string(&arg.node));
    }
    result
}

/// Helper to convert TypeExpr to string for mangling
fn type_expr_to_string(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named(name) => name.clone(),
        TypeExpr::Array(elem_ty) => format!("array_{}", type_expr_to_string(&elem_ty.node)),
        TypeExpr::Generic { name, type_args } => mangle_generic_name(name, type_args),
        _ => "unknown".to_string(),
    }
}

/// Generates statements to decode a value of the given type.
/// For simple types: Returns a single let statement: let var_name = decode_type(dec)!
/// For complex types: Returns multiple statements ending with the final let binding
fn mk_let_decode(var_name: &str, ty: &TypeExpr) -> Result<Vec<Spanned<Stmt>>, CompileError> {
    match ty {
        TypeExpr::Named(name) => {
            let decode_expr = match name.as_str() {
                "int" => mk_propagate(mk_method_call("dec", "decode_int", vec![])),
                "float" => mk_propagate(mk_method_call("dec", "decode_float", vec![])),
                "bool" => mk_propagate(mk_method_call("dec", "decode_bool", vec![])),
                "string" => mk_propagate(mk_method_call("dec", "decode_string", vec![])),
                "byte" => {
                    // byte → decode_int()! as byte
                    Expr::Cast {
                        expr: Box::new(Spanned {
                            node: mk_propagate(mk_method_call("dec", "decode_int", vec![])),
                            span: mk_span(),
                        }),
                        target_type: Spanned {
                            node: TypeExpr::Named("byte".to_string()),
                            span: mk_span(),
                        },
                    }
                }
                "void" => {
                    // void → no decoding needed
                    return Ok(vec![Spanned {
                        node: Stmt::Expr(Spanned { node: Expr::IntLit(0), span: mk_span() }),
                        span: mk_span(),
                    }]);
                }
                _ => {
                    // User-defined class or enum → call __unmarshal_T(dec)!
                    mk_propagate(mk_call(&format!("__unmarshal_{}", name), vec![mk_var("dec")]))
                }
            };

            Ok(vec![Spanned {
                node: Stmt::Let {
                    name: Spanned { node: var_name.to_string(), span: mk_span() },
                    ty: Some(Spanned { node: ty.clone(), span: mk_span() }),
                    value: Spanned { node: decode_expr, span: mk_span() },
                    is_mut: false,
                },
                span: mk_span(),
            }])
        }

        TypeExpr::Array(elem_ty) => {
            // let __len = dec.decode_array_start()!
            // let mut __result: [T] = []
            // let mut __i = 0
            // while __i < __len {
            //     let elem = decode_T(dec)!
            //     __result.push(elem)
            //     __i = __i + 1
            // }
            // dec.decode_array_end()!
            // let var_name = __result

            let mut stmts = Vec::new();

            // let __len = dec.decode_array_start()!
            stmts.push(Spanned {
                node: Stmt::Let {
                    name: Spanned { node: "__len".to_string(), span: mk_span() },
                    ty: None,
                    value: Spanned {
                        node: mk_propagate(mk_method_call("dec", "decode_array_start", vec![])),
                        span: mk_span(),
                    },
                    is_mut: false,
                },
                span: mk_span(),
            });

            // let mut __result: [T] = []
            stmts.push(Spanned {
                node: Stmt::Let {
                    name: Spanned { node: "__result".to_string(), span: mk_span() },
                    ty: Some(Spanned { node: ty.clone(), span: mk_span() }),
                    value: Spanned {
                        node: Expr::ArrayLit { elements: vec![] },
                        span: mk_span(),
                    },
                    is_mut: true,
                },
                span: mk_span(),
            });

            // let mut __i = 0
            stmts.push(Spanned {
                node: Stmt::Let {
                    name: Spanned { node: "__i".to_string(), span: mk_span() },
                    ty: Some(Spanned {
                        node: TypeExpr::Named("int".to_string()),
                        span: mk_span(),
                    }),
                    value: Spanned { node: mk_int_lit(0), span: mk_span() },
                    is_mut: true,
                },
                span: mk_span(),
            });

            // while __i < __len { ... }
            let mut loop_body = Vec::new();

            // dec.decode_array_element(__i)!
            loop_body.push(mk_stmt_expr(mk_propagate(mk_method_call("dec", "decode_array_element", vec![mk_var("__i")]))));

            // let elem = decode_T(dec)!
            loop_body.extend(mk_let_decode("__elem", &elem_ty.node)?);

            // __result.push(elem)
            loop_body.push(mk_stmt_expr(mk_call("__result.push", vec![mk_var("__elem")])));

            // __i = __i + 1
            loop_body.push(Spanned {
                node: Stmt::Assign {
                    target: Spanned { node: "__i".to_string(), span: mk_span() },
                    value: Spanned {
                        node: Expr::BinOp {
                            lhs: Box::new(Spanned { node: mk_var("__i"), span: mk_span() }),
                            op: crate::parser::ast::BinOp::Add,
                            rhs: Box::new(Spanned { node: mk_int_lit(1), span: mk_span() }),
                        },
                        span: mk_span(),
                    },
                },
                span: mk_span(),
            });

            let while_stmt = Stmt::While {
                condition: Spanned {
                    node: Expr::BinOp {
                        lhs: Box::new(Spanned { node: mk_var("__i"), span: mk_span() }),
                        op: crate::parser::ast::BinOp::Lt,
                        rhs: Box::new(Spanned { node: mk_var("__len"), span: mk_span() }),
                    },
                    span: mk_span(),
                },
                body: Spanned {
                    node: Block { stmts: loop_body },
                    span: mk_span(),
                },
            };
            stmts.push(Spanned { node: while_stmt, span: mk_span() });

            // dec.decode_array_end()
            stmts.push(mk_stmt_expr(mk_method_call("dec", "decode_array_end", vec![])));

            // let var_name = __result
            stmts.push(Spanned {
                node: Stmt::Let {
                    name: Spanned { node: var_name.to_string(), span: mk_span() },
                    ty: Some(Spanned { node: ty.clone(), span: mk_span() }),
                    value: Spanned { node: mk_var("__result"), span: mk_span() },
                    is_mut: false,
                },
                span: mk_span(),
            });

            Ok(stmts)
        }

        TypeExpr::Nullable(inner_ty) => {
            // let __is_present = dec.decode_nullable()!
            // let mut __result: T? = none
            // if __is_present {
            //     let __inner = decode_T(dec)!
            //     __result = __inner
            // }
            // let var_name = __result

            let mut stmts = Vec::new();

            // let __is_present = dec.decode_nullable()
            stmts.push(Spanned {
                node: Stmt::Let {
                    name: Spanned { node: "__is_present".to_string(), span: mk_span() },
                    ty: None,
                    value: Spanned {
                        node: mk_method_call("dec", "decode_nullable", vec![]),
                        span: mk_span(),
                    },
                    is_mut: false,
                },
                span: mk_span(),
            });

            // let mut __result: T? = none
            stmts.push(Spanned {
                node: Stmt::Let {
                    name: Spanned { node: "__result".to_string(), span: mk_span() },
                    ty: Some(Spanned { node: ty.clone(), span: mk_span() }),
                    value: Spanned { node: Expr::NoneLit, span: mk_span() },
                    is_mut: true,
                },
                span: mk_span(),
            });

            // if !__is_null { ... }
            let mut if_body = Vec::new();

            // For simple types, just decode directly; for complex types, recursively call mk_let_decode
            match &inner_ty.node {
                TypeExpr::Named(name) if matches!(name.as_str(), "int" | "float" | "bool" | "string" | "byte") => {
                    // Simple types - decode directly into __result
                    let decode_expr = match name.as_str() {
                        "int" => mk_propagate(mk_method_call("dec", "decode_int", vec![])),
                        "float" => mk_propagate(mk_method_call("dec", "decode_float", vec![])),
                        "bool" => mk_propagate(mk_method_call("dec", "decode_bool", vec![])),
                        "string" => mk_propagate(mk_method_call("dec", "decode_string", vec![])),
                        "byte" => Expr::Cast {
                            expr: Box::new(Spanned {
                                node: mk_propagate(mk_method_call("dec", "decode_int", vec![])),
                                span: mk_span(),
                            }),
                            target_type: Spanned {
                                node: TypeExpr::Named("byte".to_string()),
                                span: mk_span(),
                            },
                        },
                        _ => unreachable!(),
                    };

                    if_body.push(Spanned {
                        node: Stmt::Assign {
                            target: Spanned { node: "__result".to_string(), span: mk_span() },
                            value: Spanned { node: decode_expr, span: mk_span() },
                        },
                        span: mk_span(),
                    });
                }
                TypeExpr::Named(name) => {
                    // User-defined type
                    if_body.push(Spanned {
                        node: Stmt::Assign {
                            target: Spanned { node: "__result".to_string(), span: mk_span() },
                            value: Spanned {
                                node: mk_propagate(mk_call(&format!("__unmarshal_{}", name), vec![mk_var("dec")])),
                                span: mk_span(),
                            },
                        },
                        span: mk_span(),
                    });
                }
                _ => {
                    // Complex inner types - decode into temporary, then assign
                    if_body.extend(mk_let_decode("__inner", &inner_ty.node)?);
                    if_body.push(Spanned {
                        node: Stmt::Assign {
                            target: Spanned { node: "__result".to_string(), span: mk_span() },
                            value: Spanned { node: mk_var("__inner"), span: mk_span() },
                        },
                        span: mk_span(),
                    });
                }
            }

            let if_stmt = Stmt::If {
                condition: Spanned {
                    node: mk_var("__is_present"),
                    span: mk_span(),
                },
                then_block: Spanned {
                    node: Block { stmts: if_body },
                    span: mk_span(),
                },
                else_block: None,
            };
            stmts.push(Spanned { node: if_stmt, span: mk_span() });

            // let var_name = __result
            stmts.push(Spanned {
                node: Stmt::Let {
                    name: Spanned { node: var_name.to_string(), span: mk_span() },
                    ty: Some(Spanned { node: ty.clone(), span: mk_span() }),
                    value: Spanned { node: mk_var("__result"), span: mk_span() },
                    is_mut: false,
                },
                span: mk_span(),
            });

            Ok(stmts)
        }

        TypeExpr::Generic { name, type_args } => {
            match name.as_str() {
                "Map" => {
                    // Similar to array but decode key-value pairs
                    if type_args.len() != 2 {
                        return Err(CompileError::codegen(
                            "Map type must have exactly 2 type arguments"
                        ));
                    }

                    let key_ty = &type_args[0].node;
                    let val_ty = &type_args[1].node;
                    let mut stmts = Vec::new();

                    // let __len = dec.decode_map_start()!
                    stmts.push(Spanned {
                        node: Stmt::Let {
                            name: Spanned { node: "__len".to_string(), span: mk_span() },
                            ty: None,
                            value: Spanned {
                                node: mk_method_call("dec", "decode_map_start", vec![]),
                                span: mk_span(),
                            },
                            is_mut: false,
                        },
                        span: mk_span(),
                    });

                    // let mut __result: Map<K,V> = Map<K,V> {}
                    stmts.push(Spanned {
                        node: Stmt::Let {
                            name: Spanned { node: "__result".to_string(), span: mk_span() },
                            ty: Some(Spanned { node: ty.clone(), span: mk_span() }),
                            value: Spanned {
                                node: Expr::MapLit {
                                    key_type: type_args[0].clone(),
                                    value_type: type_args[1].clone(),
                                    entries: vec![],
                                },
                                span: mk_span(),
                            },
                            is_mut: false,
                        },
                        span: mk_span(),
                    });

                    // let mut __i = 0
                    stmts.push(Spanned {
                        node: Stmt::Let {
                            name: Spanned { node: "__i".to_string(), span: mk_span() },
                            ty: Some(Spanned {
                                node: TypeExpr::Named("int".to_string()),
                                span: mk_span(),
                            }),
                            value: Spanned { node: mk_int_lit(0), span: mk_span() },
                            is_mut: false,
                        },
                        span: mk_span(),
                    });

                    // while __i < __len { ... }
                    let mut loop_body = Vec::new();

                    // let key = decode_K(dec)!
                    loop_body.extend(mk_let_decode("__key", key_ty)?);

                    // let val = decode_V(dec)!
                    loop_body.extend(mk_let_decode("__val", val_ty)?);

                    // __result.insert(__key, __val)
                    loop_body.push(mk_stmt_expr(mk_call("__result.insert", vec![mk_var("__key"), mk_var("__val")])));

                    // __i = __i + 1
                    loop_body.push(Spanned {
                        node: Stmt::Assign {
                            target: Spanned { node: "__i".to_string(), span: mk_span() },
                            value: Spanned {
                                node: Expr::BinOp {
                                    lhs: Box::new(Spanned { node: mk_var("__i"), span: mk_span() }),
                                    op: crate::parser::ast::BinOp::Add,
                                    rhs: Box::new(Spanned { node: mk_int_lit(1), span: mk_span() }),
                                },
                                span: mk_span(),
                            },
                        },
                        span: mk_span(),
                    });

                    let while_stmt = Stmt::While {
                        condition: Spanned {
                            node: Expr::BinOp {
                                lhs: Box::new(Spanned { node: mk_var("__i"), span: mk_span() }),
                                op: crate::parser::ast::BinOp::Lt,
                                rhs: Box::new(Spanned { node: mk_var("__len"), span: mk_span() }),
                            },
                            span: mk_span(),
                        },
                        body: Spanned {
                            node: Block { stmts: loop_body },
                            span: mk_span(),
                        },
                    };
                    stmts.push(Spanned { node: while_stmt, span: mk_span() });

                    // dec.decode_map_end()
                    stmts.push(mk_stmt_expr(mk_method_call("dec", "decode_map_end", vec![])));

                    // let var_name = __result
                    stmts.push(Spanned {
                        node: Stmt::Let {
                            name: Spanned { node: var_name.to_string(), span: mk_span() },
                            ty: Some(Spanned { node: ty.clone(), span: mk_span() }),
                            value: Spanned { node: mk_var("__result"), span: mk_span() },
                            is_mut: false,
                        },
                        span: mk_span(),
                    });

                    Ok(stmts)
                }
                "Set" => {
                    // Decode as array, convert to set
                    if type_args.len() != 1 {
                        return Err(CompileError::codegen(
                            "Set type must have exactly 1 type argument"
                        ));
                    }

                    let elem_ty = &type_args[0].node;
                    let mut stmts = Vec::new();

                    // let __len = dec.decode_array_start()!
                    stmts.push(Spanned {
                        node: Stmt::Let {
                            name: Spanned { node: "__len".to_string(), span: mk_span() },
                            ty: None,
                            value: Spanned {
                                node: mk_method_call("dec", "decode_array_start", vec![]),
                                span: mk_span(),
                            },
                            is_mut: false,
                        },
                        span: mk_span(),
                    });

                    // let mut __result: Set<T> = Set<T> {}
                    stmts.push(Spanned {
                        node: Stmt::Let {
                            name: Spanned { node: "__result".to_string(), span: mk_span() },
                            ty: Some(Spanned { node: ty.clone(), span: mk_span() }),
                            value: Spanned {
                                node: Expr::SetLit {
                                    elem_type: type_args[0].clone(),
                                    elements: vec![],
                                },
                                span: mk_span(),
                            },
                            is_mut: false,
                        },
                        span: mk_span(),
                    });

                    // let mut __i = 0
                    stmts.push(Spanned {
                        node: Stmt::Let {
                            name: Spanned { node: "__i".to_string(), span: mk_span() },
                            ty: Some(Spanned {
                                node: TypeExpr::Named("int".to_string()),
                                span: mk_span(),
                            }),
                            value: Spanned { node: mk_int_lit(0), span: mk_span() },
                            is_mut: false,
                        },
                        span: mk_span(),
                    });

                    // while __i < __len { ... }
                    let mut loop_body = Vec::new();

                    // let elem = decode_T(dec)!
                    loop_body.extend(mk_let_decode("__elem", elem_ty)?);

                    // __result.insert(__elem)
                    loop_body.push(mk_stmt_expr(mk_call("__result.insert", vec![mk_var("__elem")])));

                    // __i = __i + 1
                    loop_body.push(Spanned {
                        node: Stmt::Assign {
                            target: Spanned { node: "__i".to_string(), span: mk_span() },
                            value: Spanned {
                                node: Expr::BinOp {
                                    lhs: Box::new(Spanned { node: mk_var("__i"), span: mk_span() }),
                                    op: crate::parser::ast::BinOp::Add,
                                    rhs: Box::new(Spanned { node: mk_int_lit(1), span: mk_span() }),
                                },
                                span: mk_span(),
                            },
                        },
                        span: mk_span(),
                    });

                    let while_stmt = Stmt::While {
                        condition: Spanned {
                            node: Expr::BinOp {
                                lhs: Box::new(Spanned { node: mk_var("__i"), span: mk_span() }),
                                op: crate::parser::ast::BinOp::Lt,
                                rhs: Box::new(Spanned { node: mk_var("__len"), span: mk_span() }),
                            },
                            span: mk_span(),
                        },
                        body: Spanned {
                            node: Block { stmts: loop_body },
                            span: mk_span(),
                        },
                    };
                    stmts.push(Spanned { node: while_stmt, span: mk_span() });

                    // dec.decode_array_end()
                    stmts.push(mk_stmt_expr(mk_method_call("dec", "decode_array_end", vec![])));

                    // let var_name = __result
                    stmts.push(Spanned {
                        node: Stmt::Let {
                            name: Spanned { node: var_name.to_string(), span: mk_span() },
                            ty: Some(Spanned { node: ty.clone(), span: mk_span() }),
                            value: Spanned { node: mk_var("__result"), span: mk_span() },
                            is_mut: false,
                        },
                        span: mk_span(),
                    });

                    Ok(stmts)
                }
                _ => {
                    // Generic user type → call __unmarshal_Box__int(dec)
                    let mangled_name = mangle_generic_name(name, type_args);
                    let decode_expr = mk_call(&format!("__unmarshal_{}", mangled_name), vec![mk_var("dec")]);

                    Ok(vec![Spanned {
                        node: Stmt::Let {
                            name: Spanned { node: var_name.to_string(), span: mk_span() },
                            ty: Some(Spanned { node: ty.clone(), span: mk_span() }),
                            value: Spanned { node: decode_expr, span: mk_span() },
                            is_mut: false,
                        },
                        span: mk_span(),
                    }])
                }
            }
        }

        _ => {
            // Other types: Qualified, Fn, Stream
            Err(CompileError::codegen(
                "Decoding for this type not yet implemented"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{BinOp, TypeExpr};

    // ===== type_expr_to_string tests =====

    #[test]
    fn test_type_expr_to_string_named() {
        let ty = TypeExpr::Named("int".to_string());
        assert_eq!(type_expr_to_string(&ty), "int");

        let ty = TypeExpr::Named("MyClass".to_string());
        assert_eq!(type_expr_to_string(&ty), "MyClass");
    }

    #[test]
    fn test_type_expr_to_string_array() {
        let ty = TypeExpr::Array(Box::new(Spanned {
            node: TypeExpr::Named("int".to_string()),
            span: Span { start: 0, end: 0, file_id: 0 },
        }));
        assert_eq!(type_expr_to_string(&ty), "array_int");
    }

    #[test]
    fn test_type_expr_to_string_nested_array() {
        let ty = TypeExpr::Array(Box::new(Spanned {
            node: TypeExpr::Array(Box::new(Spanned {
                node: TypeExpr::Named("string".to_string()),
                span: Span { start: 0, end: 0, file_id: 0 },
            })),
            span: Span { start: 0, end: 0, file_id: 0 },
        }));
        assert_eq!(type_expr_to_string(&ty), "array_array_string");
    }

    #[test]
    fn test_type_expr_to_string_generic() {
        let ty = TypeExpr::Generic {
            name: "Box".to_string(),
            type_args: vec![Spanned {
                node: TypeExpr::Named("int".to_string()),
                span: Span { start: 0, end: 0, file_id: 0 },
            }],
        };
        assert_eq!(type_expr_to_string(&ty), "Box$$int");
    }

    #[test]
    fn test_type_expr_to_string_generic_multiple_args() {
        let ty = TypeExpr::Generic {
            name: "Pair".to_string(),
            type_args: vec![
                Spanned {
                    node: TypeExpr::Named("int".to_string()),
                    span: Span { start: 0, end: 0, file_id: 0 },
                },
                Spanned {
                    node: TypeExpr::Named("string".to_string()),
                    span: Span { start: 0, end: 0, file_id: 0 },
                },
            ],
        };
        assert_eq!(type_expr_to_string(&ty), "Pair$$int$string");
    }

    #[test]
    fn test_type_expr_to_string_unknown() {
        // Fn types should return "unknown"
        let ty = TypeExpr::Fn {
            params: vec![],
            return_type: Box::new(Spanned {
                node: TypeExpr::Named("void".to_string()),
                span: Span { start: 0, end: 0, file_id: 0 },
            }),
        };
        assert_eq!(type_expr_to_string(&ty), "unknown");
    }

    // ===== mangle_generic_name tests =====

    #[test]
    fn test_mangle_generic_name_single_arg() {
        let type_args = vec![Spanned {
            node: TypeExpr::Named("int".to_string()),
            span: Span { start: 0, end: 0, file_id: 0 },
        }];
        let result = mangle_generic_name("Box", &type_args);
        assert_eq!(result, "Box$$int");
    }

    #[test]
    fn test_mangle_generic_name_multiple_args() {
        let type_args = vec![
            Spanned {
                node: TypeExpr::Named("int".to_string()),
                span: Span { start: 0, end: 0, file_id: 0 },
            },
            Spanned {
                node: TypeExpr::Named("string".to_string()),
                span: Span { start: 0, end: 0, file_id: 0 },
            },
        ];
        let result = mangle_generic_name("Pair", &type_args);
        assert_eq!(result, "Pair$$int$string");
    }

    #[test]
    fn test_mangle_generic_name_three_args() {
        let type_args = vec![
            Spanned {
                node: TypeExpr::Named("int".to_string()),
                span: Span { start: 0, end: 0, file_id: 0 },
            },
            Spanned {
                node: TypeExpr::Named("float".to_string()),
                span: Span { start: 0, end: 0, file_id: 0 },
            },
            Spanned {
                node: TypeExpr::Named("bool".to_string()),
                span: Span { start: 0, end: 0, file_id: 0 },
            },
        ];
        let result = mangle_generic_name("Triple", &type_args);
        assert_eq!(result, "Triple$$int$float$bool");
    }

    #[test]
    fn test_mangle_generic_name_nested_generic() {
        let type_args = vec![Spanned {
            node: TypeExpr::Generic {
                name: "Box".to_string(),
                type_args: vec![Spanned {
                    node: TypeExpr::Named("int".to_string()),
                    span: Span { start: 0, end: 0, file_id: 0 },
                }],
            },
            span: Span { start: 0, end: 0, file_id: 0 },
        }];
        let result = mangle_generic_name("Container", &type_args);
        assert_eq!(result, "Container$$Box$$int");
    }

    #[test]
    fn test_mangle_generic_name_array_type_arg() {
        let type_args = vec![Spanned {
            node: TypeExpr::Array(Box::new(Spanned {
                node: TypeExpr::Named("int".to_string()),
                span: Span { start: 0, end: 0, file_id: 0 },
            })),
            span: Span { start: 0, end: 0, file_id: 0 },
        }];
        let result = mangle_generic_name("Wrapper", &type_args);
        assert_eq!(result, "Wrapper$$array_int");
    }

    // ===== AST construction helper tests =====

    #[test]
    fn test_mk_var() {
        let expr = mk_var("x");
        match expr {
            Expr::Ident(name) => assert_eq!(name, "x"),
            _ => panic!("Expected Ident"),
        }
    }

    #[test]
    fn test_mk_int_lit() {
        let expr = mk_int_lit(42);
        match expr {
            Expr::IntLit(value) => assert_eq!(value, 42),
            _ => panic!("Expected IntLit"),
        }

        let expr = mk_int_lit(-100);
        match expr {
            Expr::IntLit(value) => assert_eq!(value, -100),
            _ => panic!("Expected IntLit"),
        }
    }

    #[test]
    fn test_mk_string_lit() {
        let expr = mk_string_lit("hello");
        match expr {
            Expr::StringLit(value) => assert_eq!(value, "hello"),
            _ => panic!("Expected StringLit"),
        }

        let expr = mk_string_lit("");
        match expr {
            Expr::StringLit(value) => assert_eq!(value, ""),
            _ => panic!("Expected StringLit"),
        }
    }

    #[test]
    fn test_mk_field_access() {
        let expr = mk_field_access("obj", "field");
        match expr {
            Expr::FieldAccess { object, field } => {
                match object.node {
                    Expr::Ident(name) => assert_eq!(name, "obj"),
                    _ => panic!("Expected Ident for object"),
                }
                assert_eq!(field.node, "field");
            }
            _ => panic!("Expected FieldAccess"),
        }
    }

    #[test]
    fn test_mk_propagate() {
        let inner = mk_var("x");
        let expr = mk_propagate(inner);
        match expr {
            Expr::Propagate { expr } => match expr.node {
                Expr::Ident(name) => assert_eq!(name, "x"),
                _ => panic!("Expected Ident inside Propagate"),
            },
            _ => panic!("Expected Propagate"),
        }
    }

    #[test]
    fn test_mk_method_call() {
        let expr = mk_method_call("obj", "method", vec![mk_int_lit(42)]);
        match expr {
            Expr::MethodCall { object, method, args } => {
                match object.node {
                    Expr::Ident(name) => assert_eq!(name, "obj"),
                    _ => panic!("Expected Ident for object"),
                }
                assert_eq!(method.node, "method");
                assert_eq!(args.len(), 1);
                match &args[0].node {
                    Expr::IntLit(v) => assert_eq!(*v, 42),
                    _ => panic!("Expected IntLit arg"),
                }
            }
            _ => panic!("Expected MethodCall"),
        }
    }

    #[test]
    fn test_mk_method_call_no_args() {
        let expr = mk_method_call("obj", "method", vec![]);
        match expr {
            Expr::MethodCall { args, .. } => assert_eq!(args.len(), 0),
            _ => panic!("Expected MethodCall"),
        }
    }

    #[test]
    fn test_mk_method_call_on_expr() {
        let inner_expr = mk_var("x");
        let expr = mk_method_call_on_expr(inner_expr, "foo", vec![mk_string_lit("test")]);
        match expr {
            Expr::MethodCall { object, method, args } => {
                match object.node {
                    Expr::Ident(name) => assert_eq!(name, "x"),
                    _ => panic!("Expected Ident for object"),
                }
                assert_eq!(method.node, "foo");
                assert_eq!(args.len(), 1);
            }
            _ => panic!("Expected MethodCall"),
        }
    }

    #[test]
    fn test_mk_return() {
        let stmt = mk_return(mk_int_lit(42));
        match stmt.node {
            Stmt::Return(Some(expr)) => match expr.node {
                Expr::IntLit(v) => assert_eq!(v, 42),
                _ => panic!("Expected IntLit in return"),
            },
            _ => panic!("Expected Return statement"),
        }
    }

    #[test]
    fn test_mk_struct_lit() {
        let fields = vec![
            ("x".to_string(), mk_int_lit(10)),
            ("y".to_string(), mk_int_lit(20)),
        ];
        let expr = mk_struct_lit("Point", fields);
        match expr {
            Expr::StructLit { name, fields, .. } => {
                assert_eq!(name.node, "Point");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].0.node, "x");
                assert_eq!(fields[1].0.node, "y");
            }
            _ => panic!("Expected StructLit"),
        }
    }

    #[test]
    fn test_mk_struct_lit_empty() {
        let expr = mk_struct_lit("Empty", vec![]);
        match expr {
            Expr::StructLit { name, fields, .. } => {
                assert_eq!(name.node, "Empty");
                assert_eq!(fields.len(), 0);
            }
            _ => panic!("Expected StructLit"),
        }
    }

    #[test]
    fn test_mk_call_function() {
        let expr = mk_call("foo", vec![mk_int_lit(1), mk_int_lit(2)]);
        match expr {
            Expr::Call { name, args, .. } => {
                assert_eq!(name.node, "foo");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected Call"),
        }
    }

    #[test]
    fn test_mk_call_method_with_dot() {
        let expr = mk_call("obj.method", vec![mk_int_lit(42)]);
        match expr {
            Expr::MethodCall { object, method, args } => {
                match object.node {
                    Expr::Ident(name) => assert_eq!(name, "obj"),
                    _ => panic!("Expected Ident for object"),
                }
                assert_eq!(method.node, "method");
                assert_eq!(args.len(), 1);
            }
            _ => panic!("Expected MethodCall"),
        }
    }

    #[test]
    fn test_mk_stmt_expr() {
        let stmt = mk_stmt_expr(mk_int_lit(42));
        match stmt.node {
            Stmt::Expr(expr) => match expr.node {
                Expr::IntLit(v) => assert_eq!(v, 42),
                _ => panic!("Expected IntLit"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    // ===== substitute_type_in_type_expr tests =====

    #[test]
    fn test_substitute_type_exact_match() {
        let ty = TypeExpr::Named("T".to_string());
        let concrete = TypeExpr::Named("int".to_string());
        let result = substitute_type_in_type_expr(&ty, "T", &concrete);
        match result {
            TypeExpr::Named(name) => assert_eq!(name, "int"),
            _ => panic!("Expected Named"),
        }
    }

    #[test]
    fn test_substitute_type_no_match() {
        let ty = TypeExpr::Named("SomeClass".to_string());
        let concrete = TypeExpr::Named("int".to_string());
        let result = substitute_type_in_type_expr(&ty, "T", &concrete);
        match result {
            TypeExpr::Named(name) => assert_eq!(name, "SomeClass"),
            _ => panic!("Expected Named"),
        }
    }

    #[test]
    fn test_substitute_type_in_array() {
        let ty = TypeExpr::Array(Box::new(Spanned {
            node: TypeExpr::Named("T".to_string()),
            span: Span { start: 0, end: 0, file_id: 0 },
        }));
        let concrete = TypeExpr::Named("string".to_string());
        let result = substitute_type_in_type_expr(&ty, "T", &concrete);
        match result {
            TypeExpr::Array(elem) => match &elem.node {
                TypeExpr::Named(name) => assert_eq!(name, "string"),
                _ => panic!("Expected Named in array"),
            },
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn test_substitute_type_in_nullable() {
        let ty = TypeExpr::Nullable(Box::new(Spanned {
            node: TypeExpr::Named("T".to_string()),
            span: Span { start: 0, end: 0, file_id: 0 },
        }));
        let concrete = TypeExpr::Named("float".to_string());
        let result = substitute_type_in_type_expr(&ty, "T", &concrete);
        match result {
            TypeExpr::Nullable(inner) => match &inner.node {
                TypeExpr::Named(name) => assert_eq!(name, "float"),
                _ => panic!("Expected Named in nullable"),
            },
            _ => panic!("Expected Nullable"),
        }
    }

    #[test]
    fn test_substitute_type_in_generic() {
        let ty = TypeExpr::Generic {
            name: "Box".to_string(),
            type_args: vec![Spanned {
                node: TypeExpr::Named("T".to_string()),
                span: Span { start: 0, end: 0, file_id: 0 },
            }],
        };
        let concrete = TypeExpr::Named("bool".to_string());
        let result = substitute_type_in_type_expr(&ty, "T", &concrete);
        match result {
            TypeExpr::Generic { name, type_args } => {
                assert_eq!(name, "Box");
                assert_eq!(type_args.len(), 1);
                match &type_args[0].node {
                    TypeExpr::Named(n) => assert_eq!(n, "bool"),
                    _ => panic!("Expected Named type arg"),
                }
            }
            _ => panic!("Expected Generic"),
        }
    }

    #[test]
    fn test_substitute_type_nested() {
        // Array<Array<T>> with T -> int
        let ty = TypeExpr::Array(Box::new(Spanned {
            node: TypeExpr::Array(Box::new(Spanned {
                node: TypeExpr::Named("T".to_string()),
                span: Span { start: 0, end: 0, file_id: 0 },
            })),
            span: Span { start: 0, end: 0, file_id: 0 },
        }));
        let concrete = TypeExpr::Named("int".to_string());
        let result = substitute_type_in_type_expr(&ty, "T", &concrete);
        match result {
            TypeExpr::Array(outer) => match &outer.node {
                TypeExpr::Array(inner) => match &inner.node {
                    TypeExpr::Named(name) => assert_eq!(name, "int"),
                    _ => panic!("Expected Named at innermost"),
                },
                _ => panic!("Expected Array at middle"),
            },
            _ => panic!("Expected Array at outermost"),
        }
    }

    // ===== collect_types_from_type_expr tests =====

    #[test]
    fn test_collect_types_primitive() {
        let mut types = HashSet::new();
        let ty = TypeExpr::Named("int".to_string());
        collect_types_from_type_expr(&ty, &mut types);
        assert_eq!(types.len(), 0); // Primitives not collected
    }

    #[test]
    fn test_collect_types_all_primitives() {
        let mut types = HashSet::new();
        for prim in &["int", "float", "bool", "string", "byte", "void"] {
            let ty = TypeExpr::Named(prim.to_string());
            collect_types_from_type_expr(&ty, &mut types);
        }
        assert_eq!(types.len(), 0); // No primitives collected
    }

    #[test]
    fn test_collect_types_class() {
        let mut types = HashSet::new();
        let ty = TypeExpr::Named("MyClass".to_string());
        collect_types_from_type_expr(&ty, &mut types);
        assert_eq!(types.len(), 1);
        assert!(types.contains("MyClass"));
    }

    #[test]
    fn test_collect_types_from_array() {
        let mut types = HashSet::new();
        let ty = TypeExpr::Array(Box::new(Spanned {
            node: TypeExpr::Named("User".to_string()),
            span: Span { start: 0, end: 0, file_id: 0 },
        }));
        collect_types_from_type_expr(&ty, &mut types);
        assert_eq!(types.len(), 1);
        assert!(types.contains("User"));
    }

    #[test]
    fn test_collect_types_from_nullable() {
        let mut types = HashSet::new();
        let ty = TypeExpr::Nullable(Box::new(Spanned {
            node: TypeExpr::Named("Product".to_string()),
            span: Span { start: 0, end: 0, file_id: 0 },
        }));
        collect_types_from_type_expr(&ty, &mut types);
        assert_eq!(types.len(), 1);
        assert!(types.contains("Product"));
    }

    #[test]
    fn test_collect_types_from_map() {
        let mut types = HashSet::new();
        let ty = TypeExpr::Generic {
            name: "Map".to_string(),
            type_args: vec![
                Spanned {
                    node: TypeExpr::Named("string".to_string()),
                    span: Span { start: 0, end: 0, file_id: 0 },
                },
                Spanned {
                    node: TypeExpr::Named("User".to_string()),
                    span: Span { start: 0, end: 0, file_id: 0 },
                },
            ],
        };
        collect_types_from_type_expr(&ty, &mut types);
        // Map itself not collected (built-in), but User is
        assert_eq!(types.len(), 1);
        assert!(types.contains("User"));
    }

    #[test]
    fn test_collect_types_from_set() {
        let mut types = HashSet::new();
        let ty = TypeExpr::Generic {
            name: "Set".to_string(),
            type_args: vec![Spanned {
                node: TypeExpr::Named("Product".to_string()),
                span: Span { start: 0, end: 0, file_id: 0 },
            }],
        };
        collect_types_from_type_expr(&ty, &mut types);
        assert_eq!(types.len(), 1);
        assert!(types.contains("Product"));
    }

    #[test]
    fn test_collect_types_from_user_generic() {
        let mut types = HashSet::new();
        let ty = TypeExpr::Generic {
            name: "Box".to_string(),
            type_args: vec![Spanned {
                node: TypeExpr::Named("int".to_string()),
                span: Span { start: 0, end: 0, file_id: 0 },
            }],
        };
        collect_types_from_type_expr(&ty, &mut types);
        // User-defined generic: collects mangled name
        assert_eq!(types.len(), 1);
        assert!(types.contains("Box$$int"));
    }

    #[test]
    fn test_collect_types_from_qualified() {
        let mut types = HashSet::new();
        let ty = TypeExpr::Qualified {
            module: "math".to_string(),
            name: "Vector".to_string(),
        };
        collect_types_from_type_expr(&ty, &mut types);
        assert_eq!(types.len(), 1);
        assert!(types.contains("math.Vector"));
    }

    #[test]
    fn test_collect_types_multiple() {
        let mut types = HashSet::new();
        
        // Collect from multiple type expressions
        let ty1 = TypeExpr::Named("User".to_string());
        collect_types_from_type_expr(&ty1, &mut types);
        
        let ty2 = TypeExpr::Named("Product".to_string());
        collect_types_from_type_expr(&ty2, &mut types);
        
        let ty3 = TypeExpr::Named("User".to_string()); // Duplicate
        collect_types_from_type_expr(&ty3, &mut types);
        
        assert_eq!(types.len(), 2); // User and Product (no duplicate)
        assert!(types.contains("User"));
        assert!(types.contains("Product"));
    }

    #[test]
    fn test_collect_types_complex_nested() {
        let mut types = HashSet::new();
        // Map<string, Array<User>>
        let ty = TypeExpr::Generic {
            name: "Map".to_string(),
            type_args: vec![
                Spanned {
                    node: TypeExpr::Named("string".to_string()),
                    span: Span { start: 0, end: 0, file_id: 0 },
                },
                Spanned {
                    node: TypeExpr::Array(Box::new(Spanned {
                        node: TypeExpr::Named("User".to_string()),
                        span: Span { start: 0, end: 0, file_id: 0 },
                    })),
                    span: Span { start: 0, end: 0, file_id: 0 },
                },
            ],
        };
        collect_types_from_type_expr(&ty, &mut types);
        assert_eq!(types.len(), 1);
        assert!(types.contains("User"));
    }

    // ===== Phase 1: value_expr_to_string tests =====

    #[test]
    fn test_value_expr_to_string_ident() {
        let expr = Expr::Ident("myVar".to_string());
        let result = value_expr_to_string(&expr).unwrap();
        assert_eq!(result, "myVar");
    }

    #[test]
    fn test_value_expr_to_string_field_access() {
        let expr = Expr::FieldAccess {
            object: Box::new(Spanned {
                node: Expr::Ident("obj".to_string()),
                span: mk_span(),
            }),
            field: Spanned {
                node: "field".to_string(),
                span: mk_span(),
            },
        };
        let result = value_expr_to_string(&expr).unwrap();
        assert_eq!(result, "obj.field");
    }

    #[test]
    fn test_value_expr_to_string_nested_field_access() {
        let expr = Expr::FieldAccess {
            object: Box::new(Spanned {
                node: Expr::FieldAccess {
                    object: Box::new(Spanned {
                        node: Expr::Ident("a".to_string()),
                        span: mk_span(),
                    }),
                    field: Spanned {
                        node: "b".to_string(),
                        span: mk_span(),
                    },
                },
                span: mk_span(),
            }),
            field: Spanned {
                node: "c".to_string(),
                span: mk_span(),
            },
        };
        let result = value_expr_to_string(&expr).unwrap();
        assert_eq!(result, "a.b.c");
    }

    #[test]
    fn test_value_expr_to_string_complex_expr_fails() {
        let expr = Expr::IntLit(42);
        let result = value_expr_to_string(&expr);
        assert!(result.is_err());
    }

    // ===== Phase 2: Generic instantiation tests =====

    #[test]
    fn test_instantiate_generic_class_single_param() {
        use crate::parser::ast::{ClassDecl, Field, Lifecycle};
        use uuid::Uuid;

        let template = ClassDecl {
            id: Uuid::new_v4(),
            name: Spanned {
                node: "Box".to_string(),
                span: mk_span(),
            },
            type_params: vec![Spanned {
                node: "T".to_string(),
                span: mk_span(),
            }],
            type_param_bounds: HashMap::new(),
            fields: vec![Field {
                id: Uuid::new_v4(),
                name: Spanned {
                    node: "value".to_string(),
                    span: mk_span(),
                },
                ty: Spanned {
                    node: TypeExpr::Named("T".to_string()),
                    span: mk_span(),
                },
                is_injected: false,
                is_ambient: false,
            }],
            methods: vec![],
            invariants: vec![],
            impl_traits: vec![],
            uses: vec![],
            is_pub: false,
            lifecycle: Lifecycle::Singleton,
        };

        let result = instantiate_generic_class(&template, "Box$$int", "int").unwrap();

        assert_eq!(result.name.node, "Box$$int");
        assert_eq!(result.type_params.len(), 0);
        assert_eq!(result.fields.len(), 1);
        match &result.fields[0].ty.node {
            TypeExpr::Named(name) => assert_eq!(name, "int"),
            _ => panic!("Expected Named type"),
        }
    }

    #[test]
    fn test_instantiate_generic_class_nested_type() {
        use crate::parser::ast::{ClassDecl, Field, Lifecycle};
        use uuid::Uuid;

        let template = ClassDecl {
            id: Uuid::new_v4(),
            name: Spanned {
                node: "Container".to_string(),
                span: mk_span(),
            },
            type_params: vec![Spanned {
                node: "T".to_string(),
                span: mk_span(),
            }],
            type_param_bounds: HashMap::new(),
            fields: vec![Field {
                id: Uuid::new_v4(),
                name: Spanned {
                    node: "items".to_string(),
                    span: mk_span(),
                },
                ty: Spanned {
                    node: TypeExpr::Array(Box::new(Spanned {
                        node: TypeExpr::Named("T".to_string()),
                        span: mk_span(),
                    })),
                    span: mk_span(),
                },
                is_injected: false,
                is_ambient: false,
            }],
            methods: vec![],
            invariants: vec![],
            impl_traits: vec![],
            uses: vec![],
            is_pub: false,
            lifecycle: Lifecycle::Singleton,
        };

        let result = instantiate_generic_class(&template, "Container$$string", "string").unwrap();

        assert_eq!(result.fields.len(), 1);
        match &result.fields[0].ty.node {
            TypeExpr::Array(elem) => match &elem.node {
                TypeExpr::Named(name) => assert_eq!(name, "string"),
                _ => panic!("Expected Named type in array"),
            },
            _ => panic!("Expected Array type"),
        }
    }

    #[test]
    fn test_instantiate_generic_enum_single_param() {
        use crate::parser::ast::{EnumDecl, EnumVariant, Field};
        use uuid::Uuid;

        let template = EnumDecl {
            id: Uuid::new_v4(),
            name: Spanned {
                node: "Option".to_string(),
                span: mk_span(),
            },
            type_params: vec![Spanned {
                node: "T".to_string(),
                span: mk_span(),
            }],
            type_param_bounds: HashMap::new(),
            variants: vec![
                EnumVariant {
                    id: Uuid::new_v4(),
                    name: Spanned {
                        node: "Some".to_string(),
                        span: mk_span(),
                    },
                    fields: vec![Field {
                        id: Uuid::new_v4(),
                        name: Spanned {
                            node: "value".to_string(),
                            span: mk_span(),
                        },
                        ty: Spanned {
                            node: TypeExpr::Named("T".to_string()),
                            span: mk_span(),
                        },
                        is_injected: false,
                        is_ambient: false,
                    }],
                },
                EnumVariant {
                    id: Uuid::new_v4(),
                    name: Spanned {
                        node: "None".to_string(),
                        span: mk_span(),
                    },
                    fields: vec![],
                },
            ],
            is_pub: false,
        };

        let result = instantiate_generic_enum(&template, "Option$$int", "int").unwrap();

        assert_eq!(result.name.node, "Option$$int");
        assert_eq!(result.type_params.len(), 0);
        assert_eq!(result.variants.len(), 2);
        assert_eq!(result.variants[0].fields.len(), 1);
        match &result.variants[0].fields[0].ty.node {
            TypeExpr::Named(name) => assert_eq!(name, "int"),
            _ => panic!("Expected Named type"),
        }
        assert_eq!(result.variants[1].fields.len(), 0);
    }

    #[test]
    fn test_instantiate_generic_enum_complex_field() {
        use crate::parser::ast::{EnumDecl, EnumVariant, Field};
        use uuid::Uuid;

        let template = EnumDecl {
            id: Uuid::new_v4(),
            name: Spanned {
                node: "Result".to_string(),
                span: mk_span(),
            },
            type_params: vec![Spanned {
                node: "T".to_string(),
                span: mk_span(),
            }],
            type_param_bounds: HashMap::new(),
            variants: vec![EnumVariant {
                id: Uuid::new_v4(),
                name: Spanned {
                    node: "Ok".to_string(),
                    span: mk_span(),
                },
                fields: vec![Field {
                    id: Uuid::new_v4(),
                    name: Spanned {
                        node: "value".to_string(),
                        span: mk_span(),
                    },
                    ty: Spanned {
                        node: TypeExpr::Nullable(Box::new(Spanned {
                            node: TypeExpr::Named("T".to_string()),
                            span: mk_span(),
                        })),
                        span: mk_span(),
                    },
                    is_injected: false,
                    is_ambient: false,
                }],
            }],
            is_pub: false,
        };

        let result = instantiate_generic_enum(&template, "Result$$bool", "bool").unwrap();

        assert_eq!(result.variants[0].fields.len(), 1);
        match &result.variants[0].fields[0].ty.node {
            TypeExpr::Nullable(inner) => match &inner.node {
                TypeExpr::Named(name) => assert_eq!(name, "bool"),
                _ => panic!("Expected Named type in nullable"),
            },
            _ => panic!("Expected Nullable type"),
        }
    }

    // ===== Phase 3: mk_encode_value tests =====

    #[test]
    fn test_mk_encode_value_int() {
        let ty = TypeExpr::Named("int".to_string());
        let expr = mk_var("x");
        let stmts = mk_encode_value(&ty, expr).unwrap();

        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::Expr(e) => match &e.node {
                Expr::MethodCall { object, method, args } => {
                    assert!(matches!(&object.node, Expr::Ident(n) if n == "enc"));
                    assert_eq!(method.node, "encode_int");
                    assert_eq!(args.len(), 1);
                }
                _ => panic!("Expected MethodCall"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_mk_encode_value_float() {
        let ty = TypeExpr::Named("float".to_string());
        let expr = mk_var("y");
        let stmts = mk_encode_value(&ty, expr).unwrap();

        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::Expr(e) => match &e.node {
                Expr::MethodCall { method, .. } => {
                    assert_eq!(method.node, "encode_float");
                }
                _ => panic!("Expected MethodCall"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_mk_encode_value_bool() {
        let ty = TypeExpr::Named("bool".to_string());
        let expr = mk_var("flag");
        let stmts = mk_encode_value(&ty, expr).unwrap();

        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::Expr(e) => match &e.node {
                Expr::MethodCall { method, .. } => {
                    assert_eq!(method.node, "encode_bool");
                }
                _ => panic!("Expected MethodCall"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_mk_encode_value_string() {
        let ty = TypeExpr::Named("string".to_string());
        let expr = mk_var("s");
        let stmts = mk_encode_value(&ty, expr).unwrap();

        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::Expr(e) => match &e.node {
                Expr::MethodCall { method, .. } => {
                    assert_eq!(method.node, "encode_string");
                }
                _ => panic!("Expected MethodCall"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_mk_encode_value_byte() {
        let ty = TypeExpr::Named("byte".to_string());
        let expr = mk_var("b");
        let stmts = mk_encode_value(&ty, expr).unwrap();

        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::Expr(e) => match &e.node {
                Expr::MethodCall { method, args, .. } => {
                    assert_eq!(method.node, "encode_int");
                    // Should cast byte to int
                    match &args[0].node {
                        Expr::Cast { target_type, .. } => {
                            assert!(matches!(&target_type.node, TypeExpr::Named(n) if n == "int"));
                        }
                        _ => panic!("Expected Cast"),
                    }
                }
                _ => panic!("Expected MethodCall"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_mk_encode_value_void() {
        let ty = TypeExpr::Named("void".to_string());
        let expr = mk_var("v");
        let stmts = mk_encode_value(&ty, expr).unwrap();

        assert_eq!(stmts.len(), 1);
        // void encoding should be a no-op (just IntLit(0))
        match &stmts[0].node {
            Stmt::Expr(_) => {} // Accept any expr statement for void
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_mk_encode_value_user_class() {
        let ty = TypeExpr::Named("MyClass".to_string());
        let expr = mk_var("obj");
        let stmts = mk_encode_value(&ty, expr).unwrap();

        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::Expr(e) => match &e.node {
                Expr::Call { name, args, .. } => {
                    assert_eq!(name.node, "__marshal_MyClass");
                    assert_eq!(args.len(), 2); // value, enc
                }
                _ => panic!("Expected Call"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_mk_encode_value_array() {
        let ty = TypeExpr::Array(Box::new(Spanned {
            node: TypeExpr::Named("int".to_string()),
            span: mk_span(),
        }));
        let expr = mk_var("arr");
        let stmts = mk_encode_value(&ty, expr).unwrap();

        // Should generate multiple statements: encode_array_start, while loop, encode_array_end
        assert!(stmts.len() > 1);

        // First statement should be encode_array_start
        match &stmts[0].node {
            Stmt::Expr(e) => match &e.node {
                Expr::MethodCall { method, .. } => {
                    assert_eq!(method.node, "encode_array_start");
                }
                _ => panic!("Expected MethodCall"),
            },
            _ => panic!("Expected Expr statement"),
        }

        // Should have a while loop
        assert!(stmts.iter().any(|s| matches!(&s.node, Stmt::While { .. })));

        // Last statement should be encode_array_end
        match &stmts[stmts.len() - 1].node {
            Stmt::Expr(e) => match &e.node {
                Expr::MethodCall { method, .. } => {
                    assert_eq!(method.node, "encode_array_end");
                }
                _ => panic!("Expected MethodCall"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_mk_encode_value_nullable() {
        let ty = TypeExpr::Nullable(Box::new(Spanned {
            node: TypeExpr::Named("int".to_string()),
            span: mk_span(),
        }));
        let expr = mk_var("maybe");
        let stmts = mk_encode_value(&ty, expr).unwrap();

        assert_eq!(stmts.len(), 1);
        // Should generate an if statement checking for none
        match &stmts[0].node {
            Stmt::If { condition, then_block, else_block } => {
                // Condition should compare to none
                match &condition.node {
                    Expr::BinOp { op, .. } => {
                        assert_eq!(*op, BinOp::Eq);
                    }
                    _ => panic!("Expected BinOp"),
                }
                // Then block should encode_null
                assert!(!then_block.node.stmts.is_empty());
                // Else block should encode the unwrapped value
                assert!(else_block.is_some());
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_mk_encode_value_map() {
        let ty = TypeExpr::Generic {
            name: "Map".to_string(),
            type_args: vec![
                Spanned {
                    node: TypeExpr::Named("string".to_string()),
                    span: mk_span(),
                },
                Spanned {
                    node: TypeExpr::Named("int".to_string()),
                    span: mk_span(),
                },
            ],
        };
        let expr = mk_var("m");
        let stmts = mk_encode_value(&ty, expr).unwrap();

        // Should generate multiple statements: encode_map_start, keys binding, while loop, encode_map_end
        assert!(stmts.len() > 1);

        // First statement should be encode_map_start
        match &stmts[0].node {
            Stmt::Expr(e) => match &e.node {
                Expr::MethodCall { method, .. } => {
                    assert_eq!(method.node, "encode_map_start");
                }
                _ => panic!("Expected MethodCall"),
            },
            _ => panic!("Expected Expr statement"),
        }

        // Should have a while loop
        assert!(stmts.iter().any(|s| matches!(&s.node, Stmt::While { .. })));

        // Last statement should be encode_map_end
        match &stmts[stmts.len() - 1].node {
            Stmt::Expr(e) => match &e.node {
                Expr::MethodCall { method, .. } => {
                    assert_eq!(method.node, "encode_map_end");
                }
                _ => panic!("Expected MethodCall"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_mk_encode_value_set() {
        let ty = TypeExpr::Generic {
            name: "Set".to_string(),
            type_args: vec![Spanned {
                node: TypeExpr::Named("string".to_string()),
                span: mk_span(),
            }],
        };
        let expr = mk_var("s");
        let stmts = mk_encode_value(&ty, expr).unwrap();

        // Should generate multiple statements (encode as array)
        assert!(stmts.len() > 1);

        // First statement should be encode_array_start
        match &stmts[0].node {
            Stmt::Expr(e) => match &e.node {
                Expr::MethodCall { method, .. } => {
                    assert_eq!(method.node, "encode_array_start");
                }
                _ => panic!("Expected MethodCall"),
            },
            _ => panic!("Expected Expr statement"),
        }

        // Last statement should be encode_array_end
        match &stmts[stmts.len() - 1].node {
            Stmt::Expr(e) => match &e.node {
                Expr::MethodCall { method, .. } => {
                    assert_eq!(method.node, "encode_array_end");
                }
                _ => panic!("Expected MethodCall"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_mk_encode_value_user_generic() {
        let ty = TypeExpr::Generic {
            name: "Box".to_string(),
            type_args: vec![Spanned {
                node: TypeExpr::Named("int".to_string()),
                span: mk_span(),
            }],
        };
        let expr = mk_var("box");
        let stmts = mk_encode_value(&ty, expr).unwrap();

        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::Expr(e) => match &e.node {
                Expr::Call { name, args, .. } => {
                    // Should call __marshal_Box$$int
                    assert_eq!(name.node, "__marshal_Box$$int");
                    assert_eq!(args.len(), 2); // value, enc
                }
                _ => panic!("Expected Call"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    // ===== Phase 4: mk_let_decode tests =====

    #[test]
    fn test_mk_let_decode_int() {
        let ty = TypeExpr::Named("int".to_string());
        let stmts = mk_let_decode("x", &ty).unwrap();

        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::Let { name, ty: Some(ty_expr), value, .. } => {
                assert_eq!(name.node, "x");
                assert!(matches!(&ty_expr.node, TypeExpr::Named(n) if n == "int"));
                // Value should be propagate(decode_int)
                match &value.node {
                    Expr::Propagate { expr } => match &expr.node {
                        Expr::MethodCall { method, .. } => {
                            assert_eq!(method.node, "decode_int");
                        }
                        _ => panic!("Expected MethodCall inside Propagate"),
                    },
                    _ => panic!("Expected Propagate"),
                }
            }
            _ => panic!("Expected Let statement"),
        }
    }

    #[test]
    fn test_mk_let_decode_float() {
        let ty = TypeExpr::Named("float".to_string());
        let stmts = mk_let_decode("y", &ty).unwrap();

        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::Let { value, .. } => match &value.node {
                Expr::Propagate { expr } => match &expr.node {
                    Expr::MethodCall { method, .. } => {
                        assert_eq!(method.node, "decode_float");
                    }
                    _ => panic!("Expected MethodCall"),
                },
                _ => panic!("Expected Propagate"),
            },
            _ => panic!("Expected Let statement"),
        }
    }

    #[test]
    fn test_mk_let_decode_bool() {
        let ty = TypeExpr::Named("bool".to_string());
        let stmts = mk_let_decode("flag", &ty).unwrap();

        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::Let { value, .. } => match &value.node {
                Expr::Propagate { expr } => match &expr.node {
                    Expr::MethodCall { method, .. } => {
                        assert_eq!(method.node, "decode_bool");
                    }
                    _ => panic!("Expected MethodCall"),
                },
                _ => panic!("Expected Propagate"),
            },
            _ => panic!("Expected Let statement"),
        }
    }

    #[test]
    fn test_mk_let_decode_string() {
        let ty = TypeExpr::Named("string".to_string());
        let stmts = mk_let_decode("s", &ty).unwrap();

        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::Let { value, .. } => match &value.node {
                Expr::Propagate { expr } => match &expr.node {
                    Expr::MethodCall { method, .. } => {
                        assert_eq!(method.node, "decode_string");
                    }
                    _ => panic!("Expected MethodCall"),
                },
                _ => panic!("Expected Propagate"),
            },
            _ => panic!("Expected Let statement"),
        }
    }

    #[test]
    fn test_mk_let_decode_byte() {
        let ty = TypeExpr::Named("byte".to_string());
        let stmts = mk_let_decode("b", &ty).unwrap();

        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::Let { value, .. } => match &value.node {
                Expr::Cast { target_type, .. } => {
                    // Should cast decode_int() to byte
                    assert!(matches!(&target_type.node, TypeExpr::Named(n) if n == "byte"));
                }
                _ => panic!("Expected Cast"),
            },
            _ => panic!("Expected Let statement"),
        }
    }

    #[test]
    fn test_mk_let_decode_void() {
        let ty = TypeExpr::Named("void".to_string());
        let stmts = mk_let_decode("v", &ty).unwrap();

        assert_eq!(stmts.len(), 1);
        // void decoding should be a no-op
        match &stmts[0].node {
            Stmt::Expr(_) => {} // Accept any expr statement for void
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_mk_let_decode_user_class() {
        let ty = TypeExpr::Named("MyClass".to_string());
        let stmts = mk_let_decode("obj", &ty).unwrap();

        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::Let { value, .. } => match &value.node {
                Expr::Propagate { expr } => match &expr.node {
                    Expr::Call { name, .. } => {
                        assert_eq!(name.node, "__unmarshal_MyClass");
                    }
                    _ => panic!("Expected Call"),
                },
                _ => panic!("Expected Propagate"),
            },
            _ => panic!("Expected Let statement"),
        }
    }

    #[test]
    fn test_mk_let_decode_array() {
        let ty = TypeExpr::Array(Box::new(Spanned {
            node: TypeExpr::Named("int".to_string()),
            span: mk_span(),
        }));
        let stmts = mk_let_decode("arr", &ty).unwrap();

        // Should generate multiple statements: decode_array_start, empty array, while loop, final let
        assert!(stmts.len() > 1);

        // Should have decode_array_start
        assert!(stmts.iter().any(|s| {
            matches!(&s.node, Stmt::Let { value, .. } if matches!(&value.node, Expr::Propagate { .. }))
        }));

        // Should have a while loop
        assert!(stmts.iter().any(|s| matches!(&s.node, Stmt::While { .. })));

        // Final statement should be the let binding for arr
        match &stmts[stmts.len() - 1].node {
            Stmt::Let { name, .. } => {
                assert_eq!(name.node, "arr");
            }
            _ => panic!("Expected Let statement at end"),
        }
    }

    #[test]
    fn test_mk_let_decode_nullable() {
        let ty = TypeExpr::Nullable(Box::new(Spanned {
            node: TypeExpr::Named("int".to_string()),
            span: mk_span(),
        }));
        let stmts = mk_let_decode("maybe", &ty).unwrap();

        // Should generate: let __is_null = ..., if statement, final let
        assert!(stmts.len() > 1);

        // Should have an if statement
        assert!(stmts.iter().any(|s| matches!(&s.node, Stmt::If { .. })));

        // Final statement should be the let binding for maybe
        match &stmts[stmts.len() - 1].node {
            Stmt::Let { name, .. } => {
                assert_eq!(name.node, "maybe");
            }
            _ => panic!("Expected Let statement at end"),
        }
    }

    #[test]
    fn test_mk_let_decode_map() {
        let ty = TypeExpr::Generic {
            name: "Map".to_string(),
            type_args: vec![
                Spanned {
                    node: TypeExpr::Named("string".to_string()),
                    span: mk_span(),
                },
                Spanned {
                    node: TypeExpr::Named("int".to_string()),
                    span: mk_span(),
                },
            ],
        };
        let stmts = mk_let_decode("m", &ty).unwrap();

        // Should generate multiple statements: decode_map_start, empty map, while loop, final let
        assert!(stmts.len() > 1);

        // Should have a while loop
        assert!(stmts.iter().any(|s| matches!(&s.node, Stmt::While { .. })));

        // Final statement should be the let binding for m
        match &stmts[stmts.len() - 1].node {
            Stmt::Let { name, .. } => {
                assert_eq!(name.node, "m");
            }
            _ => panic!("Expected Let statement at end"),
        }
    }

    #[test]
    fn test_mk_let_decode_set() {
        let ty = TypeExpr::Generic {
            name: "Set".to_string(),
            type_args: vec![Spanned {
                node: TypeExpr::Named("string".to_string()),
                span: mk_span(),
            }],
        };
        let stmts = mk_let_decode("s", &ty).unwrap();

        // Should generate multiple statements (decode as array, build set)
        assert!(stmts.len() > 1);

        // Should have a while loop
        assert!(stmts.iter().any(|s| matches!(&s.node, Stmt::While { .. })));

        // Final statement should be the let binding for s
        match &stmts[stmts.len() - 1].node {
            Stmt::Let { name, .. } => {
                assert_eq!(name.node, "s");
            }
            _ => panic!("Expected Let statement at end"),
        }
    }

    #[test]
    fn test_mk_let_decode_user_generic() {
        let ty = TypeExpr::Generic {
            name: "Box".to_string(),
            type_args: vec![Spanned {
                node: TypeExpr::Named("int".to_string()),
                span: mk_span(),
            }],
        };
        let stmts = mk_let_decode("box", &ty).unwrap();

        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::Let { value, .. } => match &value.node {
                // Generic user types do NOT wrap in Propagate (unlike Named user types)
                Expr::Call { name, .. } => {
                    // Should call __unmarshal_Box$$int
                    assert_eq!(name.node, "__unmarshal_Box$$int");
                }
                _ => panic!("Expected Call (not Propagate) for generic user types"),
            },
            _ => panic!("Expected Let statement"),
        }
    }
}
