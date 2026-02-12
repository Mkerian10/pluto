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

        // Qualified access — TODO: evaluate if should be allowed
        Expr::QualifiedAccess { .. } => Err(CompileError::syntax(
            "qualified access is not allowed in contract expressions",
            span,
        )),

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
    }
}
