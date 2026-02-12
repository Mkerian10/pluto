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

// ==============================================================================
// Migration Verification Tests (Phases 1-3)
// ==============================================================================
//
// These tests verify the correctness of visitor implementations that were
// converted during the visitor pattern migration. They use end-to-end
// compilation tests to ensure the visitors work correctly in practice.
//
// Coverage goal: Verify all major visitor-based features work correctly
// ==============================================================================

mod common;
use common::{compile_and_run, compile_and_run_stdout, compile_should_fail};

#[test]
fn test_visitor_spawn_closure_detection() {
    // SpawnClosureCollector (codegen/mod.rs) detects spawn expressions
    let source = r#"
        fn main() {
            let t1 = spawn foo()
            let t2 = spawn bar()
            t1.detach()
            t2.detach()
        }

        fn foo() int { return 1 }
        fn bar() int { return 2 }
    "#;

    let result = compile_and_run(source);
    assert_eq!(result, 0, "Spawn closure collection should work");
}

#[test]
fn test_visitor_error_propagation_detection() {
    // PropagateDetector (typeck/errors.rs) finds error propagation operators
    let source = r#"
        error MyErr { msg: string }

        fn foo() int {
            bar()!
            return 42
        }

        fn bar() int {
            raise MyErr { msg: "error" }
        }

        fn main() {
            let x = foo() catch err { 0 }
            print(x)
        }
    "#;

    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "0", "Propagate detection should work");
}

#[test]
fn test_visitor_free_variable_collection() {
    // FreeVarCollector (typeck/closures.rs) detects captured variables
    let source = r#"
        fn main() {
            let x = 10
            let y = 20
            let f = () => x + y
            let result = f()
            print(result)
        }
    "#;

    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "30", "Free variable collection should work");
}

#[test]
fn test_visitor_free_variables_with_params() {
    // Free variables vs bound parameters
    let source = r#"
        fn main() {
            let x = 100
            let f = (y: int) => x + y
            let result = f(5)
            print(result)
        }
    "#;

    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "105", "Parameter binding should work correctly");
}

#[test]
fn test_visitor_nested_closure_captures() {
    // Nested closures with multiple levels of capture
    let source = r#"
        fn main() {
            let x = 10
            let f = () => {
                let y = 20
                let g = () => x + y
                return g()
            }
            let result = f()
            print(result)
        }
    "#;

    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "30", "Nested closure captures should work");
}

#[test]
fn test_visitor_self_mutation_immutable_method() {
    // SelfMutationChecker (typeck/check.rs) detects illegal mutations
    let source = r#"
        class Counter {
            value: int

            fn increment(self) {
                self.value = self.value + 1
            }
        }

        fn main() {}
    "#;

    compile_should_fail(source);
}

#[test]
fn test_visitor_self_mutation_mutable_method() {
    // Mutable methods can mutate self
    let source = r#"
        class Counter {
            value: int

            fn increment(mut self) {
                self.value = self.value + 1
            }

            fn get(self) int {
                return self.value
            }
        }

        fn main() {
            let mut c = Counter { value: 0 }
            c.increment()
            c.increment()
            print(c.get())
        }
    "#;

    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "2", "Mutable method mutation should work");
}

#[test]
fn test_visitor_identifier_collection() {
    // IdentCollector (typeck/check.rs) collects identifiers
    let source = r#"
        class Foo {
            x: int
            y: int

            fn swap(mut self) {
                let temp = self.x
                self.x = self.y
                self.y = temp
            }

            fn sum(self) int {
                return self.x + self.y
            }
        }

        fn main() {
            let mut f = Foo { x: 10, y: 20 }
            f.swap()
            print(f.sum())
        }
    "#;

    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "30", "Identifier collection should work");
}

#[test]
fn test_visitor_dependency_injection() {
    // DependencyCollector (derived.rs) collects injected dependencies
    let source = r#"
        class Database {
            fn query(self) string {
                return "data"
            }
        }

        class Service[db: Database] {
            fn process(self) string {
                return self.db.query()
            }
        }

        app MyApp[service: Service] {
            fn main(self) {
                let result = self.service.process()
                print(result)
            }
        }
    "#;

    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "data", "Dependency collection should work");
}

#[test]
fn test_visitor_spawn_in_nested_contexts() {
    // Spawn detection in various contexts
    let source = r#"
        fn compute(x: int) int {
            return x * 2
        }

        fn main() {
            let results = [
                spawn compute(1),
                spawn compute(2),
                spawn compute(3),
            ]

            let sum = results[0].get()! + results[1].get()! + results[2].get()!
            print(sum)
        }
    "#;

    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "12", "Spawn in arrays should work");
}

#[test]
fn test_visitor_propagate_in_nested_blocks() {
    // Propagate detection in complex control flow
    let source = r#"
        error Err { code: int }

        fn risky() int {
            raise Err { code: 42 }
        }

        fn process() int {
            if true {
                return risky()!
            }
            return 0
        }

        fn main() {
            let x = process() catch err { 99 }
            print(x)
        }
    "#;

    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "99", "Nested propagate detection should work");
}
