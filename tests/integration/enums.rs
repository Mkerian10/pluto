mod common;
use common::{compile_and_run_stdout, compile_should_fail, compile_should_fail_with};

// ═══════════════════════════════════════════════════════════════════════════════
// BASIC CONSTRUCTION & MATCHING
// ═══════════════════════════════════════════════════════════════════════════════

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
    compile_should_fail_with(
        "enum Color {\n    Red\n    Blue\n    Green\n}\n\nfn main() {\n    let c = Color.Red\n    match c {\n        Color.Red {\n            print(1)\n        }\n        Color.Blue {\n            print(2)\n        }\n    }\n}",
        "non-exhaustive match",
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
    let out = compile_and_run_stdout(
        "enum Wrapper {\n    Val { x: int }\n    Empty\n}\n\nfn main() {\n    let x = 100\n    let w = Wrapper.Val { x: 7 }\n    match w {\n        Wrapper.Val { x } {\n            print(x)\n        }\n        Wrapper.Empty {\n            print(0)\n        }\n    }\n    print(x)\n}",
    );
    assert_eq!(out, "7\n100\n");
}

// ═══════════════════════════════════════════════════════════════════════════════
// NEW BASIC TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn enum_single_variant() {
    // Edge case: an enum with only one variant. Match is trivially exhaustive.
    let out = compile_and_run_stdout(
        "enum Singleton {\n    Only { value: int }\n}\n\nfn main() {\n    let s = Singleton.Only { value: 42 }\n    match s {\n        Singleton.Only { value } {\n            print(value)\n        }\n    }\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn enum_many_variants() {
    // Stress test: 8 variants, exhaustive match
    let out = compile_and_run_stdout(
        "enum Direction {\n    North\n    South\n    East\n    West\n    NorthEast\n    NorthWest\n    SouthEast\n    SouthWest\n}\n\nfn main() {\n    let d = Direction.NorthEast\n    match d {\n        Direction.North { print(1) }\n        Direction.South { print(2) }\n        Direction.East { print(3) }\n        Direction.West { print(4) }\n        Direction.NorthEast { print(5) }\n        Direction.NorthWest { print(6) }\n        Direction.SouthEast { print(7) }\n        Direction.SouthWest { print(8) }\n    }\n}",
    );
    assert_eq!(out, "5\n");
}

#[test]
fn enum_multiple_data_variants_different_counts() {
    // Variants with 0, 1, 2, and 3 fields
    let out = compile_and_run_stdout(
        "enum Expr {\n    Lit { value: int }\n    Add { left: int, right: int }\n    Cond { a: int, b: int, c: int }\n    Eof\n}\n\nfn main() {\n    let e = Expr.Cond { a: 1, b: 2, c: 3 }\n    match e {\n        Expr.Lit { value } { print(value) }\n        Expr.Add { left, right } { print(left + right) }\n        Expr.Cond { a, b, c } { print(a + b + c) }\n        Expr.Eof { print(0) }\n    }\n}",
    );
    assert_eq!(out, "6\n");
}

#[test]
fn enum_same_field_name_across_variants() {
    // Two variants with the same field name "value"
    let out = compile_and_run_stdout(
        "enum Container {\n    IntVal { value: int }\n    StrVal { value: string }\n}\n\nfn main() {\n    let a = Container.IntVal { value: 42 }\n    let b = Container.StrVal { value: \"hello\" }\n    match a {\n        Container.IntVal { value } { print(value) }\n        Container.StrVal { value } { print(value) }\n    }\n    match b {\n        Container.IntVal { value } { print(value) }\n        Container.StrVal { value } { print(value) }\n    }\n}",
    );
    assert_eq!(out, "42\nhello\n");
}

#[test]
fn enum_two_enums_in_same_program() {
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nenum Shape {\n    Circle { radius: int }\n    Square { side: int }\n}\n\nfn main() {\n    let c = Color.Red\n    let s = Shape.Circle { radius: 5 }\n    match c {\n        Color.Red { print(1) }\n        Color.Blue { print(2) }\n    }\n    match s {\n        Shape.Circle { radius } { print(radius) }\n        Shape.Square { side } { print(side) }\n    }\n}",
    );
    assert_eq!(out, "1\n5\n");
}

#[test]
fn enum_in_if_else_branches() {
    // If-expressions are now supported
    let out = compile_and_run_stdout(
        "enum Result {\n    Ok { value: int }\n    Err { code: int }\n}\n\nfn main() {\n    let flag = true\n    let r = if flag {\n        Result.Ok { value: 10 }\n    } else {\n        Result.Err { code: -1 }\n    }\n    match r {\n        Result.Ok { value } { print(value) }\n        Result.Err { code } { print(code) }\n    }\n}",
    );
    assert_eq!(out, "10\n");
}

#[test]
fn enum_variant_order_independence() {
    // Match arms in different order than variant declaration
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Green\n    Blue\n}\n\nfn main() {\n    let c = Color.Green\n    match c {\n        Color.Blue { print(3) }\n        Color.Red { print(1) }\n        Color.Green { print(2) }\n    }\n}",
    );
    assert_eq!(out, "2\n");
}

#[test]
fn enum_match_on_function_result() {
    // Match directly on a function call result (not a variable)
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn get_color() Color {\n    return Color.Blue\n}\n\nfn main() {\n    match get_color() {\n        Color.Red { print(1) }\n        Color.Blue { print(2) }\n    }\n}",
    );
    assert_eq!(out, "2\n");
}

// ═══════════════════════════════════════════════════════════════════════════════
// FIELD TYPE VARIATIONS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn enum_bool_field() {
    let out = compile_and_run_stdout(
        "enum Gate {\n    Open { locked: bool }\n    Closed\n}\n\nfn main() {\n    let g = Gate.Open { locked: false }\n    match g {\n        Gate.Open { locked } {\n            if locked {\n                print(1)\n            } else {\n                print(0)\n            }\n        }\n        Gate.Closed { print(2) }\n    }\n}",
    );
    assert_eq!(out, "0\n");
}

#[test]
fn enum_float_field() {
    let out = compile_and_run_stdout(
        "enum Measurement {\n    Length { meters: float }\n    Weight { kg: float }\n}\n\nfn main() {\n    let m = Measurement.Length { meters: 3.14 }\n    match m {\n        Measurement.Length { meters } { print(meters) }\n        Measurement.Weight { kg } { print(kg) }\n    }\n}",
    );
    assert_eq!(out, "3.140000\n");
}

