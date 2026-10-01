//! Property-based tests for AST invariants.
//!
//! These tests use proptest to verify compiler invariants hold across
//! a wide variety of generated programs.

use proptest::prelude::*;
use pluto::lexer::lex;

// Simple program generators - start with basic constructs
fn arb_simple_program() -> impl Strategy<Value = String> {
    prop::collection::vec(arb_statement(), 1..5).prop_map(|stmts| stmts.join("\n"))
}

fn arb_statement() -> impl Strategy<Value = String> {
    prop_oneof![
        arb_let_statement(),
        arb_function_call(),
        arb_return_statement(),
    ]
}

fn arb_let_statement() -> impl Strategy<Value = String> {
    (arb_identifier(), arb_simple_expr()).prop_map(|(name, expr)| format!("let {name} = {expr}"))
}

fn arb_function_call() -> impl Strategy<Value = String> {
    arb_identifier().prop_map(|name| format!("{name}()"))
}

fn arb_return_statement() -> impl Strategy<Value = String> {
    arb_simple_expr().prop_map(|expr| format!("return {expr}"))
}

fn arb_simple_expr() -> impl Strategy<Value = String> {
    prop_oneof![
        arb_int_lit(),
        arb_string_lit(),
        arb_bool_lit(),
        arb_identifier(),
    ]
}

fn arb_identifier() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,10}".prop_map(|s| s.to_string())
}

fn arb_int_lit() -> impl Strategy<Value = String> {
    (0i64..1000).prop_map(|n| n.to_string())
}

fn arb_string_lit() -> impl Strategy<Value = String> {
    "[a-zA-Z ]{0,20}".prop_map(|s| format!("\"{s}\""))
}

fn arb_bool_lit() -> impl Strategy<Value = String> {
    prop_oneof![Just("true".to_string()), Just("false".to_string())]
}

// ─────────────────────────────────────────────────────────────────────────────
// New grammar surface: typestates (`where` constraints, `must_release`),
// contracts (`ensures` with `old()`, `requires`), `guarded_by` field clauses,
// `object` declarations, state-carrying errors, and `at` placement.
// ─────────────────────────────────────────────────────────────────────────────

/// Typestated class: `where` state constraints on methods, transitions, and
/// (optionally) a `must_release` obligation that the driver discharges.
fn arb_typestate_program() -> impl Strategy<Value = String> {
    (1..100i64, any::<bool>(), any::<bool>()).prop_map(|(id, with_release, with_extra)| {
        let release_clause = if with_release {
            "\n    must_release Held\n"
        } else {
            ""
        };
        let extra_method = if with_extra {
            r#"
    fn peek(self) int where S == Held {
        return self.id
    }
"#
        } else {
            ""
        };
        format!(
            r#"class Idle {{ tag: int }}
class Held {{ tag: int }}

class Lease<S> {{
    id: int
{release_clause}
    fn acquire(self) Lease<Held> where S == Idle {{
        return Lease<Held> {{ id: self.id }}
    }}

    fn release(self) Lease<Idle> where S == Held {{
        return Lease<Idle> {{ id: self.id }}
    }}
{extra_method}
    fn describe(self) int {{
        return self.id
    }}
}}

fn main() {{
    let l = Lease<Idle> {{ id: {id} }}
    let h = l.acquire()
    let done = h.release()
    print(done.describe())
}}
"#
        )
    })
}

/// Method contracts: `requires`, and `ensures` relating exit state to
/// `old()` entry state through provable linear arithmetic.
fn arb_contract_program() -> impl Strategy<Value = String> {
    (1..20i64, 1..100i64, 20..1000i64).prop_map(|(step, amt, start)| {
        format!(
            r#"class Counter {{
    n: int

    fn bump(mut self) ensures self.n == old(self.n) + {step} {{
        self.n = self.n + {step}
    }}
}}

class Account {{
    balance: int

    fn withdraw(mut self, amt: int) requires amt > 0
        ensures self.balance == old(self.balance) - amt {{
        self.balance = self.balance - amt
    }}
}}

fn main() {{
    let mut c = Counter {{ n: 0 }}
    c.bump()
    print(c.n)
    let mut a = Account {{ balance: {start} }}
    a.withdraw({amt})
    print(a.balance)
}}
"#
        )
    })
}

