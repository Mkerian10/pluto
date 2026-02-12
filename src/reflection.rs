/// Reflection intrinsics - generates TypeInfo implementations for all types
use crate::diagnostics::CompileError;
use crate::parser::ast::{Block, Expr, Function, Program, Stmt, TypeExpr};
use crate::span::{Span, Spanned};
use crate::typeck::env::TypeEnv;
use std::collections::HashMap;
use uuid::Uuid;

/// Synthetic span for generated code
fn synthetic_span() -> Span {
    Span { start: 0, end: 0, file_id: 0 }
}

/// Generate TypeInfo implementations for all types in the program.
/// This creates `TypeInfo_type_name__T()` and `TypeInfo_kind__T()` functions
/// for each type T (classes, enums, primitives).
pub fn generate_type_info_impls(program: &mut Program, env: &TypeEnv) -> Result<(), CompileError> {
    let mut generated_functions = Vec::new();

    // Generate for each class
    for class_name in env.classes.keys() {
        generated_functions.push(generate_type_name_impl(class_name)?);
        generated_functions.push(generate_kind_impl_for_class(class_name, env)?);
    }

    // Generate for each enum
    for enum_name in env.enums.keys() {
        generated_functions.push(generate_type_name_impl(enum_name)?);
        generated_functions.push(generate_kind_impl_for_enum(enum_name, env)?);
    }

    // TODO: Generate for primitives (int, float, bool, string, etc.)

    // Add generated functions to the program
    program.functions.extend(generated_functions);

    Ok(())
}

/// Generate TypeInfo_type_name_T() function that returns the type name as a string
fn generate_type_name_impl(type_name: &str) -> Result<Spanned<Function>, CompileError> {
    let func_name = format!("TypeInfo_type_name_{}", type_name);

    let body = Spanned {
        node: Block {
            stmts: vec![
                Spanned {
                    node: Stmt::Return(Some(Spanned {
                        node: Expr::StringLit(type_name.to_string()),
                        span: synthetic_span(),
                    })),
                    span: synthetic_span(),
                }
            ],
        },
        span: synthetic_span(),
    };

    let function = Function {
        id: Uuid::new_v4(),
        name: Spanned {
            node: func_name,
            span: synthetic_span(),
        },
        type_params: vec![],
        type_param_bounds: HashMap::new(),
        params: vec![],
        return_type: Some(Spanned {
            node: TypeExpr::Named("string".to_string()),
            span: synthetic_span(),
        }),
        contracts: vec![],
        body,
        is_pub: false,
        is_override: false,
        is_generator: false,
    };

    Ok(Spanned {
        node: function,
        span: synthetic_span(),
    })
}

/// Generate TypeInfo_kind_T() function for a class
fn generate_kind_impl_for_class(class_name: &str, env: &TypeEnv) -> Result<Spanned<Function>, CompileError> {
    let func_name = format!("TypeInfo_kind_{}", class_name);

    let class_info = env.classes.get(class_name).ok_or_else(|| {
        CompileError::codegen(format!("class '{}' not found during reflection generation", class_name))
    })?;

    // Build FieldInfo struct literals for each field
    let mut field_info_exprs = Vec::new();
    for (field_name, field_type, _is_injected) in &class_info.fields {
        let field_info = Expr::StructLit {
            name: Spanned {
                node: "FieldInfo".to_string(),
                span: synthetic_span(),
            },
            type_args: vec![],
            fields: vec![
                (
                    Spanned { node: "name".to_string(), span: synthetic_span() },
                    Spanned { node: Expr::StringLit(field_name.clone()), span: synthetic_span() },
                ),
                (
                    Spanned { node: "type_name".to_string(), span: synthetic_span() },
                    Spanned { node: Expr::StringLit(format!("{:?}", field_type)), span: synthetic_span() },
                ),
                (
                    Spanned { node: "offset".to_string(), span: synthetic_span() },
                    Spanned { node: Expr::IntLit(0), span: synthetic_span() },
                ),
            ],
            target_id: None,
        };
        field_info_exprs.push(Spanned {
            node: field_info,
            span: synthetic_span(),
        });
    }

    // Build ClassInfo struct literal
    let class_info_expr = Expr::StructLit {
        name: Spanned {
            node: "ClassInfo".to_string(),
            span: synthetic_span(),
        },
        type_args: vec![],
        fields: vec![
            (
                Spanned { node: "name".to_string(), span: synthetic_span() },
                Spanned { node: Expr::StringLit(class_name.to_string()), span: synthetic_span() },
            ),
            (
                Spanned { node: "fields".to_string(), span: synthetic_span() },
                Spanned { node: Expr::ArrayLit { elements: field_info_exprs }, span: synthetic_span() },
            ),
        ],
        target_id: None,
    };

    // Build TypeKind.Class { info: class_info }
    let type_kind_expr = Expr::EnumData {
        enum_name: Spanned {
            node: "TypeKind".to_string(),
            span: synthetic_span(),
        },
        variant: Spanned {
            node: "Class".to_string(),
            span: synthetic_span(),
        },
        type_args: vec![],
        fields: vec![
            (
                Spanned { node: "info".to_string(), span: synthetic_span() },
                Spanned { node: class_info_expr, span: synthetic_span() },
            )
        ],
        enum_id: None,
        variant_id: None,
    };

    let body = Spanned {
        node: Block {
            stmts: vec![
                Spanned {
                    node: Stmt::Return(Some(Spanned {
                        node: type_kind_expr,
                        span: synthetic_span(),
                    })),
                    span: synthetic_span(),
                }
            ],
        },
        span: synthetic_span(),
    };

    let function = Function {
        id: Uuid::new_v4(),
        name: Spanned {
            node: func_name,
            span: synthetic_span(),
        },
        type_params: vec![],
        type_param_bounds: HashMap::new(),
        params: vec![],
        return_type: Some(Spanned {
            node: TypeExpr::Named("TypeKind".to_string()),
            span: synthetic_span(),
        }),
        contracts: vec![],
        body,
        is_pub: false,
        is_override: false,
        is_generator: false,
    };

    Ok(Spanned {
        node: function,
        span: synthetic_span(),
    })
}