#[test]
fn enum_multiple_string_fields() {
    let out = compile_and_run_stdout(
        "enum Entry {\n    Full { first: string, last: string }\n    Anonymous\n}\n\nfn main() {\n    let e = Entry.Full { first: \"John\", last: \"Doe\" }\n    match e {\n        Entry.Full { first, last } {\n            print(first)\n            print(last)\n        }\n        Entry.Anonymous { print(\"anon\") }\n    }\n}",
    );
    assert_eq!(out, "John\nDoe\n");
}

#[test]
fn enum_mixed_type_fields() {
    // Variant with int, string, and bool together
    let out = compile_and_run_stdout(
        "enum Record {\n    Data { id: int, name: string, active: bool }\n    Empty\n}\n\nfn main() {\n    let r = Record.Data { id: 42, name: \"test\", active: true }\n    match r {\n        Record.Data { id, name, active } {\n            print(id)\n            print(name)\n            print(active)\n        }\n        Record.Empty { print(0) }\n    }\n}",
    );
    assert_eq!(out, "42\ntest\ntrue\n");
}

#[test]
fn enum_large_variant() {
    // Variant with 5 fields
    let out = compile_and_run_stdout(
        "enum BigData {\n    Record { a: int, b: int, c: int, d: int, e: int }\n    None\n}\n\nfn main() {\n    let r = BigData.Record { a: 1, b: 2, c: 3, d: 4, e: 5 }\n    match r {\n        BigData.Record { a, b, c, d, e } {\n            print(a + b + c + d + e)\n        }\n        BigData.None { print(0) }\n    }\n}",
    );
    assert_eq!(out, "15\n");
}

#[test]
fn enum_array_field() {
    let out = compile_and_run_stdout(
        "enum Container {\n    Items { data: [int] }\n    Empty\n}\n\nfn main() {\n    let c = Container.Items { data: [10, 20, 30] }\n    match c {\n        Container.Items { data } {\n            print(data[0])\n            print(data[1])\n            print(data[2])\n        }\n        Container.Empty { print(0) }\n    }\n}",
    );
    assert_eq!(out, "10\n20\n30\n");
}

#[test]
fn enum_class_field() {
    // Enum variant fields referencing class types are now supported with two-pass type registration
    let out = compile_and_run_stdout(
        "class Point {\n    x: int\n    y: int\n}\n\nenum Shape {\n    Located { pos: Point }\n    Origin\n}\n\nfn main() {\n    let p = Point { x: 10, y: 20 }\n    let s = Shape.Located { pos: p }\n    match s {\n        Shape.Located { pos } {\n            print(pos.x)\n            print(pos.y)\n        }\n        Shape.Origin { print(0) }\n    }\n}",
    );
    assert_eq!(out, "10\n20\n");
}

#[test]
fn enum_nested_enum_field() {
    // Variant field is another enum type
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nenum Item {\n    Colored { color: Color, label: string }\n    Plain\n}\n\nfn main() {\n    let item = Item.Colored { color: Color.Red, label: \"apple\" }\n    match item {\n        Item.Colored { color, label } {\n            print(label)\n            match color {\n                Color.Red { print(\"red\") }\n                Color.Blue { print(\"blue\") }\n            }\n        }\n        Item.Plain { print(\"plain\") }\n    }\n}",
    );
    assert_eq!(out, "apple\nred\n");
}

// ═══════════════════════════════════════════════════════════════════════════════
// ENUM IN VARIOUS POSITIONS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn enum_in_array() {
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Green\n    Blue\n}\n\nfn describe(c: Color) {\n    match c {\n        Color.Red { print(1) }\n        Color.Green { print(2) }\n        Color.Blue { print(3) }\n    }\n}\n\nfn main() {\n    let colors = [Color.Red, Color.Green, Color.Blue]\n    for c in colors {\n        describe(c)\n    }\n}",
    );
    assert_eq!(out, "1\n2\n3\n");
}

#[test]
fn enum_as_class_field() {
    let out = compile_and_run_stdout(
        "enum Status {\n    Active\n    Inactive\n}\n\nclass User {\n    name: string\n    status: Status\n}\n\nfn main() {\n    let u = User { name: \"Alice\", status: Status.Active }\n    match u.status {\n        Status.Active { print(\"active\") }\n        Status.Inactive { print(\"inactive\") }\n    }\n}",
    );
    assert_eq!(out, "active\n");
}

#[test]
fn enum_passed_through_three_calls() {
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn step1(c: Color) Color {\n    return step2(c)\n}\n\nfn step2(c: Color) Color {\n    return step3(c)\n}\n\nfn step3(c: Color) Color {\n    return c\n}\n\nfn main() {\n    let c = step1(Color.Blue)\n    match c {\n        Color.Red { print(1) }\n        Color.Blue { print(2) }\n    }\n}",
    );
    assert_eq!(out, "2\n");
}

#[test]
fn enum_construction_as_function_arg() {
    // Construct enum directly in function argument position
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn describe(c: Color) {\n    match c {\n        Color.Red { print(1) }\n        Color.Blue { print(2) }\n    }\n}\n\nfn main() {\n    describe(Color.Red)\n    describe(Color.Blue)\n}",
    );
    assert_eq!(out, "1\n2\n");
}

#[test]
fn enum_construction_in_return() {
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn make_red() Color {\n    return Color.Red\n}\n\nfn make_blue() Color {\n    return Color.Blue\n}\n\nfn main() {\n    match make_red() {\n        Color.Red { print(1) }\n        Color.Blue { print(2) }\n    }\n    match make_blue() {\n        Color.Red { print(1) }\n        Color.Blue { print(2) }\n    }\n}",
    );
    assert_eq!(out, "1\n2\n");
}

#[test]
fn enum_in_array_literal() {
    // Construct enums directly inside array literal
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let colors = [Color.Red, Color.Blue, Color.Red]\n    match colors[1] {\n        Color.Red { print(1) }\n        Color.Blue { print(2) }\n    }\n}",
    );
    assert_eq!(out, "2\n");
}