/// `object` entities: a guarded field (`guarded_by (binder) predicate`),
/// invariants, and a generic object with a discharged invariant.
fn arb_object_program() -> impl Strategy<Value = String> {
    (1..10i64, 1..100i64).prop_map(|(step, val)| {
        format!(
            r#"error StaleGrant {{
    token: int
}}

class WriteGrant {{
    token: int
}}

object Authority {{
    data: int guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    invariant self.epoch >= 0

    fn grant(mut self) WriteGrant {{
        self.epoch = self.epoch + {step}
        return WriteGrant {{ token: self.epoch }}
    }}

    fn apply(mut self, grant: WriteGrant, d: int) {{
        if grant.token != self.epoch {{
            raise StaleGrant {{ token: grant.token }}
        }}
        self.data = d
    }}
}}

object Cell<T> {{
    value: T
    count: int

    fn touch(mut self) {{
        self.count = self.count + {step}
    }}

    fn get(self) T {{
        return self.value
    }}
}}

fn main() {{
    let mut auth = Authority {{ data: 0, epoch: 0 }}
    let g = auth.grant()
    auth.apply(g, {val}) catch err {{
        print(0)
    }}
    let mut cell = Cell<int> {{ value: {val}, count: 0 }}
    cell.touch()
    print(cell.get())
}}
"#
        )
    })
}

/// State-carrying error declarations: an error whose payload is a typestated
/// value in a specific (droppable) state, handled by a typed catch.
fn arb_state_error_program() -> impl Strategy<Value = String> {
    (1..100i64,).prop_map(|(id,)| {
        format!(
            r#"class Idle {{ tag: int }}
class Held {{ tag: int }}
class Revoked {{ tag: int }}

class Lease<S> {{
    id: int

    must_release Held

    fn acquire(self) Lease<Held> where S == Idle {{
        return Lease<Held> {{ id: self.id }}
    }}

    fn check(self) int where S == Held {{
        if self.id < 0 {{
            raise Degraded {{ lease: Lease<Revoked> {{ id: self.id }} }}
        }}
        return self.id
    }}

    fn release(self) Lease<Idle> where S == Held {{
        return Lease<Idle> {{ id: self.id }}
    }}
}}

error Degraded {{ lease: Lease<Revoked> }}

fn main() {{
    let l = Lease<Idle> {{ id: {id} }}
    let h = l.acquire()
    let n = h.check() catch e: Degraded {{
        print(e.lease.id)
        return
    }}
    print(n)
    let done = h.release()
    print(done.id)
}}
"#
        )
    })
}

/// `at` placement: an entity handed across a logical domain boundary.
fn arb_at_program() -> impl Strategy<Value = String> {
    (1..100i64,).prop_map(|(secret,)| {
        format!(
            r#"object Vault {{
    secret: int

    fn reveal(self) int {{
        return self.secret
    }}
}}

class PayService {{
    fn check(self, v: Vault) int {{
        return v.reveal()
    }}
}}

app A[pay: domain PayService] {{
    fn main(self) {{
        let v = Vault {{ secret: {secret} }}
        let r = at self.pay {{ check(v) }} catch -1
        print(r)
    }}
}}
"#
        )
    })
}

/// Structurally valid programs over the new grammar surface. Every value
/// this strategy produces must parse and type check.
fn arb_new_grammar_program() -> impl Strategy<Value = String> {
    prop_oneof![
        arb_typestate_program(),
        arb_contract_program(),
        arb_object_program(),
        arb_state_error_program(),
        arb_at_program(),
    ]
}

