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
    for stage in &mut program.stages {
        for method in &mut stage.node.methods {
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
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &mut arm.op {
                    SelectOp::Recv { channel, .. } => {
                        desugar_expr(&mut channel.node, channel.span);
                    }
                    SelectOp::Send { channel, value } => {
                        desugar_expr(&mut channel.node, channel.span);
                        desugar_expr(&mut value.node, value.span);
                    }
                }
                desugar_block(&mut arm.body.node);
            }
            if let Some(def) = default {
                desugar_block(&mut def.node);
            }
        }
        Stmt::Scope { seeds, body, .. } => {
            for seed in seeds {
                desugar_expr(&mut seed.node, seed.span);
            }
            desugar_block(&mut body.node);
        }
        Stmt::Yield { value, .. } => {
            desugar_expr(&mut value.node, value.span);
        }
        Stmt::Break | Stmt::Continue => {}
    }
}

fn desugar_expr(expr: &mut Expr, span: Span) {
    match expr {
        Expr::Spawn { call } => {
            // First recurse into the call's arguments (and object for method calls)
            match &mut call.node {
                Expr::Call { args, .. } => {
                    for arg in args.iter_mut() {
                        desugar_expr(&mut arg.node, arg.span);
                    }
                }
                Expr::MethodCall { object, args, .. } => {
                    desugar_expr(&mut object.node, object.span);
                    for arg in args.iter_mut() {
                        desugar_expr(&mut arg.node, arg.span);
                    }
                }
                _ => {}
            }
            // Replace the Call/MethodCall with a Closure wrapping it
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
            **call = Spanned::new(closure, call_span);
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
                CatchHandler::Wildcard { body, .. } => desugar_block(&mut body.node),
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
        Expr::NullPropagate { expr: inner } => {
            desugar_expr(&mut inner.node, inner.span);
        }
        Expr::StaticTraitCall { args, .. } => {
            for arg in args {
                desugar_expr(&mut arg.node, arg.span);
            }
        }
        Expr::QualifiedAccess { segments } => {
            panic!(
                "QualifiedAccess should be resolved by module flattening before spawn. Segments: {:?}",
                segments.iter().map(|s| &s.node).collect::<Vec<_>>()
            )
        }
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_)
        | Expr::Ident(_) | Expr::EnumUnit { .. } | Expr::ClosureCreate { .. }
        | Expr::NoneLit => {}
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
    fn spawn_desugars_function_call() {
        let mut expr = Expr::Spawn {
            call: Box::new(spanned(Expr::Call {
                name: spanned("foo".to_string()),
                args: vec![],
                type_args: vec![],
                target_id: None,
            })),
        };

        desugar_expr(&mut expr, dummy_span());

        // Check that spawn now contains a closure
        match expr {
            Expr::Spawn { call } => match &call.node {
                Expr::Closure { params, body, return_type } => {
                    assert_eq!(params.len(), 0);
                    assert!(return_type.is_none());
                    assert_eq!(body.node.stmts.len(), 1);
                    // Check the closure body contains a return statement with the original call
                    match &body.node.stmts[0].node {
                        Stmt::Return(Some(ret_expr)) => match &ret_expr.node {
                            Expr::Call { name, .. } => {
                                assert_eq!(name.node, "foo");
                            }
                            _ => panic!("Expected call in return statement"),
                        },
                        _ => panic!("Expected return statement in closure body"),
                    }
                }
                _ => panic!("Expected closure after desugaring"),
            },
            _ => panic!("Expected spawn expression"),
        }
    }

    #[test]
    fn spawn_desugars_method_call() {
        let mut expr = Expr::Spawn {
            call: Box::new(spanned(Expr::MethodCall {
                object: Box::new(spanned(Expr::Ident("obj".to_string()))),
                method: spanned("bar".to_string()),
                args: vec![],
            })),
        };

        desugar_expr(&mut expr, dummy_span());

        // Check that spawn now contains a closure
        match expr {
            Expr::Spawn { call } => match &call.node {
                Expr::Closure { params, body, return_type } => {
                    assert_eq!(params.len(), 0);
                    assert!(return_type.is_none());
                    assert_eq!(body.node.stmts.len(), 1);
                    match &body.node.stmts[0].node {
                        Stmt::Return(Some(ret_expr)) => match &ret_expr.node {
                            Expr::MethodCall { method, .. } => {
                                assert_eq!(method.node, "bar");
                            }
                            _ => panic!("Expected method call in return statement"),
                        },
                        _ => panic!("Expected return statement in closure body"),
                    }
                }
                _ => panic!("Expected closure after desugaring"),
            },
            _ => panic!("Expected spawn expression"),
        }
    }

    #[test]
    fn spawn_preserves_function_call_args() {
        let mut expr = Expr::Spawn {
            call: Box::new(spanned(Expr::Call {
                name: spanned("foo".to_string()),
                args: vec![
                    spanned(Expr::IntLit(42)),
                    spanned(Expr::Ident("x".to_string())),
                ],
                type_args: vec![],
                target_id: None,
            })),
        };

        desugar_expr(&mut expr, dummy_span());

        match expr {
            Expr::Spawn { call } => match &call.node {
                Expr::Closure { body, .. } => match &body.node.stmts[0].node {
                    Stmt::Return(Some(ret_expr)) => match &ret_expr.node {
                        Expr::Call { name, args, .. } => {
                            assert_eq!(name.node, "foo");
                            assert_eq!(args.len(), 2);
                            assert!(matches!(args[0].node, Expr::IntLit(42)));
                            assert!(matches!(args[1].node, Expr::Ident(_)));
                        }
                        _ => panic!("Expected call"),
                    },
                    _ => panic!("Expected return"),
                },
                _ => panic!("Expected closure"),
            },
            _ => panic!("Expected spawn"),
        }
    }

    #[test]
    fn desugar_recurses_into_nested_expressions() {
        // Test that desugaring recurses properly into nested structures
        let mut expr = Expr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(spanned(Expr::Spawn {
                call: Box::new(spanned(Expr::Call {
                    name: spanned("foo".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            })),
            rhs: Box::new(spanned(Expr::IntLit(1))),
        };

        desugar_expr(&mut expr, dummy_span());

        // Check that the spawn in the lhs was desugared
        match expr {
            Expr::BinOp { lhs, .. } => match &lhs.node {
                Expr::Spawn { call } => match &call.node {
                    Expr::Closure { .. } => {
                        // Success - spawn was desugared
                    }
                    _ => panic!("Spawn should contain closure after desugaring"),
                },
                _ => panic!("Expected spawn in lhs"),
            },
            _ => panic!("Expected binop"),
        }
    }

    #[test]
    fn desugar_handles_array_literals() {
        let mut expr = Expr::ArrayLit {
            elements: vec![
                spanned(Expr::IntLit(1)),
                spanned(Expr::Spawn {
                    call: Box::new(spanned(Expr::Call {
                        name: spanned("foo".to_string()),
                        args: vec![],
                        type_args: vec![],
                        target_id: None,
                    })),
                }),
            ],
        };

        desugar_expr(&mut expr, dummy_span());

        match expr {
            Expr::ArrayLit { elements } => {
                assert_eq!(elements.len(), 2);
                // Second element should be desugared spawn
                match &elements[1].node {
                    Expr::Spawn { call } => match &call.node {
                        Expr::Closure { .. } => {
                            // Success
                        }
                        _ => panic!("Spawn should contain closure"),
                    },
                    _ => panic!("Expected spawn in array"),
                }
            }
            _ => panic!("Expected array literal"),
        }
    }

    #[test]
    fn desugar_stmt_let_binding() {
        let mut stmt = Stmt::Let {
            name: spanned("x".to_string()),
            ty: None,
            is_mut: false,
            value: spanned(Expr::Spawn {
                call: Box::new(spanned(Expr::Call {
                    name: spanned("foo".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            }),
        };

        desugar_stmt(&mut stmt);

        match stmt {
            Stmt::Let { value, .. } => match &value.node {
                Expr::Spawn { call } => match &call.node {
                    Expr::Closure { .. } => {
                        // Success
                    }
                    _ => panic!("Spawn should be desugared"),
                },
                _ => panic!("Expected spawn"),
            },
            _ => panic!("Expected let statement"),
        }
    }

    #[test]
    fn desugar_block_multiple_statements() {
        let mut block = Block {
            stmts: vec![
                spanned(Stmt::Let {
                    name: spanned("x".to_string()),
                    ty: None,
                    is_mut: false,
                    value: spanned(Expr::Spawn {
                        call: Box::new(spanned(Expr::Call {
                            name: spanned("foo".to_string()),
                            args: vec![],
                            type_args: vec![],
                            target_id: None,
                        })),
                    }),
                }),
                spanned(Stmt::Return(Some(spanned(Expr::Spawn {
                    call: Box::new(spanned(Expr::Call {
                        name: spanned("bar".to_string()),
                        args: vec![],
                        type_args: vec![],
                        target_id: None,
                    })),
                })))),
            ],
        };

        desugar_block(&mut block);

        // Both statements should have their spawns desugared
        assert_eq!(block.stmts.len(), 2);

        // Check first statement
        match &block.stmts[0].node {
            Stmt::Let { value, .. } => match &value.node {
                Expr::Spawn { call } => match &call.node {
                    Expr::Closure { .. } => {}
                    _ => panic!("First spawn should be desugared"),
                },
                _ => panic!("Expected spawn in let"),
            },
            _ => panic!("Expected let statement"),
        }

        // Check second statement
        match &block.stmts[1].node {
            Stmt::Return(Some(ret_expr)) => match &ret_expr.node {
                Expr::Spawn { call } => match &call.node {
                    Expr::Closure { .. } => {}
                    _ => panic!("Second spawn should be desugared"),
                },
                _ => panic!("Expected spawn in return"),
            },
            _ => panic!("Expected return statement"),
        }
    }
}
