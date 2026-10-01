// Error Recovery & Malformed Input Tests
// Inspired by Rust's parse-fail tests and Go's error recovery
//
// Tests parser's ability to produce helpful errors for malformed input
// Target: 18 tests

mod common;
use common::*;

// ============================================================
// Missing Tokens
// ============================================================

#[test]
fn missing_opening_paren() {
    compile_should_fail_with(r#"
        fn main) {
            print("test")
        }
    "#, "expected");
}

#[test]
fn missing_closing_paren() {
    compile_should_fail_with(r#"
        fn main( {
            print("test")
        }
    "#, "expected");
}

#[test]
fn missing_opening_brace() {
    compile_should_fail_with(r#"
        fn main()
            print("test")
        }
    "#, "expected");
}

#[test]
fn missing_closing_brace_at_eof() {
    compile_should_fail_with(r#"
        fn main() {
            print("test")
    "#, "expected");
}

#[test]
fn missing_comma_in_function_params() {
    compile_should_fail_with(r#"
        fn add(x: int y: int) int {
            return x + y
        }
    "#, "expected");
}

#[test]
fn missing_colon_in_type_annotation() {
    compile_should_fail_with(r#"
        fn main() {
            let x int = 5
        }
    "#, "expected");
}

#[test]
fn missing_equals_in_let_binding() {
    compile_should_fail_with(r#"
        fn main() {
            let x: int 5
        }
    "#, "expected");
}

#[test]
fn missing_arrow_in_closure() {
    compile_should_fail_with(r#"
        fn main() {
            let f = (x: int) x + 1
        }
    "#, "expected");
}

// ============================================================
// Extra/Unexpected Tokens
// ============================================================

#[test]
fn double_comma_in_params() {
    compile_should_fail_with(r#"
        fn foo(x: int,, y: int) int {
            return x + y
        }
    "#, "expected");
}

#[test]
fn unexpected_keyword_as_identifier() {
    // Using 'fn' as a variable name
    compile_should_fail_with(r#"
        fn main() {
            let fn = 5
        }
    "#, "expected");
}

#[test]
fn stray_closing_brace() {
    compile_should_fail_with(r#"
        fn main() {
            print("test")
        }
        }
    "#, "found }");
}

#[test]
fn double_operator() {
    // ++ is not supported in Pluto (not increment operator)
    compile_should_fail_with(r#"
        fn main() {
            let x = 5
            x++
        }
    "#, "cannot assign to immutable variable 'x'");
}

// ============================================================
// Incomplete Constructs
// ============================================================

#[test]
fn incomplete_if_statement() {
    compile_should_fail_with(r#"
        fn main() {
            if true
        }
    "#, "expected");
}

#[test]
fn incomplete_while_loop() {
    compile_should_fail_with(r#"
        fn main() {
            while x < 10
        }
    "#, "expected");
}

#[test]
fn incomplete_function_definition() {
    compile_should_fail_with(r#"
        fn foo()
    "#, "expected");
}

#[test]
fn incomplete_class_definition() {
    compile_should_fail_with(r#"
        class Foo
    "#, "expected");
}

#[test]
fn incomplete_match_expression() {
    compile_should_fail_with(r#"
        fn main() {
            let x = 5
            match x {
        }
    "#, "expected");
}

#[test]
fn incomplete_struct_literal() {
    compile_should_fail_with(r#"
        class Point { x: int, y: int }

        fn main() {
            let p = Point {
        }
    "#, "expected");
}

// ============================================================
// Acceptance boundaries of the newer clause grammar
// (typestates, contracts, guarded_by, objects, at placement).
// Never-valid syntax has lurked in tests for years (freestanding
// `impl T {}`, explicit `!` return annotations) — these pin the
// boundary: each shape must be *rejected*, and rejected cleanly.
// ============================================================

#[test]
fn freestanding_impl_block_rejected() {
    compile_should_fail_with(r#"
        class T { x: int }

        impl T {
            fn get(self) int {
                return self.x
            }
        }

        fn main() {
            let t = T { x: 1 }
            print(t.get())
        }
    "#, "found impl");
}

#[test]
fn explicit_bang_return_annotation_rejected() {
    // Fallibility is inferred on declarations; `!` belongs only on fn
    // *types* (`fn(int) int!`), never on a declaration's return type.
    compile_should_fail_with(r#"
        error Boom {
        }

        fn f() int! {
            raise Boom { }
        }

        fn main() {
            let x = f() catch 0
            print(x)
        }
    "#, "expected {, found !");
}

#[test]
fn ensures_before_return_type_rejected() {
    // Clause order is fixed: return type, then contract clauses.
    compile_should_fail(r#"
        class C {
            n: int

            fn bump(mut self) ensures self.n == old(self.n) + 1 int {
                self.n = self.n + 1
                return self.n
            }
        }

        fn main() {
            let mut c = C { n: 0 }
            print(c.bump())
        }
    "#);
}

#[test]
fn guarded_by_without_binder_rejected() {
    // The binder parens are mandatory: `guarded_by (g: Grant) pred`.
    compile_should_fail_with(r#"
        class Grant { token: int }

        object Blob {
            data: int guarded_by g.token == self.epoch
            epoch: int
        }

        fn main() {
            let b = Blob { data: 0, epoch: 0 }
        }
    "#, "expected (");
}

#[test]
fn must_release_on_non_typestate_class_rejected() {
    compile_should_fail(r#"
        class Plain {
            id: int

            must_release Held

            fn get(self) int {
                return self.id
            }
        }

        fn main() {
            let p = Plain { id: 1 }
            print(p.get())
        }
    "#);
}

#[test]
fn at_on_non_entity_value_rejected() {
    compile_should_fail_with(r#"
        class Box { v: int }

        fn foo() int {
            return 1
        }

        fn main() {
            let b = Box { v: 1 }
            let r = at b { foo() } catch -1
            print(r)
        }
    "#, "'at' requires a domain or an entity");
}

#[test]
fn contract_clauses_on_field_rejected() {
    // `ensures` is a method clause; fields only take `guarded_by`.
    compile_should_fail(r#"
        class C {
            x: int ensures self.x > 0
        }

        fn main() {
            let c = C { x: 1 }
        }
    "#);
}

#[test]
fn interleaved_where_ensures_clause_jumble_rejected() {
    compile_should_fail(r#"
        class Lease<S> {
            id: int

            fn acquire(self) Lease<Held> where S == Idle ensures self.id == old(self.id) where S == Held {
                return Lease<Held> { id: 1 }
            }
        }

        fn main() {
            print(1)
        }
    "#);
}

#[test]
fn object_unclosed_at_eof_rejected() {
    compile_should_fail(r#"
        object Broken<T> {
            value: T

            fn get(self) T {
                return self.value
    "#);
}

#[test]
fn bare_invariant_without_expression_rejected() {
    compile_should_fail(r#"
        class C {
            x: int

            invariant
        }

        fn main() {
            let c = C { x: 1 }
        }
    "#);
}