#[test]
fn enum_in_for_loop() {
    let out = compile_and_run_stdout(
        "enum Op {\n    Add { n: int }\n    Mul { n: int }\n    Noop\n}\n\nfn main() {\n    let ops = [Op.Add { n: 5 }, Op.Mul { n: 3 }, Op.Noop]\n    let mut result = 0\n    for op in ops {\n        match op {\n            Op.Add { n } { result = result + n }\n            Op.Mul { n } { result = result * n }\n            Op.Noop { result = result }\n        }\n    }\n    print(result)\n}",
    );
    assert_eq!(out, "15\n");
}

#[test]
fn enum_in_while_loop() {
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n    Green\n}\n\nfn main() {\n    let colors = [Color.Red, Color.Blue, Color.Green]\n    let i = 0\n    while i < 3 {\n        match colors[i] {\n            Color.Red { print(1) }\n            Color.Blue { print(2) }\n            Color.Green { print(3) }\n        }\n        i = i + 1\n    }\n}",
    );
    assert_eq!(out, "1\n2\n3\n");
}

// ═══════════════════════════════════════════════════════════════════════════════
// MATCH EDGE CASES
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn match_all_arms_return() {
    // When all match arms return, the function terminates
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn value(c: Color) int {\n    match c {\n        Color.Red { return 1 }\n        Color.Blue { return 2 }\n    }\n}\n\nfn main() {\n    print(value(Color.Red))\n    print(value(Color.Blue))\n}",
    );
    assert_eq!(out, "1\n2\n");
}

#[test]
fn match_some_arms_return() {
    // Some arms return, others fall through to code after match
    let out = compile_and_run_stdout(
        "enum Mode {\n    Fast\n    Slow\n}\n\nfn process(m: Mode) int {\n    match m {\n        Mode.Fast { return 100 }\n        Mode.Slow {\n            print(\"slow path\")\n        }\n    }\n    return 1\n}\n\nfn main() {\n    print(process(Mode.Fast))\n    print(process(Mode.Slow))\n}",
    );
    assert_eq!(out, "100\nslow path\n1\n");
}

#[test]
fn match_nested() {
    // Match inside another match arm
    let out = compile_and_run_stdout(
        "enum Outer {\n    A\n    B\n}\n\nenum Inner {\n    X\n    Y\n}\n\nfn main() {\n    let o = Outer.A\n    let i = Inner.Y\n    match o {\n        Outer.A {\n            match i {\n                Inner.X { print(1) }\n                Inner.Y { print(2) }\n            }\n        }\n        Outer.B {\n            print(3)\n        }\n    }\n}",
    );
    assert_eq!(out, "2\n");
}

#[test]
fn match_binding_plus_outer_var() {
    // Match arm uses both a binding and an outer variable
    let out = compile_and_run_stdout(
        "enum Wrapper {\n    Val { x: int }\n    Empty\n}\n\nfn main() {\n    let multiplier = 10\n    let w = Wrapper.Val { x: 5 }\n    match w {\n        Wrapper.Val { x } {\n            print(x * multiplier)\n        }\n        Wrapper.Empty {\n            print(0)\n        }\n    }\n}",
    );
    assert_eq!(out, "50\n");
}

#[test]
fn match_binding_rename() {
    // Use rename syntax: { field: local_name }
    let out = compile_and_run_stdout(
        "enum Pair {\n    Data { first: int, second: int }\n    Empty\n}\n\nfn main() {\n    let p = Pair.Data { first: 10, second: 20 }\n    match p {\n        Pair.Data { first: a, second: b } {\n            print(a)\n            print(b)\n        }\n        Pair.Empty { print(0) }\n    }\n}",
    );
    assert_eq!(out, "10\n20\n");
}

#[test]
fn match_multiple_bindings_all_used() {
    let out = compile_and_run_stdout(
        "enum Vec3 {\n    Point { x: int, y: int, z: int }\n    Zero\n}\n\nfn main() {\n    let v = Vec3.Point { x: 1, y: 2, z: 3 }\n    match v {\n        Vec3.Point { x, y, z } {\n            print(x)\n            print(y)\n            print(z)\n        }\n        Vec3.Zero { print(0) }\n    }\n}",
    );
    assert_eq!(out, "1\n2\n3\n");
}

#[test]
fn match_on_class_field() {
    // Scrutinee is a field access expression
    let out = compile_and_run_stdout(
        "enum Status {\n    Active\n    Inactive\n}\n\nclass Item {\n    name: string\n    status: Status\n}\n\nfn main() {\n    let item = Item { name: \"widget\", status: Status.Inactive }\n    match item.status {\n        Status.Active { print(\"active\") }\n        Status.Inactive { print(\"inactive\") }\n    }\n}",
    );
    assert_eq!(out, "inactive\n");
}

#[test]
fn match_two_times_same_var() {
    // Match the same variable twice
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    match c {\n        Color.Red { print(1) }\n        Color.Blue { print(2) }\n    }\n    match c {\n        Color.Red { print(3) }\n        Color.Blue { print(4) }\n    }\n}",
    );
    assert_eq!(out, "1\n3\n");
}

