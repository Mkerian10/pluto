use std::collections::{HashMap, HashSet};
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

    // Skip marshaler generation if std.wire isn't imported (validation will still run)
    let has_wire_import = program.imports.iter().any(|imp| {
        let path_str = imp.node.path.iter().map(|s| s.node.as_str()).collect::<Vec<_>>().join(".");
        path_str == "wire" || path_str == "std.wire"
    });
    if !has_wire_import {
        return Ok(());
    }

    // Collect all types that need marshalers
    let types_to_marshal = collect_types_from_stage_methods(program)?;

    // Generate marshal and unmarshal functions for each type
    let mut generated_functions = Vec::new();

    for type_name in &types_to_marshal {
        // Check if it's a class or enum
        if let Some(class_decl) = program.classes.iter().find(|c| &c.node.name.node == type_name) {
            // Skip generic classes (they'll be handled in phase B)
            if !class_decl.node.type_params.is_empty() {
                continue;
            }

            generated_functions.push(generate_marshal_class(&class_decl.node)?);
            generated_functions.push(generate_unmarshal_class(&class_decl.node)?);
        } else if let Some(enum_decl) = program.enums.iter().find(|e| &e.node.name.node == type_name) {
            // Skip generic enums (they'll be handled in phase B)
            if !enum_decl.node.type_params.is_empty() {
                continue;
            }

            generated_functions.push(generate_marshal_enum(&enum_decl.node)?);
            generated_functions.push(generate_unmarshal_enum(&enum_decl.node)?);
        }
    }

    // Inject generated functions into the program
    program.functions.extend(generated_functions);

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

    // Skip marshaler generation if std.wire isn't imported (validation will still run)
    let has_wire_import = program.imports.iter().any(|imp| {
        let path_str = imp.node.path.iter().map(|s| s.node.as_str()).collect::<Vec<_>>().join(".");
        path_str == "wire" || path_str == "std.wire"
    });
    if !has_wire_import {
        return Ok(());
    }

    // Collect types from stage methods (now includes monomorphized names like Box__int)
    let types_to_marshal = collect_types_from_stage_methods(program)?;

    let mut generated_functions = Vec::new();

    for type_name in &types_to_marshal {
        // Check if it's a monomorphized type (contains __) that doesn't have a marshaler yet
        if type_name.contains("__") {
            let has_marshal = program.functions.iter()
                .any(|f| f.node.name.node == format!("__marshal_{}", type_name));

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
fn collect_types_from_stage_methods(program: &Program) -> Result<HashSet<String>, CompileError> {
    let mut types = HashSet::new();

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

    Ok(types)
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
                    // User-defined generic (will be monomorphized)
                    // Don't collect generic template names, only instantiations
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
    let fn_name = format!("__marshal_{}", class_name);

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
                    node: TypeExpr::Named("wire.Encoder".to_string()),
                    span: Span { start: 0, end: 0, file_id: 0 },
                },
                is_mut: true,
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
    let fn_name = format!("__unmarshal_{}", class_name);

    let data_fields: Vec<_> = class_decl.fields.iter()
        .filter(|f| !f.is_injected)
        .collect();

    let num_fields = data_fields.len();

    let mut stmts = Vec::new();

    // dec.decode_record_start("ClassName", num_fields)
    stmts.push(mk_stmt_expr(mk_call(
        "dec.decode_record_start",
        vec![
            mk_string_lit(class_name),
            mk_int_lit(num_fields as i64),
        ],
    )));

    // For each field: decode_field + decode the value
    for (index, field) in data_fields.iter().enumerate() {
        // dec.decode_field("field_name", index)
        stmts.push(mk_stmt_expr(mk_call(
            "dec.decode_field",
            vec![
                mk_string_lit(&field.name.node),
                mk_int_lit(index as i64),
            ],
        )));

        // let field_name = decode_type(dec)
        stmts.extend(mk_let_decode(&field.name.node, &field.ty.node)?);
    }

    // dec.decode_record_end()
    stmts.push(mk_stmt_expr(mk_propagate(mk_method_call("dec", "decode_record_end", vec![]))));

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
                node: TypeExpr::Named("wire.Decoder".to_string()),
                span: Span { start: 0, end: 0, file_id: 0 },
            },
            is_mut: true,
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
    let fn_name = format!("__marshal_{}", enum_name);

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
                    node: TypeExpr::Named("wire.Encoder".to_string()),
                    span: mk_span(),
                },
                is_mut: true,
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
    let fn_name = format!("__unmarshal_{}", enum_name);

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
            variant_stmts.push(mk_stmt_expr(mk_call(
                "dec.decode_field",
                vec![
                    mk_string_lit(&field.name.node),
                    mk_int_lit(field_index as i64),
                ],
            )));

            // let field_name = decode_type(dec)!
            variant_stmts.extend(mk_let_decode(&field.name.node, &field.ty.node)?);
        }

        // dec.decode_variant_end()!
        variant_stmts.push(mk_stmt_expr(mk_propagate(mk_method_call("dec", "decode_variant_end", vec![]))));

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
                node: TypeExpr::Named("wire.Decoder".to_string()),
                span: mk_span(),
            },
            is_mut: true,
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

fn mk_span() -> Span {
    Span { start: 0, end: 0, file_id: 0 }
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
                            is_mut: true,
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
                            is_mut: true,
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
    result.push_str("__");
    for (i, arg) in type_args.iter().enumerate() {
        if i > 0 {
            result.push('_');
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

            // dec.decode_array_end()!
            stmts.push(mk_stmt_expr(mk_propagate(mk_method_call("dec", "decode_array_end", vec![]))));

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

            // let __is_present = dec.decode_nullable()!
            stmts.push(Spanned {
                node: Stmt::Let {
                    name: Spanned { node: "__is_present".to_string(), span: mk_span() },
                    ty: None,
                    value: Spanned {
                        node: mk_propagate(mk_method_call("dec", "decode_nullable", vec![])),
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
                                node: mk_call(&format!("__unmarshal_{}", name), vec![mk_var("dec")]),
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
                                node: mk_propagate(mk_method_call("dec", "decode_map_start", vec![])),
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

                    // dec.decode_map_end()!
                    stmts.push(mk_stmt_expr(mk_propagate(mk_method_call("dec", "decode_map_end", vec![]))));

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
                                node: mk_propagate(mk_method_call("dec", "decode_array_start", vec![])),
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

                    // dec.decode_array_end()!
                    stmts.push(mk_stmt_expr(mk_propagate(mk_method_call("dec", "decode_array_end", vec![]))));

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
