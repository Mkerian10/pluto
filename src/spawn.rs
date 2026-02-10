use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::{Span, Spanned};

/// Desugar `spawn func(args)` into `spawn (=> { return func(args) })`.
///
/// After this pass, `Expr::Spawn { call }` contains a `Expr::Closure` instead of
/// an `Expr::Call`. The closure infrastructure (capture analysis, lifting, codegen)
/// handles the rest.
pub fn desugar_spawn(program: &mut Program) -> Result<(), CompileError> {
    for func in &mut program.functions {
        desugar_block(&mut func.node.body.node);
    }
    for class in &mut program.classes {
        for method in &mut class.node.methods {
            desugar_block(&mut method.node.body.node);
        }
    }
    if let Some(app) = &mut program.app {
        for method in &mut app.node.methods {
            desugar_block(&mut method.node.body.node);
        }
    }
    Ok(())
}

fn desugar_block(block: &mut Block) {
    for stmt in &mut block.stmts {
        desugar_stmt(&mut stmt.node);
    }
}

fn desugar_stmt(stmt: &mut Stmt) {
    match stmt {
        Stmt::Let { value, .. } => desugar_expr(&mut value.node, value.span),
        Stmt::Return(Some(expr)) => desugar_expr(&mut expr.node, expr.span),
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => desugar_expr(&mut value.node, value.span),
        Stmt::FieldAssign { object, value, .. } => {
            desugar_expr(&mut object.node, object.span);
            desugar_expr(&mut value.node, value.span);
        }
        Stmt::If { condition, then_block, else_block } => {
            desugar_expr(&mut condition.node, condition.span);
            desugar_block(&mut then_block.node);
            if let Some(eb) = else_block {
                desugar_block(&mut eb.node);
            }
        }
        Stmt::While { condition, body } => {
            desugar_expr(&mut condition.node, condition.span);
            desugar_block(&mut body.node);
        }
        Stmt::For { iterable, body, .. } => {
            desugar_expr(&mut iterable.node, iterable.span);
            desugar_block(&mut body.node);
        }
        Stmt::IndexAssign { object, index, value } => {
            desugar_expr(&mut object.node, object.span);
            desugar_expr(&mut index.node, index.span);
            desugar_expr(&mut value.node, value.span);
        }
        Stmt::Match { expr, arms } => {
            desugar_expr(&mut expr.node, expr.span);
            for arm in arms {
                desugar_block(&mut arm.body.node);
            }
        }
        Stmt::Raise { fields, .. } => {
            for (_, val) in fields {
                desugar_expr(&mut val.node, val.span);
            }
        }
        Stmt::Expr(expr) => desugar_expr(&mut expr.node, expr.span),
        Stmt::LetChan { capacity, .. } => {
            if let Some(cap) = capacity {
                desugar_expr(&mut cap.node, cap.span);
            }
        }
        Stmt::Break | Stmt::Continue => {}
    }
}

fn desugar_expr(expr: &mut Expr, span: Span) {
    match expr {
        Expr::Spawn { call } => {
            // First recurse into the call's arguments
            if let Expr::Call { args, .. } = &mut call.node {
                for arg in args.iter_mut() {
                    desugar_expr(&mut arg.node, arg.span);
                }
            }
            // Replace the Call with a Closure wrapping it
            let call_spanned = std::mem::replace(
                call,
                Box::new(Spanned::new(Expr::IntLit(0), span)), // temporary placeholder
            );
            let call_span = call_spanned.span;
            let return_stmt = Spanned::new(
                Stmt::Return(Some(*call_spanned)),
                call_span,
            );
            let closure = Expr::Closure {
                params: vec![],
                return_type: None,
                body: Spanned::new(
                    Block { stmts: vec![return_stmt] },
                    call_span,
                ),
            };
            *call = Box::new(Spanned::new(closure, call_span));
        }
        Expr::BinOp { lhs, rhs, .. } => {
            desugar_expr(&mut lhs.node, lhs.span);
            desugar_expr(&mut rhs.node, rhs.span);
        }
        Expr::UnaryOp { operand, .. } => {
            desugar_expr(&mut operand.node, operand.span);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                desugar_expr(&mut arg.node, arg.span);
            }
        }
        Expr::FieldAccess { object, .. } => {
            desugar_expr(&mut object.node, object.span);
        }
        Expr::MethodCall { object, args, .. } => {
            desugar_expr(&mut object.node, object.span);
            for arg in args {
                desugar_expr(&mut arg.node, arg.span);
            }
        }
        Expr::StructLit { fields, .. } => {
            for (_, val) in fields {
                desugar_expr(&mut val.node, val.span);
            }
        }
        Expr::ArrayLit { elements } => {
            for elem in elements {
                desugar_expr(&mut elem.node, elem.span);
            }
        }
        Expr::Index { object, index } => {
            desugar_expr(&mut object.node, object.span);
            desugar_expr(&mut index.node, index.span);
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    desugar_expr(&mut e.node, e.span);
                }
            }
        }
        Expr::EnumData { fields, .. } => {
            for (_, val) in fields {
                desugar_expr(&mut val.node, val.span);
            }
        }
        Expr::Closure { body, .. } => {
            desugar_block(&mut body.node);
        }
        Expr::Propagate { expr: inner } => {
            desugar_expr(&mut inner.node, inner.span);
        }
        Expr::Catch { expr: inner, handler } => {
            desugar_expr(&mut inner.node, inner.span);
            match handler {
                CatchHandler::Wildcard { body, .. } => desugar_expr(&mut body.node, body.span),
                CatchHandler::Shorthand(fb) => desugar_expr(&mut fb.node, fb.span),
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                desugar_expr(&mut k.node, k.span);
                desugar_expr(&mut v.node, v.span);
            }
        }
        Expr::SetLit { elements, .. } => {
            for elem in elements {
                desugar_expr(&mut elem.node, elem.span);
            }
        }
        Expr::Cast { expr: inner, .. } => {
            desugar_expr(&mut inner.node, inner.span);
        }
        Expr::Range { start, end, .. } => {
            desugar_expr(&mut start.node, start.span);
            desugar_expr(&mut end.node, end.span);
        }
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_)
        | Expr::Ident(_) | Expr::EnumUnit { .. } | Expr::ClosureCreate { .. } => {}
    }
}