#[test]
fn match_two_different_enums() {
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nenum Size {\n    Small\n    Large\n}\n\nfn main() {\n    let c = Color.Red\n    let s = Size.Large\n    match c {\n        Color.Red { print(\"red\") }\n        Color.Blue { print(\"blue\") }\n    }\n    match s {\n        Size.Small { print(\"small\") }\n        Size.Large { print(\"large\") }\n    }\n}",
    );
    assert_eq!(out, "red\nlarge\n");
}

#[test]
fn match_with_function_calls_in_arms() {
    let out = compile_and_run_stdout(
        "enum Action {\n    Greet { name: string }\n    Count { n: int }\n}\n\nfn say_hello(name: string) {\n    print(name)\n}\n\nfn double(n: int) int {\n    return n * 2\n}\n\nfn main() {\n    let a = Action.Greet { name: \"world\" }\n    let b = Action.Count { n: 21 }\n    match a {\n        Action.Greet { name } { say_hello(name) }\n        Action.Count { n } { print(double(n)) }\n    }\n    match b {\n        Action.Greet { name } { say_hello(name) }\n        Action.Count { n } { print(double(n)) }\n    }\n}",
    );
    assert_eq!(out, "world\n42\n");
}

#[test]
fn match_shadow_different_across_arms() {
    // Each arm shadows the same outer variable `val` differently
    let out = compile_and_run_stdout(
        "enum Choice {\n    A { val: int }\n    B { val: int }\n}\n\nfn main() {\n    let val = 999\n    let c = Choice.A { val: 10 }\n    match c {\n        Choice.A { val } { print(val) }\n        Choice.B { val } { print(val) }\n    }\n    print(val)\n}",
    );
    assert_eq!(out, "10\n999\n");
}

#[test]
fn match_break_continue_in_loop() {
    // Test break/continue inside match arms within a for loop
    let out = compile_and_run_stdout(
        "enum Action {\n    Skip\n    Process { n: int }\n    Stop\n}\n\nfn main() {\n    let actions = [Action.Process { n: 1 }, Action.Skip, Action.Process { n: 2 }, Action.Stop, Action.Process { n: 3 }]\n    let mut total = 0\n    for a in actions {\n        match a {\n            Action.Skip { continue }\n            Action.Process { n } { total = total + n }\n            Action.Stop { break }\n        }\n    }\n    print(total)\n}",
    );
    assert_eq!(out, "3\n");
}

#[test]
fn match_deeply_nested_enum_matching() {
    // Three levels: match outer, match inner, use innermost value
    let out = compile_and_run_stdout(
        "enum Inner {\n    Val { n: int }\n    None\n}\n\nenum Middle {\n    Wrap { inner: Inner }\n    None\n}\n\nenum Outer {\n    Box { middle: Middle }\n    None\n}\n\nfn main() {\n    let o = Outer.Box { middle: Middle.Wrap { inner: Inner.Val { n: 42 } } }\n    match o {\n        Outer.Box { middle } {\n            match middle {\n                Middle.Wrap { inner } {\n                    match inner {\n                        Inner.Val { n } { print(n) }\n                        Inner.None { print(-1) }\n                    }\n                }\n                Middle.None { print(-2) }\n            }\n        }\n        Outer.None { print(-3) }\n    }\n}",
    );
    assert_eq!(out, "42\n");
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLOSURE INTERACTION
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn enum_closure_capture() {
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    let f = () => {\n        match c {\n            Color.Red { print(1) }\n            Color.Blue { print(2) }\n        }\n    }\n    f()\n}",
    );
    assert_eq!(out, "1\n");
}

#[test]
fn enum_as_closure_param() {
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let describe = (c: Color) => {\n        match c {\n            Color.Red { print(1) }\n            Color.Blue { print(2) }\n        }\n    }\n    describe(Color.Red)\n    describe(Color.Blue)\n}",
    );
    assert_eq!(out, "1\n2\n");
}

#[test]
fn enum_as_closure_return() {
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let make_red = () => {\n        return Color.Red\n    }\n    let c = make_red()\n    match c {\n        Color.Red { print(1) }\n        Color.Blue { print(2) }\n    }\n}",
    );
    assert_eq!(out, "1\n");
}

