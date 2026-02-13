use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::{Span, Spanned};

/// Validate that every contract in a list is within the decidable fragment.
fn validate_contract_list(contracts: &[Spanned<ContractClause>]) -> Result<(), CompileError> {
    for contract in contracts {
        validate_decidable_fragment(&contract.node.expr.node, contract.node.expr.span, contract.node.kind)?;
    }
    Ok(())
}

/// Validate that all contract expressions in the program are within the decidable fragment.
/// Called after parsing, before typeck.
pub fn validate_contracts(program: &Program) -> Result<(), CompileError> {
    for class in &program.classes {
        validate_contract_list(&class.node.invariants)?;
        for method in &class.node.methods {
            validate_contract_list(&method.node.contracts)?;
        }
    }
    for func in &program.functions {
        validate_contract_list(&func.node.contracts)?;
    }
    if let Some(app) = &program.app {
        for method in &app.node.methods {
            validate_contract_list(&method.node.contracts)?;
        }
    }
    for stage in &program.stages {
        for method in &stage.node.methods {
            validate_contract_list(&method.node.contracts)?;
        }
    }
    for tr in &program.traits {
        for method in &tr.node.methods {
            validate_contract_list(&method.contracts)?;
        }
    }
    Ok(())
}

/// Validate that an expression is within the decidable fragment allowed in contracts.
///
/// Allowed:
/// - Comparisons, arithmetic, logical, bitwise operators
/// - Unary Not, Neg, BitNot
/// - Identifiers (parameter/field names, plus `result` in ensures)
/// - Field access (self.field, nested)
/// - Method call `.len()` only (no args)
/// - Int, float, bool literals
/// - `old(expr)` in ensures clauses only
///
/// Rejected:
/// - Function calls (except old() in ensures), string literals, string interpolation
/// - Struct/array/map/set/enum literals
/// - Closures, spawn, cast, index, range
/// - Catch, propagate
/// - Any method call other than `.len()`
fn validate_decidable_fragment(expr: &Expr, span: Span, kind: ContractKind) -> Result<(), CompileError> {
    match expr {
        // Literals — allowed
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) => Ok(()),

        // Identifiers — allowed
        Expr::Ident(_) => Ok(()),

        // Binary operators — allowed (recurse into operands)
        Expr::BinOp { lhs, rhs, .. } => {
            validate_decidable_fragment(&lhs.node, lhs.span, kind)?;
            validate_decidable_fragment(&rhs.node, rhs.span, kind)
        }

        // Unary operators — allowed (recurse into operand)
        Expr::UnaryOp { operand, .. } => {
            validate_decidable_fragment(&operand.node, operand.span, kind)
        }

        // Field access — allowed (recurse into object)
        Expr::FieldAccess { object, .. } => {
            validate_decidable_fragment(&object.node, object.span, kind)
        }

        // Method call — only .len() with no args
        Expr::MethodCall { object, method, args } => {
            if method.node == "len" && args.is_empty() {
                validate_decidable_fragment(&object.node, object.span, kind)
            } else {
                Err(CompileError::syntax(
                    format!(
                        "method call '.{}()' is not allowed in contract expressions (only '.len()' is permitted)",
                        method.node
                    ),
                    span,
                ))
            }
        }

        // Function calls — only old(expr) in ensures clauses
        Expr::Call { name, args, .. } => {
            if name.node == "old" && args.len() == 1 {
                if kind == ContractKind::Ensures {
                    validate_decidable_fragment(&args[0].node, args[0].span, kind)
                } else {
                    Err(CompileError::syntax(
                        "old() is only allowed in ensures clauses".to_string(),
                        span,
                    ))
                }
            } else {
                Err(CompileError::syntax(
                    format!("function call '{}()' is not allowed in contract expressions", name.node),
                    span,
                ))
            }
        }

        // None literal — allowed (useful for nullable comparisons in contracts)
        Expr::NoneLit => Ok(()),

        // Static trait calls — rejected (might not be pure)
        Expr::StaticTraitCall { .. } => Err(CompileError::syntax(
            "static trait calls are not allowed in contract expressions",
            span,
        )),

        // Null propagation — rejected (side-effectful)
        Expr::NullPropagate { .. } => Err(CompileError::syntax(
            "null propagation is not allowed in contract expressions",
            span,
        )),

        // Everything else — rejected
        Expr::StringLit(_) => Err(CompileError::syntax(
            "string literals are not allowed in contract expressions",
            span,
        )),
        Expr::StringInterp { .. } => Err(CompileError::syntax(
            "string interpolation is not allowed in contract expressions",
            span,
        )),
        Expr::StructLit { .. } => Err(CompileError::syntax(
            "struct literals are not allowed in contract expressions",
            span,
        )),
        Expr::ArrayLit { .. } => Err(CompileError::syntax(
            "array literals are not allowed in contract expressions",
            span,
        )),
        Expr::MapLit { .. } => Err(CompileError::syntax(
            "map literals are not allowed in contract expressions",
            span,
        )),
        Expr::SetLit { .. } => Err(CompileError::syntax(
            "set literals are not allowed in contract expressions",
            span,
        )),
        Expr::Closure { .. } | Expr::ClosureCreate { .. } => Err(CompileError::syntax(
            "closures are not allowed in contract expressions",
            span,
        )),
        Expr::Spawn { .. } => Err(CompileError::syntax(
            "spawn is not allowed in contract expressions",
            span,
        )),
        Expr::Cast { .. } => Err(CompileError::syntax(
            "type casts are not allowed in contract expressions",
            span,
        )),
        Expr::Index { .. } => Err(CompileError::syntax(
            "index expressions are not allowed in contract expressions",
            span,
        )),
        Expr::Range { .. } => Err(CompileError::syntax(
            "range expressions are not allowed in contract expressions",
            span,
        )),
        Expr::Propagate { .. } => Err(CompileError::syntax(
            "error propagation is not allowed in contract expressions",
            span,
        )),
        Expr::Catch { .. } => Err(CompileError::syntax(
            "catch expressions are not allowed in contract expressions",
            span,
        )),
        Expr::EnumUnit { .. } | Expr::EnumData { .. } => Err(CompileError::syntax(
            "enum expressions are not allowed in contract expressions",
            span,
        )),
        Expr::If { .. } => Err(CompileError::syntax(
            "if expressions are not allowed in contract expressions",
            span,
        )),
        Expr::Match { .. } => Err(CompileError::syntax(
            "match expressions are not allowed in contract expressions",
            span,
        )),
        Expr::QualifiedAccess { segments } => {
            panic!(
                "QualifiedAccess should be resolved by module flattening before contracts. Segments: {:?}",
                segments.iter().map(|s| &s.node).collect::<Vec<_>>()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_span() -> Span {
        Span::new(0, 0)
    }

    fn spanned<T>(node: T) -> Spanned<T> {
        Spanned::new(node, dummy_span())
    }

    #[test]
    fn validate_int_literal() {
        let expr = Expr::IntLit(42);
        assert!(validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant).is_ok());
    }

    #[test]
    fn validate_float_literal() {
        let expr = Expr::FloatLit(3.14);
        assert!(validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant).is_ok());
    }

    #[test]
    fn validate_bool_literal() {
        let expr = Expr::BoolLit(true);
        assert!(validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant).is_ok());
    }

    #[test]
    fn validate_none_literal() {
        let expr = Expr::NoneLit;
        assert!(validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant).is_ok());
    }

    #[test]
    fn validate_identifier() {
        let expr = Expr::Ident("x".to_string());
        assert!(validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant).is_ok());
    }

    #[test]
    fn validate_binary_op() {
        let expr = Expr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(spanned(Expr::IntLit(1))),
            rhs: Box::new(spanned(Expr::IntLit(2))),
        };
        assert!(validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant).is_ok());
    }

    #[test]
    fn validate_comparison() {
        let expr = Expr::BinOp {
            op: BinOp::Lt,
            lhs: Box::new(spanned(Expr::Ident("x".to_string()))),
            rhs: Box::new(spanned(Expr::IntLit(10))),
        };
        assert!(validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant).is_ok());
    }

    #[test]
    fn validate_unary_op() {
        let expr = Expr::UnaryOp {
            op: UnaryOp::Neg,
            operand: Box::new(spanned(Expr::IntLit(5))),
        };
        assert!(validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant).is_ok());
    }

    #[test]
    fn validate_field_access() {
        let expr = Expr::FieldAccess {
            object: Box::new(spanned(Expr::Ident("self".to_string()))),
            field: spanned("value".to_string()),
        };
        assert!(validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant).is_ok());
    }

    #[test]
    fn validate_nested_field_access() {
        let expr = Expr::FieldAccess {
            object: Box::new(spanned(Expr::FieldAccess {
                object: Box::new(spanned(Expr::Ident("self".to_string()))),
                field: spanned("child".to_string()),
            })),
            field: spanned("value".to_string()),
        };
        assert!(validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant).is_ok());
    }

    #[test]
    fn validate_len_method() {
        let expr = Expr::MethodCall {
            object: Box::new(spanned(Expr::Ident("items".to_string()))),
            method: spanned("len".to_string()),
            args: vec![],
        };
        assert!(validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant).is_ok());
    }

    #[test]
    fn reject_method_with_args() {
        let expr = Expr::MethodCall {
            object: Box::new(spanned(Expr::Ident("x".to_string()))),
            method: spanned("foo".to_string()),
            args: vec![spanned(Expr::IntLit(1))],
        };
        assert!(validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant).is_err());
    }

    #[test]
    fn reject_non_len_method() {
        let expr = Expr::MethodCall {
            object: Box::new(spanned(Expr::Ident("x".to_string()))),
            method: spanned("foo".to_string()),
            args: vec![],
        };
        let result = validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("method call '.foo()' is not allowed"));
    }

    #[test]
    fn validate_old_in_ensures() {
        let expr = Expr::Call {
            name: spanned("old".to_string()),
            args: vec![spanned(Expr::Ident("x".to_string()))],
            type_args: vec![],
            target_id: None,
        };
        assert!(validate_decidable_fragment(&expr, dummy_span(), ContractKind::Ensures).is_ok());
    }

    #[test]
    fn reject_old_in_invariant() {
        let expr = Expr::Call {
            name: spanned("old".to_string()),
            args: vec![spanned(Expr::Ident("x".to_string()))],
            type_args: vec![],
            target_id: None,
        };
        let result = validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("old() is only allowed in ensures clauses"));
    }

    #[test]
    fn reject_old_in_requires() {
        let expr = Expr::Call {
            name: spanned("old".to_string()),
            args: vec![spanned(Expr::Ident("x".to_string()))],
            type_args: vec![],
            target_id: None,
        };
        let result = validate_decidable_fragment(&expr, dummy_span(), ContractKind::Requires);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("old() is only allowed in ensures clauses"));
    }

    #[test]
    fn reject_function_call() {
        let expr = Expr::Call {
            name: spanned("foo".to_string()),
            args: vec![],
            type_args: vec![],
            target_id: None,
        };
        let result = validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("function call 'foo()' is not allowed"));
    }

    #[test]
    fn reject_string_literal() {
        let expr = Expr::StringLit("hello".to_string());
        let result = validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("string literals are not allowed"));
    }

    #[test]
    fn reject_string_interpolation() {
        let expr = Expr::StringInterp {
            parts: vec![],
        };
        let result = validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("string interpolation is not allowed"));
    }

    #[test]
    fn reject_struct_literal() {
        let expr = Expr::StructLit {
            name: spanned("Point".to_string()),
            type_args: vec![],
            fields: vec![],
            target_id: None,
        };
        let result = validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("struct literals are not allowed"));
    }

    #[test]
    fn reject_array_literal() {
        let expr = Expr::ArrayLit {
            elements: vec![],
        };
        let result = validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("array literals are not allowed"));
    }

    #[test]
    fn reject_closure() {
        let expr = Expr::Closure {
            params: vec![],
            body: spanned(Block { stmts: vec![] }),
            return_type: None,
        };
        let result = validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("closures are not allowed"));
    }

    #[test]
    fn reject_spawn() {
        let expr = Expr::Spawn {
            call: Box::new(spanned(Expr::IntLit(1))),
        };
        let result = validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("spawn is not allowed"));
    }

    #[test]
    fn reject_cast() {
        let expr = Expr::Cast {
            expr: Box::new(spanned(Expr::IntLit(1))),
            target_type: spanned(TypeExpr::Named("float".to_string())),
        };
        let result = validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("type casts are not allowed"));
    }

    #[test]
    fn reject_index() {
        let expr = Expr::Index {
            object: Box::new(spanned(Expr::Ident("arr".to_string()))),
            index: Box::new(spanned(Expr::IntLit(0))),
        };
        let result = validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("index expressions are not allowed"));
    }

    #[test]
    fn reject_range() {
        let expr = Expr::Range {
            start: Box::new(spanned(Expr::IntLit(0))),
            end: Box::new(spanned(Expr::IntLit(10))),
            inclusive: false,
        };
        let result = validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("range expressions are not allowed"));
    }

    #[test]
    fn reject_propagate() {
        let expr = Expr::Propagate {
            expr: Box::new(spanned(Expr::Ident("x".to_string()))),
        };
        let result = validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("error propagation is not allowed"));
    }

    #[test]
    fn reject_null_propagate() {
        let expr = Expr::NullPropagate {
            expr: Box::new(spanned(Expr::Ident("x".to_string()))),
        };
        let result = validate_decidable_fragment(&expr, dummy_span(), ContractKind::Invariant);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("null propagation is not allowed"));
    }
}
