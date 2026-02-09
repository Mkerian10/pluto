mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

#[test]
fn error_catch_shorthand_on_error() {
    let out = compile_and_run_stdout(
        "error NotFound {\n    code: int\n}\n\nfn find(id: int) int {\n    if id < 0 {\n        raise NotFound { code: -1 }\n    }\n    return id * 2\n}\n\nfn main() {\n    let result = find(-1) catch 0\n    print(result)\n}",
    );
    assert_eq!(out, "0\n");
}

#[test]
fn error_catch_shorthand_no_error() {
    let out = compile_and_run_stdout(
        "error NotFound {\n    code: int\n}\n\nfn find(id: int) int {\n    if id < 0 {\n        raise NotFound { code: -1 }\n    }\n    return id * 2\n}\n\nfn main() {\n    let result = find(5) catch 0\n    print(result)\n}",
    );
    assert_eq!(out, "10\n");
}

#[test]
fn error_catch_wildcard() {
    let out = compile_and_run_stdout(
        "error NotFound {\n    code: int\n}\n\nfn find(id: int) int {\n    if id < 0 {\n        raise NotFound { code: -1 }\n    }\n    return id * 2\n}\n\nfn main() {\n    let result = find(-1) catch err { -99 }\n    print(result)\n}",
    );
    assert_eq!(out, "-99\n");
}

#[test]
fn error_propagation_then_catch() {
    let out = compile_and_run_stdout(
        "error BadInput {\n    code: int\n}\n\nfn validate(x: int) int {\n    if x < 0 {\n        raise BadInput { code: x }\n    }\n    return x\n}\n\nfn process(x: int) int {\n    let v = validate(x)!\n    return v * 10\n}\n\nfn main() {\n    let a = process(-5) catch 0\n    print(a)\n    let b = process(3) catch 0\n    print(b)\n}",
    );
    assert_eq!(out, "0\n30\n");
}

#[test]
fn error_transitive_propagation() {
    let out = compile_and_run_stdout(
        "error Fail {}\n\nfn step1() int {\n    raise Fail {}\n    return 0\n}\n\nfn step2() int {\n    let x = step1()!\n    return x + 1\n}\n\nfn step3() int {\n    let x = step2()!\n    return x + 1\n}\n\nfn main() {\n    let result = step3() catch -1\n    print(result)\n}",
    );
    assert_eq!(out, "-1\n");
}

#[test]
fn error_code_after_propagation_skipped() {
    let out = compile_and_run_stdout(
        "error Fail {}\n\nfn might_fail(x: int) int {\n    if x == 0 {\n        raise Fail {}\n    }\n    return x\n}\n\nfn wrapper(x: int) int {\n    let a = might_fail(x)!\n    print(999)\n    return a\n}\n\nfn main() {\n    let r = wrapper(0) catch -1\n    print(r)\n}",
    );
    assert_eq!(out, "-1\n");
}

#[test]
fn error_conditional_raise() {
    let out = compile_and_run_stdout(
        "error TooSmall {\n    val: int\n}\n\nfn check_positive(x: int) int {\n    if x <= 0 {\n        raise TooSmall { val: x }\n    }\n    return x\n}\n\nfn main() {\n    let a = check_positive(5) catch 0\n    print(a)\n    let b = check_positive(-3) catch 0\n    print(b)\n    let c = check_positive(10) catch 0\n    print(c)\n}",
    );
    assert_eq!(out, "5\n0\n10\n");
}

