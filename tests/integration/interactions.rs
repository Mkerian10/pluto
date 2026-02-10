mod common;
use common::compile_and_run_stdout;

// ── Generics + Closures ─────────────────────────────────────────────────────

#[test]
fn generic_fn_taking_closure() {
    let out = compile_and_run_stdout(
        "fn apply<T>(f: fn(T) T, x: T) T {\n    return f(x)\n}\n\nfn main() {\n    let double = (x: int) => x * 2\n    print(apply(double, 5))\n}",
    );
    assert_eq!(out, "10\n");
}

#[test]
fn closure_returning_generic_class() {
    let out = compile_and_run_stdout(
        "class Box<T> {\n    value: T\n}\n\nfn main() {\n    let make_box = (x: int) => Box<int> { value: x }\n    let b = make_box(42)\n    print(b.value)\n}",
    );
    assert_eq!(out, "42\n");
}

// ── Generics + Errors ───────────────────────────────────────────────────────

#[test]
fn error_propagation_through_generic() {
    let out = compile_and_run_stdout(
        "error MathError {\n    msg: string\n}\n\nfn safe_div(a: int, b: int) int {\n    if b == 0 {\n        raise MathError { msg: \"zero\" }\n    }\n    return a / b\n}\n\nfn identity<T>(x: T) T {\n    return x\n}\n\nfn main() {\n    let r = safe_div(10, 0) catch -1\n    print(identity(r))\n}",
    );
    assert_eq!(out, "-1\n");
}

// ── String interpolation + expressions ──────────────────────────────────────

#[test]
fn string_interp_with_function_call() {
    let out = compile_and_run_stdout(
        "fn double(x: int) int {\n    return x * 2\n}\n\nfn main() {\n    print(\"result: {double(5)}\")\n}",
    );
    assert_eq!(out, "result: 10\n");
}

#[test]
fn string_interp_with_nested_calls() {
    let out = compile_and_run_stdout(
        "fn double(x: int) int {\n    return x * 2\n}\n\nfn main() {\n    print(\"result: {double(double(3))}\")\n}",
    );
    assert_eq!(out, "result: 12\n");
}

#[test]
fn string_interp_with_field_access() {
    let out = compile_and_run_stdout(
        "class Point {\n    x: int\n    y: int\n}\n\nfn main() {\n    let p = Point { x: 10, y: 20 }\n    print(\"x={p.x} y={p.y}\")\n}",
    );
    assert_eq!(out, "x=10 y=20\n");
}

// ── For loops + closures ────────────────────────────────────────────────────

#[test]
fn closure_capturing_loop_variable() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut sum = 0\n    let arr = [1, 2, 3]\n    for item in arr {\n        let add = () => item\n        sum = sum + add()\n    }\n    print(sum)\n}",
    );
    assert_eq!(out, "6\n");
}

// ── Chained stdlib operations ───────────────────────────────────────────────

#[test]
fn array_map_with_closure() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let arr = [1, 2, 3, 4, 5]\n    let mut result = 0\n    for item in arr {\n        let double = (x: int) => x * 2\n        result = result + double(item)\n    }\n    print(result)\n}",
    );
    assert_eq!(out, "30\n");
}

// ── Enum + generic interaction ──────────────────────────────────────────────

#[test]
fn generic_class_with_enum() {
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nclass Box<T> {\n    value: T\n}\n\nfn main() {\n    let b = Box<Color> { value: Color.Red }\n    match b.value {\n        Color.Red {\n            print(\"red\")\n        }\n        Color.Blue {\n            print(\"blue\")\n        }\n    }\n}",
    );
    assert_eq!(out, "red\n");
}

// ── Boundary conditions ─────────────────────────────────────────────────────

#[test]
fn long_string_concatenation() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut s = \"\"\n    let mut i = 0\n    while i < 100 {\n        s = s + \"a\"\n        i = i + 1\n    }\n    print(s.len())\n}",
    );
    assert_eq!(out, "100\n");
}
