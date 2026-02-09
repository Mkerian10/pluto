mod common;
use common::{compile_and_run_stdout, compile_should_fail};

// ── Generic Functions ────────────────────────────────────────────

#[test]
fn generic_fn_identity_int() {
    let out = compile_and_run_stdout(
        "fn identity<T>(x: T) T {\n    return x\n}\n\nfn main() {\n    print(identity(42))\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn generic_fn_identity_string() {
    let out = compile_and_run_stdout(
        "fn identity<T>(x: T) T {\n    return x\n}\n\nfn main() {\n    print(identity(\"hello\"))\n}",
    );
    assert_eq!(out, "hello\n");
}

#[test]
fn generic_fn_identity_both() {
    let out = compile_and_run_stdout(
        "fn identity<T>(x: T) T {\n    return x\n}\n\nfn main() {\n    print(identity(42))\n    print(identity(\"hello\"))\n}",
    );
    assert_eq!(out, "42\nhello\n");
}

#[test]
fn generic_fn_two_params() {
    let out = compile_and_run_stdout(
        "fn first<A, B>(a: A, b: B) A {\n    return a\n}\n\nfn main() {\n    print(first(42, \"hello\"))\n}",
    );
    assert_eq!(out, "42\n");
}

// ── Generic Classes ──────────────────────────────────────────────

#[test]
fn generic_class_basic() {
    let out = compile_and_run_stdout(
        "class Box<T> {\n    value: T\n}\n\nfn main() {\n    let b = Box<int> { value: 42 }\n    print(b.value)\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn generic_class_string() {
    let out = compile_and_run_stdout(
        "class Box<T> {\n    value: T\n}\n\nfn main() {\n    let b = Box<string> { value: \"hello\" }\n    print(b.value)\n}",
    );
    assert_eq!(out, "hello\n");
}

#[test]
fn generic_class_two_params() {
    let out = compile_and_run_stdout(
        "class Pair<A, B> {\n    first: A\n    second: B\n}\n\nfn main() {\n    let p = Pair<int, string> { first: 42, second: \"hello\" }\n    print(p.first)\n    print(p.second)\n}",
    );
    assert_eq!(out, "42\nhello\n");
}

#[test]
fn generic_class_method() {
    let out = compile_and_run_stdout(
        "class Box<T> {\n    value: T\n\n    fn get(self) T {\n        return self.value\n    }\n}\n\nfn main() {\n    let b = Box<int> { value: 99 }\n    print(b.get())\n}",
    );
    assert_eq!(out, "99\n");
}

// ── Generic Enums ────────────────────────────────────────────────

#[test]
fn generic_enum_option() {
    let out = compile_and_run_stdout(
        "enum Option<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n    let o = Option<int>.Some { value: 42 }\n    match o {\n        Option.Some { value: v } {\n            print(v)\n        }\n        Option.None {\n            print(0)\n        }\n    }\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn generic_enum_option_none() {
    let out = compile_and_run_stdout(
        "enum Option<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n    let o = Option<int>.None\n    match o {\n        Option.Some { value: v } {\n            print(v)\n        }\n        Option.None {\n            print(0)\n        }\n    }\n}",
    );
    assert_eq!(out, "0\n");
}

// ── Multiple Instantiations ──────────────────────────────────────

#[test]
fn generic_multiple_instantiations() {
    let out = compile_and_run_stdout(
        "class Box<T> {\n    value: T\n}\n\nfn main() {\n    let a = Box<int> { value: 42 }\n    let b = Box<string> { value: \"hi\" }\n    print(a.value)\n    print(b.value)\n}",
    );
    assert_eq!(out, "42\nhi\n");
}

#[test]
fn generic_fn_with_generic_class() {
    let out = compile_and_run_stdout(
        "class Box<T> {\n    value: T\n}\n\nfn get_value(b: Box<int>) int {\n    return b.value\n}\n\nfn main() {\n    let b = Box<int> { value: 42 }\n    print(get_value(b))\n}",
    );
    assert_eq!(out, "42\n");
}
