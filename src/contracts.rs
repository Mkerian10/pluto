use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::Span;

/// Validate that all contract expressions in the program are within the decidable fragment.
/// Called after parsing, before typeck.
pub fn validate_contracts(program: &Program) -> Result<(), CompileError> {
    // Validate class invariants
    for class in &program.classes {
        for inv in &class.node.invariants {
            validate_decidable_fragment(&inv.node.expr.node, inv.node.expr.span)?;
        }
    }

    // Validate function contracts (requires/ensures — parsed but not enforced yet)
    for func in &program.functions {
        for contract in &func.node.contracts {
            validate_decidable_fragment(&contract.node.expr.node, contract.node.expr.span)?;
        }
    }

    // Validate method contracts on classes
    for class in &program.classes {
        for method in &class.node.methods {
            for contract in &method.node.contracts {
                validate_decidable_fragment(&contract.node.expr.node, contract.node.expr.span)?;
            }
        }
    }

    // Validate app method contracts
    if let Some(app) = &program.app {
        for method in &app.node.methods {
            for contract in &method.node.contracts {
                validate_decidable_fragment(&contract.node.expr.node, contract.node.expr.span)?;
            }
        }
    }

    // Validate trait method contracts
    for tr in &program.traits {
        for method in &tr.node.methods {
            for contract in &method.contracts {
                validate_decidable_fragment(&contract.node.expr.node, contract.node.expr.span)?;
            }
        }
    }

    Ok(())
}

/// Validate that an expression is within the decidable fragment allowed in contracts.
///
/// Allowed:
/// - Comparisons, arithmetic, logical, bitwise operators
/// - Unary Not, Neg, BitNot
/// - Identifiers (parameter/field names)
/// - Field access (self.field, nested)
/// - Method call `.len()` only (no args)
/// - Int, float, bool literals
///
/// Rejected:
/// - Function calls, string literals, string interpolation
/// - Struct/array/map/set/enum literals
/// - Closures, spawn, cast, index, range
/// - Catch, propagate
/// - Any method call other than `.len()`
fn validate_decidable_fragment(expr: &Expr, span: Span) -> Result<(), CompileError> {
    match expr {
        // Literals — allowed
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) => Ok(()),

        // Identifiers — allowed
        Expr::Ident(_) => Ok(()),

        // Binary operators — allowed (recurse into operands)
        Expr::BinOp { lhs, rhs, .. } => {
            validate_decidable_fragment(&lhs.node, lhs.span)?;
            validate_decidable_fragment(&rhs.node, rhs.span)
        }

        // Unary operators — allowed (recurse into operand)
        Expr::UnaryOp { operand, .. } => {
            validate_decidable_fragment(&operand.node, operand.span)
        }

        // Field access — allowed (recurse into object)
        Expr::FieldAccess { object, .. } => {
            validate_decidable_fragment(&object.node, object.span)
        }

        // Method call — only .len() with no args
        Expr::MethodCall { object, method, args } => {
            if method.node == "len" && args.is_empty() {
                validate_decidable_fragment(&object.node, object.span)
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

        // Everything else — rejected
        Expr::Call { name, .. } => Err(CompileError::syntax(
            format!("function call '{}()' is not allowed in contract expressions", name.node),
            span,
        )),
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