/// Near-valid mutations of the new clause shapes: each misuses exactly one
/// construct (misplaced `ensures`, binderless `guarded_by`, `old()` outside
/// contracts, `must_release` on a non-typestate class, `at` on a non-entity,
/// malformed clause jumbles). The compiler must *reject* these — or accept
/// where the grammar genuinely allows them — but never panic.
fn arb_near_valid_mutation() -> impl Strategy<Value = String> {
    (0..10usize, 1..100i64).prop_map(|(which, n)| match which {
        // generic object + invariant (rejected by design: invariants are
        // proof obligations, generic bodies check against opaque params)
        9 => format!(
            "object Cell<T> {{\n    value: T\n    count: int\n\n    invariant self.count >= 0\n\n    fn touch(mut self) {{\n        self.count = self.count + {n}\n    }}\n}}\n\nfn main() {{\n    let mut c = Cell<int> {{ value: 1, count: 0 }}\n    c.touch()\n}}\n"
        ),
        // `ensures` before the return type
        0 => format!(
            "class C {{\n    n: int\n\n    fn bump(mut self) ensures self.n == old(self.n) + 1 int {{\n        self.n = self.n + {n}\n        return self.n\n    }}\n}}\n\nfn main() {{\n    let mut c = C {{ n: 0 }}\n    print(c.bump())\n}}\n"
        ),
        // `guarded_by` without a binder
        1 => format!(
            "class Grant {{ token: int }}\n\nobject Blob {{\n    data: int guarded_by g.token == self.epoch\n    epoch: int\n}}\n\nfn main() {{\n    let b = Blob {{ data: {n}, epoch: 0 }}\n}}\n"
        ),
        // `old()` outside any contract clause
        2 => format!(
            "class C {{\n    x: int\n\n    fn f(self) int {{\n        return old(self.x) + {n}\n    }}\n}}\n\nfn main() {{\n    let c = C {{ x: 1 }}\n    print(c.f())\n}}\n"
        ),
        // `must_release` on a class with no state parameter
        3 => format!(
            "class Plain {{\n    id: int\n\n    must_release Held\n\n    fn get(self) int {{\n        return self.id\n    }}\n}}\n\nfn main() {{\n    let p = Plain {{ id: {n} }}\n    print(p.get())\n}}\n"
        ),
        // `at` over a non-entity, non-domain local
        4 => format!(
            "class Box {{ v: int }}\n\nfn main() {{\n    let b = Box {{ v: {n} }}\n    let r = at b {{ v }} catch -1\n    print(r)\n}}\n"
        ),
        // freestanding `impl T {}` (never-valid syntax found lurking in old tests)
        5 => format!(
            "class T {{ x: int }}\n\nimpl T {{\n    fn get(self) int {{\n        return self.x\n    }}\n}}\n\nfn main() {{\n    let t = T {{ x: {n} }}\n    print(t.get())\n}}\n"
        ),
        // explicit `!` return annotation (never-valid)
        6 => format!(
            "error Boom {{\n}}\n\nfn f() int! {{\n    raise Boom {{ }}\n}}\n\nfn main() {{\n    let x = f() catch {n}\n    print(x)\n}}\n"
        ),
        // `where` state constraint on a free function
        7 => format!(
            "fn f() int where S == Held {{\n    return {n}\n}}\n\nfn main() {{\n    print(f())\n}}\n"
        ),
        // clause jumble: duplicate/interleaved where + ensures
        _ => format!(
            "class Lease<S> {{\n    id: int\n\n    fn acquire(self) Lease<Held> where S == Idle ensures self.id == old(self.id) where S == Held {{\n        return Lease<Held> {{ id: {n} }}\n    }}\n}}\n\nfn main() {{\n    print({n})\n}}\n"
        ),
    })
}

