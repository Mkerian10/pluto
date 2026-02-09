mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

// ── Option<T> Usage ─────────────────────────────────────────────

#[test]
fn prelude_option_some() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let o = Option<int>.Some { value: 42 }\n    match o {\n        Option.Some { value: v } {\n            print(v)\n        }\n        Option.None {\n            print(0)\n        }\n    }\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn prelude_option_none() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let o = Option<int>.None\n    match o {\n        Option.Some { value: v } {\n            print(v)\n        }\n        Option.None {\n            print(0)\n        }\n    }\n}",
    );
    assert_eq!(out, "0\n");
}

#[test]
fn prelude_option_string() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let o = Option<string>.Some { value: \"hello\" }\n    match o {\n        Option.Some { value: v } {\n            print(v)\n        }\n        Option.None {\n            print(\"none\")\n        }\n    }\n}",
    );
    assert_eq!(out, "hello\n");
}

// ── Result<T, E> Usage ──────────────────────────────────────────

#[test]
fn prelude_result_ok() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let r = Result<int, string>.Ok { value: 42 }\n    match r {\n        Result.Ok { value: v } {\n            print(v)\n        }\n        Result.Err { err: e } {\n            print(e)\n        }\n    }\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn prelude_result_err() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let r = Result<int, string>.Err { err: \"bad\" }\n    match r {\n        Result.Ok { value: v } {\n            print(v)\n        }\n        Result.Err { err: e } {\n            print(e)\n        }\n    }\n}",
    );
    assert_eq!(out, "bad\n");
}

// ── Prelude types as function params/returns ─────────────────────

#[test]
fn prelude_option_as_param() {
    let out = compile_and_run_stdout(
        "fn unwrap_or(o: Option<int>, default: int) int {\n    match o {\n        Option.Some { value: v } {\n            return v\n        }\n        Option.None {\n            return default\n        }\n    }\n}\n\nfn main() {\n    let a = Option<int>.Some { value: 10 }\n    let b = Option<int>.None\n    print(unwrap_or(a, 0))\n    print(unwrap_or(b, 99))\n}",
    );
    assert_eq!(out, "10\n99\n");
}

#[test]
fn prelude_option_as_return() {
    let out = compile_and_run_stdout(
        "fn find(x: int) Option<int> {\n    if x > 0 {\n        return Option<int>.Some { value: x }\n    }\n    return Option<int>.None\n}\n\nfn main() {\n    let a = find(5)\n    let b = find(0)\n    match a {\n        Option.Some { value: v } {\n            print(v)\n        }\n        Option.None {\n            print(0)\n        }\n    }\n    match b {\n        Option.Some { value: v } {\n            print(v)\n        }\n        Option.None {\n            print(0)\n        }\n    }\n}",
    );
    assert_eq!(out, "5\n0\n");
}

#[test]
fn prelude_option_in_interpolation() {
    let out = compile_and_run_stdout(
        "fn unwrap(o: Option<int>) int {\n    match o {\n        Option.Some { value: v } {\n            return v\n        }\n        Option.None {\n            return 0\n        }\n    }\n}\n\nfn main() {\n    let o = Option<int>.Some { value: 42 }\n    print(\"got {unwrap(o)}\")\n}",
    );
    assert_eq!(out, "got 42\n");
}

// ── Conflict Detection ──────────────────────────────────────────

#[test]
fn prelude_cannot_redefine_option() {
    compile_should_fail_with(
        "enum Option<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n}",
        "conflicts with built-in prelude type",
    );
}

#[test]
fn prelude_cannot_redefine_result() {
    compile_should_fail_with(
        "enum Result<T, E> {\n    Ok { value: T }\n    Err { err: E }\n}\n\nfn main() {\n}",
        "conflicts with built-in prelude type",
    );
}

#[test]
fn prelude_cannot_shadow_with_class() {
    compile_should_fail_with(
        "class Option {\n    value: int\n}\n\nfn main() {\n}",
        "conflicts with built-in prelude type",
    );
}

#[test]
fn prelude_cannot_shadow_with_trait() {
    compile_should_fail_with(
        "trait Result {\n    fn get(self) int\n}\n\nfn main() {\n}",
        "conflicts with built-in prelude type",
    );
}

#[test]
fn prelude_cannot_shadow_with_error() {
    compile_should_fail_with(
        "error Option {\n    msg: string\n}\n\nfn main() {\n}",
        "conflicts with built-in prelude type",
    );
}

#[test]
fn prelude_cannot_shadow_result_with_error() {
    compile_should_fail_with(
        "error Result {\n    msg: string\n}\n\nfn main() {\n}",
        "conflicts with built-in prelude type",
    );
}