#[test]
fn enum_match_in_closure_body() {
    // COMPILER GAP: closures with match + return don't properly infer return type.
    // The closure body type infers as void even though all arms return int.
    compile_should_fail_with(
        "enum Op {\n    Add { a: int, b: int }\n    Neg { n: int }\n}\n\nfn main() {\n    let eval = (op: Op) => {\n        match op {\n            Op.Add { a, b } { return a + b }\n            Op.Neg { n } { return 0 - n }\n        }\n    }\n    print(eval(Op.Add { a: 3, b: 4 }))\n    print(eval(Op.Neg { n: 5 }))\n}",
        "return type mismatch",
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// GENERIC ENUMS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn generic_enum_basic() {
    let out = compile_and_run_stdout(
        "enum MyOption<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n    let x = MyOption<int>.Some { value: 42 }\n    match x {\n        MyOption.Some { value } { print(value) }\n        MyOption.None { print(0) }\n    }\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn generic_enum_multiple_instantiations() {
    // Same generic enum with int and string
    let out = compile_and_run_stdout(
        "enum MyOption<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n    let a = MyOption<int>.Some { value: 42 }\n    let b = MyOption<string>.Some { value: \"hello\" }\n    match a {\n        MyOption.Some { value } { print(value) }\n        MyOption.None { print(0) }\n    }\n    match b {\n        MyOption.Some { value } { print(value) }\n        MyOption.None { print(\"none\") }\n    }\n}",
    );
    assert_eq!(out, "42\nhello\n");
}

#[test]
fn generic_enum_two_type_params() {
    let out = compile_and_run_stdout(
        "enum Either<A, B> {\n    Left { value: A }\n    Right { value: B }\n}\n\nfn main() {\n    let x = Either<int, string>.Left { value: 42 }\n    let y = Either<int, string>.Right { value: \"hello\" }\n    match x {\n        Either.Left { value } { print(value) }\n        Either.Right { value } { print(value) }\n    }\n    match y {\n        Either.Left { value } { print(value) }\n        Either.Right { value } { print(value) }\n    }\n}",
    );
    assert_eq!(out, "42\nhello\n");
}

#[test]
fn generic_enum_as_param_and_return() {
    let out = compile_and_run_stdout(
        "enum MyOption<T> {\n    Some { value: T }\n    None\n}\n\nfn unwrap_or(opt: MyOption<int>, fallback: int) int {\n    match opt {\n        MyOption.Some { value } { return value }\n        MyOption.None { return fallback }\n    }\n}\n\nfn wrap(x: int) MyOption<int> {\n    return MyOption<int>.Some { value: x }\n}\n\nfn main() {\n    print(unwrap_or(wrap(42), 0))\n    print(unwrap_or(MyOption<int>.None, 99))\n}",
    );
    assert_eq!(out, "42\n99\n");
}

#[test]
fn generic_enum_none_variant() {
    // Test generic enum's unit variant
    let out = compile_and_run_stdout(
        "enum MyOption<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n    let x = MyOption<int>.None\n    match x {\n        MyOption.Some { value } { print(value) }\n        MyOption.None { print(\"none\") }\n    }\n}",
    );
    assert_eq!(out, "none\n");
}

#[test]
fn generic_enum_with_type_bounds() {
    let out = compile_and_run_stdout(
        "trait Describable {\n    fn describe(self) int\n}\n\nclass Num impl Describable {\n    val: int\n    fn describe(self) int {\n        return self.val\n    }\n}\n\nenum Holder<T: Describable> {\n    Has { item: T }\n    Empty\n}\n\nfn main() {\n    let h = Holder<Num>.Has { item: Num { val: 42 } }\n    match h {\n        Holder.Has { item } { print(item.describe()) }\n        Holder.Empty { print(0) }\n    }\n}",
    );
    assert_eq!(out, "42\n");
}

// ═══════════════════════════════════════════════════════════════════════════════
// NULLABLE ENUMS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn nullable_enum_basic() {
    // Function returning Color? that returns a value
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn get_color(flag: bool) Color? {\n    if flag {\n        return Color.Red\n    }\n    return none\n}\n\nfn describe(flag: bool) string? {\n    let c = get_color(flag)?\n    match c {\n        Color.Red { return \"red\" }\n        Color.Blue { return \"blue\" }\n    }\n}\n\nfn main() {\n    let r = describe(true)\n    let s = describe(false)\n    print(1)\n}",
    );
    assert_eq!(out, "1\n");
}

#[test]
fn enum_with_nullable_field() {
    // none literal now properly coerces to T? in enum variant fields
    let out = compile_and_run_stdout(
        "enum Entry {\n    Named { name: string, nickname: string? }\n    Anonymous\n}\n\nfn main() {\n    let e = Entry.Named { name: \"Alice\", nickname: none }\n    match e {\n        Entry.Named { name, nickname } {\n            print(name)\n        }\n        Entry.Anonymous { print(\"anon\") }\n    }\n}",
    );
    assert_eq!(out, "Alice\n");
}

// ═══════════════════════════════════════════════════════════════════════════════
// EQUALITY
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn enum_equality_same_reference() {
    // Same variable compared to itself (same pointer)
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    if c == c {\n        print(\"equal\")\n    } else {\n        print(\"not equal\")\n    }\n}",
    );
    assert_eq!(out, "equal\n");
}

#[test]
fn enum_inequality_different_variants() {
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let r = Color.Red\n    let b = Color.Blue\n    if r != b {\n        print(\"different\")\n    } else {\n        print(\"same\")\n    }\n}",
    );
    assert_eq!(out, "different\n");
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMPLEX / EDGE CASES
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn enum_state_machine_pattern() {
    // Classic state machine: enum + loop + match
    let out = compile_and_run_stdout(
        "enum State {\n    Start\n    Running { count: int }\n    Done\n}\n\nfn next(s: State) State {\n    match s {\n        State.Start { return State.Running { count: 3 } }\n        State.Running { count } {\n            if count > 1 {\n                return State.Running { count: count - 1 }\n            }\n            return State.Done\n        }\n        State.Done { return State.Done }\n    }\n}\n\nfn is_done(s: State) bool {\n    match s {\n        State.Done { return true }\n        State.Start { return false }\n        State.Running { count } { return false }\n    }\n}\n\nfn main() {\n    let s = State.Start\n    let steps = 0\n    while is_done(s) == false {\n        s = next(s)\n        steps = steps + 1\n    }\n    print(steps)\n}",
    );
    assert_eq!(out, "4\n");
}

#[test]
fn enum_linked_list_pattern() {
    // Self-referential enums are now supported with two-pass type registration
    let out = compile_and_run_stdout(
        "enum IntList {\n    Cons { head: int, tail: IntList }\n    Nil\n}\n\nfn sum_list(list: IntList) int {\n    match list {\n        IntList.Cons { head, tail } {\n            return head + sum_list(tail)\n        }\n        IntList.Nil { return 0 }\n    }\n}\n\nfn main() {\n    let list = IntList.Cons { head: 1, tail: IntList.Cons { head: 2, tail: IntList.Cons { head: 3, tail: IntList.Nil } } }\n    print(sum_list(list))\n}",
    );
    assert_eq!(out, "6\n");
}

#[test]
fn enum_multiple_matches_in_function() {
    // Function with multiple match statements on different enum values
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nenum Size {\n    Small\n    Large\n}\n\nfn describe(c: Color, s: Size) {\n    match c {\n        Color.Red { print(\"red\") }\n        Color.Blue { print(\"blue\") }\n    }\n    match s {\n        Size.Small { print(\"small\") }\n        Size.Large { print(\"large\") }\n    }\n}\n\nfn main() {\n    describe(Color.Red, Size.Large)\n}",
    );
    assert_eq!(out, "red\nlarge\n");
}

#[test]
fn enum_reassign_variable() {
    // Reassign an enum variable to a different variant
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n    Green\n}\n\nfn main() {\n    let c = Color.Red\n    match c {\n        Color.Red { print(1) }\n        Color.Blue { print(2) }\n        Color.Green { print(3) }\n    }\n    c = Color.Green\n    match c {\n        Color.Red { print(1) }\n        Color.Blue { print(2) }\n        Color.Green { print(3) }\n    }\n}",
    );
    assert_eq!(out, "1\n3\n");
}

#[test]
fn enum_used_after_match() {
    // Verify the enum value is still usable after matching on it
    let out = compile_and_run_stdout(
        "enum Color {\n    Red\n    Blue\n}\n\nfn describe(c: Color) {\n    match c {\n        Color.Red { print(1) }\n        Color.Blue { print(2) }\n    }\n}\n\nfn main() {\n    let c = Color.Red\n    describe(c)\n    describe(c)\n}",
    );
    assert_eq!(out, "1\n1\n");
}

#[test]
fn enum_conditional_construction() {
    // Use enum in complex conditional logic
    let out = compile_and_run_stdout(
        "enum Priority {\n    Low\n    Medium\n    High\n    Critical\n}\n\nfn classify(score: int) Priority {\n    if score >= 90 {\n        return Priority.Critical\n    }\n    if score >= 70 {\n        return Priority.High\n    }\n    if score >= 50 {\n        return Priority.Medium\n    }\n    return Priority.Low\n}\n\nfn main() {\n    let p1 = classify(95)\n    let p2 = classify(75)\n    let p3 = classify(55)\n    let p4 = classify(30)\n    match p1 {\n        Priority.Low { print(1) }\n        Priority.Medium { print(2) }\n        Priority.High { print(3) }\n        Priority.Critical { print(4) }\n    }\n    match p2 {\n        Priority.Low { print(1) }\n        Priority.Medium { print(2) }\n        Priority.High { print(3) }\n        Priority.Critical { print(4) }\n    }\n    match p3 {\n        Priority.Low { print(1) }\n        Priority.Medium { print(2) }\n        Priority.High { print(3) }\n        Priority.Critical { print(4) }\n    }\n    match p4 {\n        Priority.Low { print(1) }\n        Priority.Medium { print(2) }\n        Priority.High { print(3) }\n        Priority.Critical { print(4) }\n    }\n}",
    );
    assert_eq!(out, "4\n3\n2\n1\n");
}

#[test]
fn enum_array_of_data_variants() {
    // Array containing data variants with different payloads
    let out = compile_and_run_stdout(
        "enum Item {\n    Named { name: string }\n    Numbered { id: int }\n}\n\nfn main() {\n    let items = [Item.Named { name: \"apple\" }, Item.Numbered { id: 42 }, Item.Named { name: \"banana\" }]\n    for item in items {\n        match item {\n            Item.Named { name } { print(name) }\n            Item.Numbered { id } { print(id) }\n        }\n    }\n}",
    );
    assert_eq!(out, "apple\n42\nbanana\n");
}

#[test]
fn enum_match_arm_with_computation() {
    // Complex expressions inside match arm bodies
    let out = compile_and_run_stdout(
        "enum Shape {\n    Rect { w: int, h: int }\n    Circle { r: int }\n    Triangle { base: int, height: int }\n}\n\nfn area(s: Shape) int {\n    match s {\n        Shape.Rect { w, h } { return w * h }\n        Shape.Circle { r } { return r * r * 3 }\n        Shape.Triangle { base, height } { return base * height / 2 }\n    }\n}\n\nfn main() {\n    print(area(Shape.Rect { w: 5, h: 3 }))\n    print(area(Shape.Circle { r: 4 }))\n    print(area(Shape.Triangle { base: 6, height: 4 }))\n}",
    );
    assert_eq!(out, "15\n48\n12\n");
}

// ═══════════════════════════════════════════════════════════════════════════════
// NEGATIVE TESTS: CONSTRUCTION ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn fail_wrong_field_name_construction() {
    compile_should_fail_with(
        "enum Shape {\n    Circle { radius: int }\n    Square { side: int }\n}\n\nfn main() {\n    let s = Shape.Circle { r: 10 }\n}",
        "has no field",
    );
}

#[test]
fn fail_missing_field_construction() {
    compile_should_fail_with(
        "enum Event {\n    Click { x: int, y: int }\n    Keypress { code: int }\n}\n\nfn main() {\n    let e = Event.Click { x: 10 }\n}",
        "fields",
    );
}

#[test]
fn fail_extra_field_construction() {
    compile_should_fail_with(
        "enum Shape {\n    Circle { radius: int }\n    Square { side: int }\n}\n\nfn main() {\n    let s = Shape.Circle { radius: 10, color: 1 }\n}",
        "fields",
    );
}

#[test]
fn fail_wrong_field_type_construction() {
    compile_should_fail_with(
        "enum Shape {\n    Circle { radius: int }\n    Square { side: int }\n}\n\nfn main() {\n    let s = Shape.Circle { radius: \"ten\" }\n}",
        "expected int, found string",
    );
}

#[test]
fn fail_unknown_variant_construction() {
    compile_should_fail_with(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Yellow\n}",
        "has no variant",
    );
}

#[test]
fn fail_unknown_enum_construction() {
    compile_should_fail_with(
        "fn main() {\n    let c = UnknownEnum.Variant\n}",
        "undefined variable",
    );
}

#[test]
fn fail_data_variant_used_as_unit() {
    compile_should_fail_with(
        "enum Shape {\n    Circle { radius: int }\n    Square { side: int }\n}\n\nfn main() {\n    let s = Shape.Circle\n}",
        "has fields",
    );
}

#[test]
fn fail_unit_variant_with_fields() {
    compile_should_fail_with(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red { value: 1 }\n}",
        "fields",
    );
}

#[test]
fn fail_wrong_field_type_int_for_string() {
    compile_should_fail_with(
        "enum Entry {\n    Named { name: string }\n    Anonymous\n}\n\nfn main() {\n    let e = Entry.Named { name: 42 }\n}",
        "expected string, found int",
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// NEGATIVE TESTS: MATCH ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn fail_non_exhaustive_data_variants() {
    compile_should_fail_with(
        "enum Result {\n    Ok { value: int }\n    Err { code: int }\n    Unknown\n}\n\nfn main() {\n    let r = Result.Ok { value: 1 }\n    match r {\n        Result.Ok { value } { print(value) }\n        Result.Err { code } { print(code) }\n    }\n}",
        "non-exhaustive match",
    );
}

#[test]
fn fail_non_exhaustive_mixed() {
    compile_should_fail_with(
        "enum Token {\n    Number { val: int }\n    Plus\n    Minus\n    Eof\n}\n\nfn main() {\n    let t = Token.Plus\n    match t {\n        Token.Number { val } { print(val) }\n        Token.Plus { print(1) }\n        Token.Minus { print(2) }\n    }\n}",
        "non-exhaustive match",
    );
}

#[test]
fn fail_non_exhaustive_many_variants() {
    compile_should_fail_with(
        "enum Dir {\n    N\n    S\n    E\n    W\n    NE\n    NW\n    SE\n    SW\n}\n\nfn main() {\n    let d = Dir.N\n    match d {\n        Dir.N { print(1) }\n        Dir.S { print(2) }\n        Dir.E { print(3) }\n        Dir.W { print(4) }\n        Dir.NE { print(5) }\n        Dir.NW { print(6) }\n        Dir.SE { print(7) }\n    }\n}",
        "non-exhaustive match",
    );
}

#[test]
fn fail_duplicate_match_arm() {
    compile_should_fail_with(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    match c {\n        Color.Red { print(1) }\n        Color.Red { print(2) }\n        Color.Blue { print(3) }\n    }\n}",
        "duplicate match arm",
    );
}

#[test]
fn fail_wrong_enum_in_match() {
    compile_should_fail_with(
        "enum Color {\n    Red\n    Blue\n}\n\nenum Size {\n    Small\n    Large\n}\n\nfn main() {\n    let c = Color.Red\n    match c {\n        Size.Small { print(1) }\n        Size.Large { print(2) }\n    }\n}",
        "does not match scrutinee",
    );
}

#[test]
fn fail_unknown_variant_in_match() {
    compile_should_fail_with(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    match c {\n        Color.Red { print(1) }\n        Color.Yellow { print(2) }\n    }\n}",
        "has no variant",
    );
}

#[test]
fn fail_wrong_binding_count() {
    compile_should_fail_with(
        "enum Pair {\n    Data { first: int, second: int }\n    Empty\n}\n\nfn main() {\n    let p = Pair.Data { first: 1, second: 2 }\n    match p {\n        Pair.Data { first } { print(first) }\n        Pair.Empty { print(0) }\n    }\n}",
        "bindings provided",
    );
}

#[test]
fn fail_unknown_field_in_binding() {
    compile_should_fail_with(
        "enum Wrapper {\n    Val { value: int }\n    Empty\n}\n\nfn main() {\n    let w = Wrapper.Val { value: 1 }\n    match w {\n        Wrapper.Val { wrong_name } { print(wrong_name) }\n        Wrapper.Empty { print(0) }\n    }\n}",
        "has no field",
    );
}

#[test]
fn fail_match_on_int() {
    compile_should_fail_with(
        "fn main() {\n    let x = 42\n    match x {\n        Color.Red { print(1) }\n    }\n}",
        "match requires enum type",
    );
}

#[test]
fn fail_match_on_string() {
    compile_should_fail_with(
        "fn main() {\n    let s = \"hello\"\n    match s {\n        Color.Red { print(1) }\n    }\n}",
        "match requires enum type",
    );
}

#[test]
fn fail_match_on_class() {
    compile_should_fail_with(
        "class Foo {\n    x: int\n}\n\nfn main() {\n    let f = Foo { x: 1 }\n    match f {\n        Foo.Bar { print(1) }\n    }\n}",
        "match requires enum type",
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// NEGATIVE TESTS: TYPE/USAGE ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn fail_enum_field_access() {
    // Cannot access fields on enum directly (only via match)
    compile_should_fail_with(
        "enum Wrapper {\n    Val { value: int }\n    Empty\n}\n\nfn main() {\n    let w = Wrapper.Val { value: 42 }\n    print(w.value)\n}",
        "field access on non-class type",
    );
}

#[test]
fn fail_enum_lifecycle_modifier() {
    compile_should_fail_with(
        "scoped enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n}",
        "lifecycle modifiers",
    );
}

#[test]
fn fail_enum_print_directly() {
    // Enums cannot be printed directly
    compile_should_fail_with(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    print(c)\n}",
        "does not support type",
    );
}

#[test]
fn fail_enum_string_interpolation() {
    // Enums cannot be interpolated in strings
    compile_should_fail_with(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    let s = \"color: {c}\"\n}",
        "cannot interpolate",
    );
}

#[test]
fn fail_enum_arithmetic() {
    compile_should_fail_with(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    let x = c + c\n}",
        "operator not supported",
    );
}

#[test]
fn fail_enum_comparison_lt() {
    compile_should_fail_with(
        "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let a = Color.Red\n    let b = Color.Blue\n    if a < b {\n        print(1)\n    }\n}",
        "comparison not supported",
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// NEGATIVE TESTS: GENERIC ENUM ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn fail_generic_wrong_type_arg_count() {
    compile_should_fail_with(
        "enum MyOption<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n    let x = MyOption<int, string>.Some { value: 42 }\n}",
        "type arguments",
    );
}

#[test]
fn fail_generic_non_exhaustive() {
    compile_should_fail_with(
        "enum MyResult<T> {\n    Ok { value: T }\n    Err { code: int }\n    Unknown\n}\n\nfn main() {\n    let r = MyResult<int>.Ok { value: 1 }\n    match r {\n        MyResult.Ok { value } { print(value) }\n        MyResult.Err { code } { print(code) }\n    }\n}",
        "non-exhaustive match",
    );
}

#[test]
fn fail_generic_type_bound_violation() {
    compile_should_fail_with(
        "trait Printable {\n    fn display(self) int\n}\n\nenum Holder<T: Printable> {\n    Has { item: T }\n    Empty\n}\n\nclass Plain {\n    val: int\n}\n\nfn main() {\n    let h = Holder<Plain>.Has { item: Plain { val: 1 } }\n}",
        "does not satisfy",
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// MATCH-AS-EXPRESSION: POSITIVE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

// ============================================================
// Match-as-Expression: Expression Contexts (5 tests)
// ============================================================

#[test]
fn match_expr_in_return() {
    let stdout = compile_and_run_stdout(r#"
        enum Status { Active Inactive }
        fn get_code(s: Status) int {
            return match s {
                Status.Active => 1,
                Status.Inactive => 0
            }
        }
        fn main() {
            print(get_code(Status.Active))
        }
    "#);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn match_expr_in_function_arg() {
    let stdout = compile_and_run_stdout(r#"
        enum Status { Active Inactive }
        fn double(x: int) int { return x * 2 }
        fn main() {
            let s = Status.Active
            let result = double(match s {
                Status.Active => 5,
                Status.Inactive => 10
            })
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "10");
}

#[test]
fn match_expr_in_binary_op() {
    let stdout = compile_and_run_stdout(r#"
        enum Status { Active Inactive }
        fn main() {
            let s = Status.Active
            let result = (match s {
                Status.Active => 10,
                Status.Inactive => 5
            }) + 100
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "110");
}

#[test]
fn match_expr_in_if_condition() {
    let stdout = compile_and_run_stdout(r#"
        enum Status { Active Inactive }
        fn main() {
            let s = Status.Active
            if match s { Status.Active => true, Status.Inactive => false } {
                print("yes")
            } else {
                print("no")
            }
        }
    "#);
    assert_eq!(stdout.trim(), "yes");
}

#[test]
fn match_expr_in_array_literal() {
    let stdout = compile_and_run_stdout(r#"
        enum E { A B }
        fn main() {
            let e1 = E.A
            let e2 = E.B
            let arr = [
                match e1 { E.A => 1, E.B => 2 },
                match e2 { E.A => 3, E.B => 4 }
            ]
            print(arr[0] + arr[1])
        }
    "#);
    assert_eq!(stdout.trim(), "5");
}

// ============================================================
// Match-as-Expression: Data Variant Patterns (4 tests)
// ============================================================

#[test]
fn match_expr_data_variant_all_fields() {
    let stdout = compile_and_run_stdout(r#"
        enum Shape {
            Circle { radius: float }
            Rectangle { width: float, height: float }
        }
        fn main() {
            let s = Shape.Circle { radius: 5.0 }
            let area = match s {
                Shape.Circle { radius: r } => r * r * 3.0,
                Shape.Rectangle { width: w, height: h } => w * h
            }
            print(area)
        }
    "#);
    assert_eq!(stdout.trim(), "75.000000");
}

#[test]
fn match_expr_multiple_data_variants() {
    let stdout = compile_and_run_stdout(r#"
        enum Result<T> {
            Ok { value: T }
            Err { message: string }
        }
        fn main() {
            let r = Result<int>.Ok { value: 42 }
            let n = match r {
                Result.Ok { value: v } => v,
                Result.Err { message: m } => 0
            }
            print(n)
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn match_expr_field_rename() {
    let stdout = compile_and_run_stdout(r#"
        enum Wrapper {
            Val { x: int }
        }
        fn main() {
            let w = Wrapper.Val { x: 100 }
            let result = match w {
                Wrapper.Val { x: renamed } => renamed * 2
            }
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "200");
}

#[test]
fn match_expr_binding_shadows_outer() {
    let stdout = compile_and_run_stdout(r#"
        enum E {
            V { x: int }
        }
        fn main() {
            let x = 999
            let e = E.V { x: 42 }
            let inner = match e {
                E.V { x: x } => x
            }
            print(inner)
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "42\n999");
}

// ============================================================
// Match-as-Expression: Type Consistency (3 tests)
// ============================================================

#[test]
fn match_expr_all_arms_same_class() {
    let stdout = compile_and_run_stdout(r#"
        class Point {
            x: int
            y: int
        }
        enum E { A B }
        fn make_point_a() Point {
            return Point {
                x: 1
                y: 2
            }
        }
        fn make_point_b() Point {
            return Point {
                x: 3
                y: 4
            }
        }
        fn main() {
            let e = E.A
            let p = match e {
                E.A => make_point_a(),
                E.B => make_point_b()
            }
            print(p.x + p.y)
        }
    "#);
    assert_eq!(stdout.trim(), "3");
}

#[test]
fn match_expr_all_arms_same_enum() {
    let stdout = compile_and_run_stdout(r#"
        enum Color { Red Green Blue }
        enum E { A B }
        fn main() {
            let e = E.A
            let c = match e {
                E.A => Color.Red,
                E.B => Color.Blue
            }
            match c {
                Color.Red { print("red") }
                Color.Green { print("green") }
                Color.Blue { print("blue") }
            }
        }
    "#);
    assert_eq!(stdout.trim(), "red");
}

#[test]
fn match_expr_all_arms_string() {
    let stdout = compile_and_run_stdout(r#"
        enum Status { Active Inactive Suspended }
        fn main() {
            let s = Status.Suspended
            let msg = match s {
                Status.Active => "running",
                Status.Inactive => "stopped",
                Status.Suspended => "paused"
            }
            print(msg)
        }
    "#);
    assert_eq!(stdout.trim(), "paused");
}

// ============================================================
// Match-as-Expression: Generic Enums (3 tests)
// ============================================================

#[test]
fn match_expr_generic_enum_concrete() {
    let stdout = compile_and_run_stdout(r#"
        enum Option<T> {
            Some { value: T }
            None
        }
        fn main() {
            let opt = Option<int>.Some { value: 100 }
            let x = match opt {
                Option.Some { value: v } => v,
                Option.None => 0
            }
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "100");
}

#[test]
fn match_expr_generic_enum_inferred() {
    let stdout = compile_and_run_stdout(r#"
        enum Option<T> {
            Some { value: T }
            None
        }
        fn unwrap<T>(opt: Option<T>, fallback: T) T {
            return match opt {
                Option.Some { value: v } => v,
                Option.None => fallback
            }
        }
        fn main() {
            let opt = Option<int>.None
            print(unwrap<int>(opt, 42))
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn match_expr_generic_enum_nested_match() {
    let stdout = compile_and_run_stdout(r#"
        enum Option<T> {
            Some { value: T }
            None
        }
        fn main() {
            let outer = Option<Option<int>>.Some {
                value: Option<int>.Some { value: 10 }
            }
            let result = match outer {
                Option.Some { value: inner } => match inner {
                    Option.Some { value: v } => v,
                    Option.None => 0
                },
                Option.None => 0
            }
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "10");
}

// ============================================================
// Match-as-Expression: Edge Cases (3 tests)
// ============================================================

#[test]
fn match_expr_single_variant_enum() {
    let stdout = compile_and_run_stdout(r#"
        enum Singleton { Only { x: int } }
        fn main() {
            let s = Singleton.Only { x: 777 }
            let n = match s {
                Singleton.Only { x: val } => val
            }
            print(n)
        }
    "#);
    assert_eq!(stdout.trim(), "777");
}

#[test]
fn match_expr_many_arms() {
    let stdout = compile_and_run_stdout(r#"
        enum Digit { D0 D1 D2 D3 D4 D5 D6 D7 D8 D9 }
        fn main() {
            let d = Digit.D5
            let n = match d {
                Digit.D0 => 0,
                Digit.D1 => 1,
                Digit.D2 => 2,
                Digit.D3 => 3,
                Digit.D4 => 4,
                Digit.D5 => 5,
                Digit.D6 => 6,
                Digit.D7 => 7,
                Digit.D8 => 8,
                Digit.D9 => 9
            }
            print(n)
        }
    "#);
    assert_eq!(stdout.trim(), "5");
}

#[test]
fn match_expr_trailing_comma_accepted() {
    let stdout = compile_and_run_stdout(r#"
        enum E { A B }
        fn main() {
            let e = E.A
            let x = match e {
                E.A => 1,
                E.B => 2,
            }
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "1");
}