/// Run the frontend-for-analysis pipeline (the same passes `pluto`'s
/// editing/check path runs, codegen excluded) on a source string.
fn typecheck_frontend(source: &str) -> Result<(), String> {
    let mut program = pluto::parse_source(source).map_err(|e| e.to_string())?;
    pluto::modules::resolve_qualified_access_single_file(&mut program).map_err(|e| e.to_string())?;
    pluto::prelude::inject_prelude(&mut program).map_err(|e| e.to_string())?;
    pluto::stages::flatten_stage_hierarchy(&mut program).map_err(|e| e.to_string())?;
    pluto::ambient::desugar_ambient(&mut program).map_err(|e| e.to_string())?;
    pluto::generic_traits::instantiate_generic_traits(&mut program).map_err(|e| e.to_string())?;
    pluto::generic_methods::hoist_generic_methods(&mut program).map_err(|e| e.to_string())?;
    pluto::contracts::validate_contracts(&mut program).map_err(|e| e.to_string())?;
    pluto::typeck::type_check(&program)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Property: structurally valid new-grammar programs parse and type
    /// check (generator sanity + no panics anywhere in the frontend).
    #[test]
    fn new_grammar_typechecks(source in arb_new_grammar_program()) {
        if let Err(e) = typecheck_frontend(&source) {
            panic!("valid new-grammar program rejected: {e}\nsource:\n{source}");
        }
    }

    /// Property: near-valid mutations of the new clause shapes never panic
    /// the frontend (rejection is expected; acceptance is tolerated for
    /// shapes the grammar genuinely allows).
    #[test]
    fn new_grammar_mutations_no_panic(source in arb_near_valid_mutation()) {
        let _ = typecheck_frontend(&source);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Property: parse → pretty-print → reparse is stable for the new
    /// grammar surface. The pretty form must itself parse, preserve
    /// declaration counts, and pretty-print to the same text (idempotence).
    #[test]
    fn new_grammar_pretty_roundtrip(source in arb_new_grammar_program()) {
        let program = pluto::parse_for_editing(&source)
            .unwrap_or_else(|e| panic!("valid new-grammar program failed to parse: {e}\nsource:\n{source}"));
        let pretty1 = pluto::pretty::pretty_print(&program, false);
        let reparsed = pluto::parse_for_editing(&pretty1)
            .unwrap_or_else(|e| panic!("pretty output failed to reparse: {e}\npretty:\n{pretty1}"));
        prop_assert_eq!(program.functions.len(), reparsed.functions.len());
        prop_assert_eq!(program.classes.len(), reparsed.classes.len());
        prop_assert_eq!(program.enums.len(), reparsed.enums.len());
        prop_assert_eq!(program.errors.len(), reparsed.errors.len());
        prop_assert_eq!(program.app.is_some(), reparsed.app.is_some());
        let pretty2 = pluto::pretty::pretty_print(&reparsed, false);
        prop_assert_eq!(&pretty1, &pretty2, "pretty-print not idempotent for:\n{}", source);
    }

    /// Property: lexer spans over new-grammar source are in-bounds and
    /// monotonic.
    #[test]
    fn new_grammar_spans_sane(source in prop_oneof![arb_new_grammar_program(), arb_near_valid_mutation()]) {
        let tokens = lex(&source).expect("new-grammar sources use only lexable syntax");
        let source_len = source.len();
        for window in tokens.windows(2) {
            prop_assert!(window[0].span.end <= window[1].span.start);
        }
        for token in &tokens {
            prop_assert!(token.span.start <= token.span.end);
            prop_assert!(token.span.end <= source_len);
        }
    }
}

proptest! {
    /// Property: Lexer spans are monotonic (non-overlapping and ordered)
    #[test]
    fn spans_are_monotonic(source in arb_simple_program()) {
        if let Ok(tokens) = lex(&source) {
            let spans: Vec<_> = tokens.iter().map(|t| t.span).collect();
            for window in spans.windows(2) {
                assert!(
                    window[0].end <= window[1].start,
                    "Spans not monotonic: {:?} followed by {:?} in source: {}",
                    window[0],
                    window[1],
                    source
                );
            }
        }
    }

    /// Property: Lex-parse roundtrip produces valid AST or error (no panics)
    #[test]
    fn lex_parse_no_panic(source in arb_simple_program()) {
        if let Ok(tokens) = lex(&source) {
            let mut parser = pluto::parser::Parser::new(&tokens, &source);
            let _ = parser.parse_program();
            // Just verify it doesn't panic - result can be Ok or Err
        }
    }

    /// Property: All token spans are within source bounds
    #[test]
    fn spans_within_bounds(source in arb_simple_program()) {
        if let Ok(tokens) = lex(&source) {
            let source_len = source.len();
            for token in &tokens {
                assert!(
                    token.span.start <= source_len,
                    "Span start {} exceeds source length {} in: {}",
                    token.span.start,
                    source_len,
                    source
                );
                assert!(
                    token.span.end <= source_len,
                    "Span end {} exceeds source length {} in: {}",
                    token.span.end,
                    source_len,
                    source
                );
            }
        }
    }

    /// Property: Lexer is deterministic (same input produces same output)
    #[test]
    fn lexer_deterministic(source in arb_simple_program()) {
        let result1 = lex(&source);
        let result2 = lex(&source);

        match (result1, result2) {
            (Ok(tokens1), Ok(tokens2)) => {
                assert_eq!(tokens1.len(), tokens2.len(), "Token count differs for: {}", source);
                for (t1, t2) in tokens1.iter().zip(tokens2.iter()) {
                    // Compare token types (not Debug repr, which may be unstable)
                    assert_eq!(
                        std::mem::discriminant(&t1.node),
                        std::mem::discriminant(&t2.node),
                        "Token type differs for: {}",
                        source
                    );
                    assert_eq!(t1.span, t2.span, "Span differs for: {}", source);
                }
            }
            (Err(_), Err(_)) => {
                // Both errored - that's fine, just verify it's consistent
            }
            _ => panic!("Lexer non-deterministic: one succeeded, one failed for: {}", source),
        }
    }

    /// Property: Parser is deterministic (same tokens produce same result)
    #[test]
    fn parser_deterministic(source in arb_simple_program()) {
        if let Ok(tokens) = lex(&source) {
            let mut parser1 = pluto::parser::Parser::new(&tokens, &source);
            let mut parser2 = pluto::parser::Parser::new(&tokens, &source);

            let result1 = parser1.parse_program();
            let result2 = parser2.parse_program();

            match (result1, result2) {
                (Ok(_ast1), Ok(_ast2)) => {
                    // Both succeeded - verify both are Ok (full structural equality hard without PartialEq)
                }
                (Err(e1), Err(e2)) => {
                    // Both errored - verify error messages match
                    assert_eq!(
                        e1.to_string(),
                        e2.to_string(),
                        "Parser error non-deterministic for: {}",
                        source
                    );
                }
                _ => panic!("Parser non-deterministic: one succeeded, one failed for: {}", source),
            }
        }
    }
}