/// Generate TypeInfo_kind_T() function for an enum
fn generate_kind_impl_for_enum(enum_name: &str, env: &TypeEnv) -> Result<Spanned<Function>, CompileError> {
    let func_name = format!("TypeInfo_kind_{}", enum_name);

    let enum_info = env.enums.get(enum_name).ok_or_else(|| {
        CompileError::codegen(format!("enum '{}' not found during reflection generation", enum_name))
    })?;

    // Build VariantInfo struct literals for each variant
    let mut variant_info_exprs = Vec::new();
    for (variant_name, fields) in &enum_info.variants {
        // Build FieldInfo array for this variant's fields
        let mut field_info_exprs = Vec::new();
        for (field_name, field_type) in fields {
            let field_info = Expr::StructLit {
                name: Spanned {
                    node: "FieldInfo".to_string(),
                    span: synthetic_span(),
                },
                type_args: vec![],
                fields: vec![
                    (
                        Spanned { node: "name".to_string(), span: synthetic_span() },
                        Spanned { node: Expr::StringLit(field_name.clone()), span: synthetic_span() },
                    ),
                    (
                        Spanned { node: "type_name".to_string(), span: synthetic_span() },
                        Spanned { node: Expr::StringLit(format!("{:?}", field_type)), span: synthetic_span() },
                    ),
                    (
                        Spanned { node: "offset".to_string(), span: synthetic_span() },
                        Spanned { node: Expr::IntLit(0), span: synthetic_span() },
                    ),
                ],
                target_id: None,
            };
            field_info_exprs.push(Spanned {
                node: field_info,
                span: synthetic_span(),
            });
        }

        // Build VariantInfo struct literal
        let variant_info = Expr::StructLit {
            name: Spanned {
                node: "VariantInfo".to_string(),
                span: synthetic_span(),
            },
            type_args: vec![],
            fields: vec![
                (
                    Spanned { node: "name".to_string(), span: synthetic_span() },
                    Spanned { node: Expr::StringLit(variant_name.clone()), span: synthetic_span() },
                ),
                (
                    Spanned { node: "fields".to_string(), span: synthetic_span() },
                    Spanned { node: Expr::ArrayLit { elements: field_info_exprs }, span: synthetic_span() },
                ),
            ],
            target_id: None,
        };
        variant_info_exprs.push(Spanned {
            node: variant_info,
            span: synthetic_span(),
        });
    }

    // Build EnumInfo struct literal
    let enum_info_expr = Expr::StructLit {
        name: Spanned {
            node: "EnumInfo".to_string(),
            span: synthetic_span(),
        },
        type_args: vec![],
        fields: vec![
            (
                Spanned { node: "name".to_string(), span: synthetic_span() },
                Spanned { node: Expr::StringLit(enum_name.to_string()), span: synthetic_span() },
            ),
            (
                Spanned { node: "variants".to_string(), span: synthetic_span() },
                Spanned { node: Expr::ArrayLit { elements: variant_info_exprs }, span: synthetic_span() },
            ),
        ],
        target_id: None,
    };

    // Build TypeKind.Enum { info: enum_info }
    let type_kind_expr = Expr::EnumData {
        enum_name: Spanned {
            node: "TypeKind".to_string(),
            span: synthetic_span(),
        },
        variant: Spanned {
            node: "Enum".to_string(),
            span: synthetic_span(),
        },
        type_args: vec![],
        fields: vec![
            (
                Spanned { node: "info".to_string(), span: synthetic_span() },
                Spanned { node: enum_info_expr, span: synthetic_span() },
            )
        ],
        enum_id: None,
        variant_id: None,
    };

    let body = Spanned {
        node: Block {
            stmts: vec![
                Spanned {
                    node: Stmt::Return(Some(Spanned {
                        node: type_kind_expr,
                        span: synthetic_span(),
                    })),
                    span: synthetic_span(),
                }
            ],
        },
        span: synthetic_span(),
    };

    let function = Function {
        id: Uuid::new_v4(),
        name: Spanned {
            node: func_name,
            span: synthetic_span(),
        },
        type_params: vec![],
        type_param_bounds: HashMap::new(),
        params: vec![],
        return_type: Some(Spanned {
            node: TypeExpr::Named("TypeKind".to_string()),
            span: synthetic_span(),
        }),
        contracts: vec![],
        body,
        is_pub: false,
        is_override: false,
        is_generator: false,
    };

    Ok(Spanned {
        node: function,
        span: synthetic_span(),
    })
}
