use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::Spanned;
use crate::visit::{walk_expr_mut, VisitMut};

/// Desugar `spawn func(args)` into `spawn (=> { return func(args) })`.
///
/// After this pass, `Expr::Spawn { call }` contains a `Expr::Closure` instead of
/// an `Expr::Call`. The closure infrastructure (capture analysis, lifting, codegen)
/// handles the rest.
struct SpawnDesugarer;

impl VisitMut for SpawnDesugarer {
    fn visit_expr_mut(&mut self, expr: &mut Spanned<Expr>) {
        // First recurse to handle nested spawns (bottom-up)
        walk_expr_mut(self, expr);

        // Then desugar this node if it's a Spawn
        if let Expr::Spawn { call } = &mut expr.node {
            // Replace the Call/MethodCall with a Closure wrapping it
            let call_spanned = std::mem::replace(
                call,
                Box::new(Spanned::new(Expr::IntLit(0), expr.span)), // temporary placeholder
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
    }
}

/// Desugar `spawn func(args)` into `spawn (=> { return func(args) })`.
///
/// After this pass, `Expr::Spawn { call }` contains a `Expr::Closure` instead of
/// an `Expr::Call`. The closure infrastructure (capture analysis, lifting, codegen)
/// handles the rest.
pub fn desugar_spawn(program: &mut Program) -> Result<(), CompileError> {
    let mut desugarer = SpawnDesugarer;
    desugarer.visit_program_mut(program);
    Ok(())
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

    /// Helper to desugar a single expression for testing
    fn desugar_expr(expr: &mut Expr) {
        let mut spanned_expr = Spanned::new(std::mem::replace(expr, Expr::IntLit(0)), dummy_span());
        let mut desugarer = SpawnDesugarer;
        desugarer.visit_expr_mut(&mut spanned_expr);
        *expr = spanned_expr.node;
    }

    /// Helper to desugar a single statement for testing
    fn desugar_stmt(stmt: &mut Stmt) {
        let mut spanned_stmt = Spanned::new(std::mem::replace(stmt, Stmt::Break), dummy_span());
        let mut desugarer = SpawnDesugarer;
        desugarer.visit_stmt_mut(&mut spanned_stmt);
        *stmt = spanned_stmt.node;
    }

    /// Helper to desugar a block for testing
    fn desugar_block(block: &mut Block) {
        let mut spanned_block = Spanned::new(std::mem::replace(block, Block { stmts: vec![] }), dummy_span());
        let mut desugarer = SpawnDesugarer;
        desugarer.visit_block_mut(&mut spanned_block);
        *block = spanned_block.node;
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

        desugar_expr(&mut expr);

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

        desugar_expr(&mut expr);

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

        desugar_expr(&mut expr);

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

        desugar_expr(&mut expr);

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

        desugar_expr(&mut expr);

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
