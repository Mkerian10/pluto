/// Tests for the AST visitor pattern infrastructure (Phase 0)
///
/// These tests verify that the visitor traits and walk functions work correctly.
/// They don't use the full compilation pipeline, just the parser and visitor infrastructure.
use plutoc::parser::ast::*;
use plutoc::span::{Span, Spanned};
use plutoc::visit::{walk_expr, walk_stmt, Visitor};

#[test]
fn test_visitor_visits_nested_expressions() {
    // Create a simple AST: binary operation with integer literals
    let ast_program = create_simple_program();

    struct ExprCounter {
        count: usize,
    }

    impl Visitor for ExprCounter {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            self.count += 1;
            walk_expr(self, expr); // Continue recursion
        }
    }

    let mut counter = ExprCounter { count: 0 };
    counter.visit_program(&ast_program);

    // Should have visited the BinOp and both IntLit operands (3 expressions total)
    assert!(counter.count >= 3, "Expected at least 3 expressions, found {}", counter.count);
}

#[test]
fn test_visitor_can_collect_specific_nodes() {
    let program = create_simple_program();

    struct IntLitCollector {
        values: Vec<i64>,
    }

    impl Visitor for IntLitCollector {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if let Expr::IntLit(val) = &expr.node {
                self.values.push(*val);
            }
            walk_expr(self, expr);
        }
    }

    let mut collector = IntLitCollector { values: vec![] };
    collector.visit_program(&program);

    // Should have collected the integer literals from the program
    assert!(!collector.values.is_empty(), "Should have collected some integer literals");
}

#[test]
fn test_visitor_can_prune_subtree() {
    let program = create_nested_program();

    struct PruningVisitor {
        found_nested_value: bool,
    }

    impl Visitor for PruningVisitor {
        fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
            // If we find a Return statement, stop recursing (don't look inside)
            if matches!(stmt.node, Stmt::Return(_)) {
                // Don't call walk_stmt, so we won't visit expressions inside the return
                return;
            }
            walk_stmt(self, stmt);
        }

        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if let Expr::IntLit(42) = &expr.node {
                self.found_nested_value = true;
            }
            walk_expr(self, expr);
        }
    }

    let mut visitor = PruningVisitor {
        found_nested_value: false,
    };
    visitor.visit_program(&program);

    // Should NOT have found the value 42 because we pruned at the Return statement
    assert!(
        !visitor.found_nested_value,
        "Should not find value inside pruned subtree"
    );
}

#[test]
fn test_visitor_visits_type_exprs() {
    let program = create_program_with_types();

    struct TypeExprCounter {
        count: usize,
    }

    impl Visitor for TypeExprCounter {
        fn visit_type_expr(&mut self, te: &Spanned<TypeExpr>) {
            self.count += 1;
            plutoc::visit::walk_type_expr(self, te);
        }
    }

    let mut counter = TypeExprCounter { count: 0 };
    counter.visit_program(&program);

    // Should have visited type expressions in the program
    assert!(counter.count > 0, "Should have visited some type expressions");
}

// Helper functions to create AST nodes for testing

fn dummy_span() -> Span {
    Span {
        start: 0,
        end: 0,
        file_id: 0,
    }
}

fn create_simple_program() -> Program {
    // Creates a program with: fn main() { return 1 + 2 }
    let return_stmt = Stmt::Return(Some(Spanned::new(
        Expr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(Spanned::new(Expr::IntLit(1), dummy_span())),
            rhs: Box::new(Spanned::new(Expr::IntLit(2), dummy_span())),
        },
        dummy_span(),
    )));

    let function = Function {
        id: uuid::Uuid::nil(),
        name: Spanned::new("main".to_string(), dummy_span()),
        type_params: vec![],
        type_param_bounds: std::collections::HashMap::new(),
        params: vec![],
        return_type: None,
        contracts: vec![],
        body: Spanned::new(
            Block {
                stmts: vec![Spanned::new(return_stmt, dummy_span())],
            },
            dummy_span(),
        ),
        is_pub: false,
        is_override: false,
        is_generator: false,
    };

    Program {
        imports: vec![],
        functions: vec![Spanned::new(function, dummy_span())],
        extern_fns: vec![],
        classes: vec![],
        traits: vec![],
        enums: vec![],
        errors: vec![],
        app: None,
        stages: vec![],
        system: None,
        test_info: vec![],
        tests: None,
        fallible_extern_fns: vec![],
    }
}

fn create_nested_program() -> Program {
    // Creates a program with: fn main() { return 42 }
    let return_stmt = Stmt::Return(Some(Spanned::new(
        Expr::IntLit(42),
        dummy_span(),
    )));

    let function = Function {
        id: uuid::Uuid::nil(),
        name: Spanned::new("main".to_string(), dummy_span()),
        type_params: vec![],
        type_param_bounds: std::collections::HashMap::new(),
        params: vec![],
        return_type: None,
        contracts: vec![],
        body: Spanned::new(
            Block {
                stmts: vec![Spanned::new(return_stmt, dummy_span())],
            },
            dummy_span(),
        ),
        is_pub: false,
        is_override: false,
        is_generator: false,
    };

    Program {
        imports: vec![],
        functions: vec![Spanned::new(function, dummy_span())],
        extern_fns: vec![],
        classes: vec![],
        traits: vec![],
        enums: vec![],
        errors: vec![],
        app: None,
        stages: vec![],
        system: None,
        test_info: vec![],
        tests: None,
        fallible_extern_fns: vec![],
    }
}

fn create_program_with_types() -> Program {
    // Creates a program with: fn foo(x: int) int { return x }
    let param = Param {
        id: uuid::Uuid::nil(),
        name: Spanned::new("x".to_string(), dummy_span()),
        ty: Spanned::new(TypeExpr::Named("int".to_string()), dummy_span()),
        is_mut: false,
    };

    let return_stmt = Stmt::Return(Some(Spanned::new(
        Expr::Ident("x".to_string()),
        dummy_span(),
    )));

    let function = Function {
        id: uuid::Uuid::nil(),
        name: Spanned::new("foo".to_string(), dummy_span()),
        type_params: vec![],
        type_param_bounds: std::collections::HashMap::new(),
        params: vec![param],
        return_type: Some(Spanned::new(TypeExpr::Named("int".to_string()), dummy_span())),
        contracts: vec![],
        body: Spanned::new(
            Block {
                stmts: vec![Spanned::new(return_stmt, dummy_span())],
            },
            dummy_span(),
        ),
        is_pub: false,
        is_override: false,
        is_generator: false,
    };

    Program {
        imports: vec![],
        functions: vec![Spanned::new(function, dummy_span())],
        extern_fns: vec![],
        classes: vec![],
        traits: vec![],
        enums: vec![],
        errors: vec![],
        app: None,
        stages: vec![],
        system: None,
        test_info: vec![],
        tests: None,
        fallible_extern_fns: vec![],
    }
}