#[test]
fn error_no_fields() {
    let out = compile_and_run_stdout(
        "error Empty {}\n\nfn fail_always() int {\n    raise Empty {}\n    return 0\n}\n\nfn main() {\n    let x = fail_always() catch 42\n    print(x)\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn error_with_string_field() {
    let out = compile_and_run_stdout(
        "error WithMsg {\n    msg: string\n}\n\nfn greet(name: string) string {\n    if name == \"\" {\n        raise WithMsg { msg: \"empty name\" }\n    }\n    return \"hello \" + name\n}\n\nfn main() {\n    let a = greet(\"alice\") catch \"error\"\n    print(a)\n    let b = greet(\"\") catch \"error\"\n    print(b)\n}",
    );
    assert_eq!(out, "hello alice\nerror\n");
}

#[test]
fn error_multiple_types() {
    let out = compile_and_run_stdout(
        "error NotFound {\n    id: int\n}\n\nerror Forbidden {\n    reason: string\n}\n\nfn find(id: int) int {\n    if id < 0 {\n        raise NotFound { id: id }\n    }\n    return id\n}\n\nfn check_access(level: int) int {\n    if level < 5 {\n        raise Forbidden { reason: \"too low\" }\n    }\n    return level\n}\n\nfn main() {\n    let a = find(-1) catch -1\n    print(a)\n    let b = find(10) catch -1\n    print(b)\n    let c = check_access(3) catch -1\n    print(c)\n    let d = check_access(7) catch -1\n    print(d)\n}",
    );
    assert_eq!(out, "-1\n10\n-1\n7\n");
}

#[test]
fn error_propagation_in_main() {
    let out = compile_and_run_stdout(
        "error Fail {}\n\nfn will_fail() {\n    raise Fail {}\n}\n\nfn main() {\n    will_fail()!\n    print(42)\n}",
    );
    assert_eq!(out, "");
}

#[test]
fn error_catch_both_paths() {
    let out = compile_and_run_stdout(
        "error Nope {}\n\nfn maybe(x: int) int {\n    if x == 0 {\n        raise Nope {}\n    }\n    return x * 3\n}\n\nfn main() {\n    let a = maybe(0) catch -1\n    let b = maybe(4) catch -1\n    print(a)\n    print(b)\n}",
    );
    assert_eq!(out, "-1\n12\n");
}

#[test]
fn error_multiple_catches_in_sequence() {
    let out = compile_and_run_stdout(
        "error E {}\n\nfn fail() int {\n    raise E {}\n    return 0\n}\n\nfn main() {\n    let a = fail() catch 1\n    let b = fail() catch 2\n    let c = fail() catch 3\n    print(a)\n    print(b)\n    print(c)\n}",
    );
    assert_eq!(out, "1\n2\n3\n");
}

#[test]
fn error_in_while_loop() {
    let out = compile_and_run_stdout(
        "error OutOfRange {}\n\nfn check(x: int) int {\n    if x > 5 {\n        raise OutOfRange {}\n    }\n    return x\n}\n\nfn main() {\n    let i = 0\n    let sum = 0\n    while i < 10 {\n        let val = check(i) catch 0\n        sum = sum + val\n        i = i + 1\n    }\n    print(sum)\n}",
    );
    assert_eq!(out, "15\n");
}

#[test]
fn error_with_class_return() {
    let out = compile_and_run_stdout(
        "error NotFound {}\n\nclass Point {\n    x: int\n    y: int\n}\n\nfn find_point(id: int) Point {\n    if id < 0 {\n        raise NotFound {}\n    }\n    return Point { x: id, y: id * 2 }\n}\n\nfn default_point() Point {\n    return Point { x: 0, y: 0 }\n}\n\nfn main() {\n    let p = find_point(5) catch default_point()\n    print(p.x)\n    print(p.y)\n    let q = find_point(-1) catch default_point()\n    print(q.x)\n    print(q.y)\n}",
    );
    assert_eq!(out, "5\n10\n0\n0\n");
}

#[test]
fn error_catch_wildcard_variable_used() {
    // Safety-net: catch wildcard binds `err` and the body *uses* it.
    // Verifies the variable is properly scoped and accessible inside the catch block.
    let out = compile_and_run_stdout(
        "error BadInput {\n    code: int\n}\n\nfn validate(x: int) int {\n    if x < 0 {\n        raise BadInput { code: x }\n    }\n    return x\n}\n\nfn main() {\n    let x = 42\n    let result = validate(-5) catch err { x }\n    print(result)\n    print(x)\n}",
    );
    assert_eq!(out, "42\n42\n");
}

// Error handling compile-time rejection tests

#[test]
fn error_bare_fallible_call_rejected() {
    compile_should_fail_with(
        "error E {}\n\nfn fail() {\n    raise E {}\n}\n\nfn main() {\n    fail()\n}",
        "must be handled",
    );
}

#[test]
fn error_bang_on_infallible_rejected() {
    compile_should_fail_with(
        "fn safe() int {\n    return 42\n}\n\nfn main() {\n    let x = safe()!\n}",
        "infallible",
    );
}

#[test]
fn error_catch_on_infallible_rejected() {
    compile_should_fail_with(
        "fn safe() int {\n    return 42\n}\n\nfn main() {\n    let x = safe() catch 0\n}",
        "infallible",
    );
}

#[test]
fn error_raise_unknown_type_rejected() {
    compile_should_fail_with(
        "fn main() {\n    raise Nonexistent {}\n}",
        "unknown error type",
    );
}

#[test]
fn error_raise_wrong_field_rejected() {
    compile_should_fail_with(
        "error MyError {\n    code: int\n}\n\nfn main() {\n    raise MyError { wrong: 1 }\n}",
        "no field",
    );
}

#[test]
fn error_raise_wrong_field_count_rejected() {
    compile_should_fail_with(
        "error MyError {\n    code: int\n}\n\nfn main() {\n    raise MyError {}\n}",
        "fields",
    );
}

#[test]
fn error_catch_type_mismatch_rejected() {
    compile_should_fail_with(
        "error E {}\n\nfn get() int {\n    raise E {}\n    return 0\n}\n\nfn main() {\n    let x = get() catch \"oops\"\n}",
        "catch handler type mismatch",
    );
}

#[test]
fn error_bang_on_non_call_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let x = 42!\n}",
        "can only be applied to function calls",
    );
}

#[test]
fn error_raise_wrong_field_type_rejected() {
    compile_should_fail_with(
        "error MyError {\n    code: int\n}\n\nfn main() {\n    raise MyError { code: \"hello\" }\n}",
        "expected int",
    );
}
