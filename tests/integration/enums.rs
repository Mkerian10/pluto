mod common;
use common::{compile_and_run_stdout, compile_should_fail};

#[test]
fn enum_unit_variant() {
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    match c {\n        Color.Red {\n            print(1)\n        }\n        Color.Blue {\n            print(2)\n        }\n    }\n}",
    );
    assert_eq!(out, "1\n");
}

#[test]
fn enum_data_variant() {
    let out = compile_and_run_stdout(
        "enum Status {\n    Active\n    Suspended { reason: string }\n}\n\nfn main() {\n    let s = Status.Suspended { reason: \"banned\" }\n    match s {\n        Status.Active {\n            print(\"active\")\n        }\n        Status.Suspended { reason } {\n            print(reason)\n        }\n    }\n}",
    );
    assert_eq!(out, "banned\n");
}

#[test]
fn enum_as_function_param() {
    let out = compile_and_run_stdout(
        "enum Shape {\n    Circle { radius: int }\n    Square { side: int }\n}\n\nfn describe(s: Shape) {\n    match s {\n        Shape.Circle { radius } {\n            print(radius)\n        }\n        Shape.Square { side } {\n            print(side)\n        }\n    }\n}\n\nfn main() {\n    let c = Shape.Circle { radius: 10 }\n    describe(c)\n}",
    );
    assert_eq!(out, "10\n");
}

#[test]
fn enum_non_exhaustive_rejected() {
    compile_should_fail(
        "enum Color {\n    Red\n    Blue\n    Green\n}\n\nfn main() {\n    let c = Color.Red\n    match c {\n        Color.Red {\n            print(1)\n        }\n        Color.Blue {\n            print(2)\n        }\n    }\n}",
    );
}

#[test]
fn enum_multiple_data_fields() {
    let out = compile_and_run_stdout(
        "enum Event {\n    Click { x: int, y: int }\n    Keypress { code: int }\n}\n\nfn main() {\n    let e = Event.Click { x: 100, y: 200 }\n    match e {\n        Event.Click { x, y } {\n            print(x)\n            print(y)\n        }\n        Event.Keypress { code } {\n            print(code)\n        }\n    }\n}",
    );
    assert_eq!(out, "100\n200\n");
}

#[test]
fn enum_return_from_function() {
    let out = compile_and_run_stdout(
        "enum Result {\n    Ok { value: int }\n    Err { code: int }\n}\n\nfn compute(x: int) Result {\n    if x > 0 {\n        return Result.Ok { value: x * 2 }\n    }\n    return Result.Err { code: -1 }\n}\n\nfn main() {\n    let r = compute(5)\n    match r {\n        Result.Ok { value } {\n            print(value)\n        }\n        Result.Err { code } {\n            print(code)\n        }\n    }\n}",
    );
    assert_eq!(out, "10\n");
}

#[test]
fn enum_mixed_variants() {
    let out = compile_and_run_stdout(
        "enum Token {\n    Number { val: int }\n    Plus\n    Eof\n}\n\nfn describe(t: Token) {\n    match t {\n        Token.Number { val } {\n            print(val)\n        }\n        Token.Plus {\n            print(99)\n        }\n        Token.Eof {\n            print(0)\n        }\n    }\n}\n\nfn main() {\n    describe(Token.Number { val: 42 })\n    describe(Token.Plus)\n    describe(Token.Eof)\n}",
    );
    assert_eq!(out, "42\n99\n0\n");
}
