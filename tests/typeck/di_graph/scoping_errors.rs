//! DI lifecycle rules (rfc-manual-bracket-deps.md, issue #125).
//!
//! Effective lifecycle is inferred: a class depending on a scoped class is
//! itself scoped. The app's startup graph acts as the root scope, so it may
//! hold scoped-effective classes — but only auto-creatable ones (all fields
//! injected). Reaching a scoped class with data fields at startup is a
//! captive dependency: no seed exists there. Scoped instances are created by scope
//! blocks — seeds provide non-injected fields, the compiler wires deps.
//! Transient semantics are deferred (the keyword parses; rules TBD).
#[path = "../common.rs"]
mod common;
use common::{compile_and_run, compile_and_run_stdout, compile_should_fail_with};

// ── Legal: shorter-lived depending on longer-lived ───────────────────────────

#[test]
fn scoped_depends_singleton() {
    let out = compile_and_run_stdout(
        "class Logger {\n    fn log(self) int {\n        return 7\n    }\n}\n\nscoped class Handler[logger: Logger] {\n    request_id: int\n\n    fn process(self) int {\n        return self.logger.log() + self.request_id\n    }\n}\n\napp MyApp[logger: Logger] {\n    fn main(self) {\n        scope(Handler { request_id: 100 }) |h: Handler| {\n            print(h.process())\n        }\n    }\n}",
    );
    assert_eq!(out.trim(), "107");
}

#[test]
fn inferred_scoped_class_works_in_scope_block() {
    // B is inferred scoped (depends on scoped A); usable via scope blocks
    let out = compile_and_run_stdout(
        "scoped class A {\n    x: int\n}\n\nclass B[a: A] {\n    fn get(self) int {\n        return self.a.x\n    }\n}\n\nfn main(){\n    scope(A { x: 42 }) |b: B| {\n        print(b.get())\n    }\n}",
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn scoped_class_constructible_as_plain_value() {
    // `scoped` governs DI wiring; a dep-less scoped class is still an
    // ordinary constructible value (that's what seeds are)
    let out = compile_and_run_stdout(
        "scoped class A {\n    x: int\n}\n\nfn main(){\n    let a = A { x: 1 }\n    print(a.x)\n}",
    );
    assert_eq!(out.trim(), "1");
}

// ── Captive dependencies rejected ────────────────────────────────────────────

#[test]
fn app_depends_scoped() {
    compile_should_fail_with(
        "scoped class A {\n    x: int\n}\n\napp MyApp[a: A] {\n    fn main(self){}\n}",
        "captive dependency",
    );
}

#[test]
fn singleton_depends_scoped_wired_into_app() {
    compile_should_fail_with(
        "scoped class A {\n    x: int\n}\n\nclass B[a: A] {}\n\napp MyApp[b: B] {\n    fn main(self){}\n}",
        "captive dependency",
    );
}

#[test]
fn scope_chain_violation() {
    // Scoped-ness propagates transitively before the captive check
    compile_should_fail_with(
        "scoped class A {\n    x: int\n}\n\nclass B[a: A] {}\n\nclass C[b: B] {}\n\napp MyApp[c: C] {\n    fn main(self){}\n}",
        "captive dependency",
    );
}

#[test]
fn mixed_scope_deps() {
    // One scoped dep is enough to make the class scoped
    compile_should_fail_with(
        "class A {}\n\nscoped class B {\n    x: int\n}\n\nclass C[a: A, b: B] {}\n\napp MyApp[c: C] {\n    fn main(self){}\n}",
        "captive dependency",
    );
}

#[test]
fn captive_error_names_the_culprit() {
    compile_should_fail_with(
        "scoped class Ctx {\n    id: int\n}\n\nclass Service[ctx: Ctx] {}\n\napp MyApp[svc: Service] {\n    fn main(self){}\n}",
        "scoped class 'Ctx' has non-injected fields and needs a seed",
    );
}

// ── Unwired scoped-effective classes are fine ────────────────────────────────

#[test]
fn inferred_scoped_not_wired_into_app_ok() {
    // B becomes scoped by inference but the app never holds it — legal
    assert_eq!(
        compile_and_run(
            "scoped class A {\n    x: int\n}\n\nclass B[a: A] {}\n\napp MyApp {\n    fn main(self){}\n}"
        ),
        0
    );
}

// ── Transient: keyword parses, semantics deferred ────────────────────────────

#[test]
fn transient_keyword_parses() {
    assert_eq!(
        compile_and_run(
            "transient class A {\n    x: int\n}\n\napp MyApp {\n    fn main(self){}\n}"
        ),
        0
    );
}

#[test]
#[ignore] // Deferred: transient lifecycle rules (transient vs scoped/singleton mixing) are not yet specified
fn transient_depends_scoped() {
    compile_should_fail_with(
        "scoped class A {\n    x: int\n}\n\ntransient class B[a: A] {}\n\napp MyApp[b: B] {\n    fn main(self){}\n}",
        "scope",
    );
}

#[test]
#[ignore] // Deferred: transient lifecycle rules (transient vs scoped/singleton mixing) are not yet specified
fn singleton_depends_transient() {
    compile_should_fail_with(
        "transient class A {\n    x: int\n}\n\nclass B[a: A] {}\n\napp MyApp[b: B] {\n    fn main(self){}\n}",
        "scope",
    );
}
