mod common;
use common::compile_and_run_stdout;

// ── Prelude is empty (Option<T> removed in favor of T? nullable types) ──
// These tests verify the prelude infrastructure still works even when empty.

#[test]
fn prelude_empty_program_compiles() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(42)\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn prelude_user_can_define_option_enum() {
    // Since Option is no longer in the prelude, users can define their own
    let out = compile_and_run_stdout(
        "enum Option<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n    let o = Option<int>.Some { value: 42 }\n    match o {\n        Option.Some { value: v } {\n            print(v)\n        }\n        Option.None {\n            print(0)\n        }\n    }\n}",
    );
    assert_eq!(out, "42\n");
}
