mod common;
use common::{compile_and_run_stdout, compile_should_fail, compile_should_fail_with};

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

#[test]
fn match_binding_shadow_restore() {
    // Safety-net: a variable `x` exists before the match, a match arm binds the
    // same name `x`, uses it inside the arm body, and then the outer `x` is used
    // after the match to verify it is properly restored.
    let out = compile_and_run_stdout(
        "enum Wrapper {\n    Val { x: int }\n    Empty\n}\n\nfn main() {\n    let x = 100\n    let w = Wrapper.Val { x: 7 }\n    match w {\n        Wrapper.Val { x } {\n            print(x)\n        }\n        Wrapper.Empty {\n            print(0)\n        }\n    }\n    print(x)\n}",
    );
    assert_eq!(out, "7\n100\n");
}

#[test]
fn enum_single_variant() {
    let out = compile_and_run_stdout(
        "enum Single {\n    Only\n}\n\nfn main() {\n    let s = Single.Only\n    match s {\n        Single.Only {\n            print(1)\n        }\n    }\n}",
    );
    assert_eq!(out, "1\n");
}

#[test]
fn enum_many_variants() {
    let out = compile_and_run_stdout(
        "enum Dir {\n    N\n    S\n    E\n    W\n    NE\n}\n\nfn main() {\n    let d = Dir.NE\n    match d {\n        Dir.N { print(1) }\n        Dir.S { print(2) }\n        Dir.E { print(3) }\n        Dir.W { print(4) }\n        Dir.NE { print(5) }\n    }\n}",
    );
    assert_eq!(out, "5\n");
}

#[test]
fn enum_data_variant_multiple_fields() {
    let out = compile_and_run_stdout(
        "enum Event {\n    Move { x: int, y: int, speed: float }\n    Stop\n}\n\nfn main() {\n    let e = Event.Move { x: 10, y: 20, speed: 3.14 }\n    match e {\n        Event.Move { x, y, speed } {\n            print(x)\n            print(y)\n        }\n        Event.Stop {\n            print(0)\n        }\n    }\n}",
    );
    assert_eq!(out, "10\n20\n");
}

#[test]
fn enum_nested_match() {
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nenum Shape {\n    Circle { color: Color }\n    Square\n}\n\nfn main() {\n    let s = Shape.Circle { color: Color.Red }\n    match s {\n        Shape.Circle { color } {\n            match color {\n                Color.Red { print(\"red circle\") }\n                Color.Blue { print(\"blue circle\") }\n            }\n        }\n        Shape.Square {\n            print(\"square\")\n        }\n    }\n}",
    );
    assert_eq!(out, "red circle\n");
}

#[test]
fn enum_in_array() {
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let colors = [Color.Red, Color.Blue, Color.Red]\n    let first = colors[0]\n    match first {\n        Color.Red { print(\"red\") }\n        Color.Blue { print(\"blue\") }\n    }\n}",
    );
    assert_eq!(out, "red\n");
}

#[test]
fn enum_as_return_value() {
    let out = compile_and_run_stdout(
        "enum Status {\n    Ok { value: int }\n    Err { code: int }\n}\n\nfn compute(x: int) Status {\n    if x > 0 {\n        return Status.Ok { value: x * 10 }\n    }\n    return Status.Err { code: -1 }\n}\n\nfn main() {\n    let r = compute(3)\n    match r {\n        Status.Ok { value } { print(value) }\n        Status.Err { code } { print(code) }\n    }\n    let r2 = compute(-5)\n    match r2 {\n        Status.Ok { value } { print(value) }\n        Status.Err { code } { print(code) }\n    }\n}",
    );
    assert_eq!(out, "30\n-1\n");
}

#[test]
fn enum_non_exhaustive_rejected_data_variant() {
    compile_should_fail(
        "enum Shape {\n    Circle { r: int }\n    Rect { w: int, h: int }\n}\n\nfn main() {\n    let s = Shape.Circle { r: 5 }\n    match s {\n        Shape.Circle { r } {\n            print(r)\n        }\n    }\n}",
    );
}

#[test]
fn enum_wrong_field_in_match_rejected() {
    compile_should_fail(
        "enum Wrapper {\n    Val { x: int }\n    Empty\n}\n\nfn main() {\n    let w = Wrapper.Val { x: 5 }\n    match w {\n        Wrapper.Val { wrong } {\n            print(wrong)\n        }\n        Wrapper.Empty {\n            print(0)\n        }\n    }\n}",
    );
}

#[test]
fn enum_in_while_loop() {
    let out = compile_and_run_stdout(
        "enum Toggle {\n    On\n    Off\n}\n\nfn main() {\n    let mut state = Toggle.On\n    let mut count = 0\n    while count < 3 {\n        match state {\n            Toggle.On {\n                print(\"on\")\n                state = Toggle.Off\n            }\n            Toggle.Off {\n                print(\"off\")\n                state = Toggle.On\n            }\n        }\n        count = count + 1\n    }\n}",
    );
    assert_eq!(out, "on\noff\non\n");
}

#[test]
fn enum_passed_through_multiple_functions() {
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn describe(c: Color) string {\n    match c {\n        Color.Red { return \"red\" }\n        Color.Blue { return \"blue\" }\n    }\n}\n\nfn print_color(c: Color) {\n    print(describe(c))\n}\n\nfn main() {\n    print_color(Color.Blue)\n}",
    );
    assert_eq!(out, "blue\n");
}
