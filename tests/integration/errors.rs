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
        "error OutOfRange {}\n\nfn check(x: int) int {\n    if x > 5 {\n        raise OutOfRange {}\n    }\n    return x\n}\n\nfn main() {\n    let i = 0\n    let mut sum = 0\n    while i < 10 {\n        let val = check(i) catch 0\n        sum = sum + val\n        i = i + 1\n    }\n    print(sum)\n}",
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

// ── Method error enforcement tests ─────────────────────────────────────────────

#[test]
fn error_bare_fallible_method_rejected() {
    compile_should_fail_with(
        "error Fail {}\n\nclass Svc {\n    _x: int\n    fn do_thing(self) {\n        raise Fail {}\n    }\n}\n\nfn main() {\n    let s = Svc { _x: 0 }\n    s.do_thing()\n}",
        "must be handled",
    );
}

#[test]
fn error_method_propagate() {
    let out = compile_and_run_stdout(
        "error Fail {}\n\nclass Svc {\n    _x: int\n    fn do_thing(self) int {\n        raise Fail {}\n        return 0\n    }\n}\n\nfn wrapper() int {\n    let s = Svc { _x: 0 }\n    let x = s.do_thing()!\n    return x\n}\n\nfn main() {\n    let r = wrapper() catch -1\n    print(r)\n}",
    );
    assert_eq!(out, "-1\n");
}

#[test]
fn error_method_catch_wildcard() {
    let out = compile_and_run_stdout(
        "error Fail {}\n\nclass Svc {\n    _x: int\n    fn do_thing(self) int {\n        raise Fail {}\n        return 0\n    }\n}\n\nfn main() {\n    let s = Svc { _x: 0 }\n    let r = s.do_thing() catch err { -99 }\n    print(r)\n}",
    );
    assert_eq!(out, "-99\n");
}

#[test]
fn error_method_catch_shorthand() {
    let out = compile_and_run_stdout(
        "error Fail {}\n\nclass Svc {\n    _x: int\n    fn do_thing(self) int {\n        raise Fail {}\n        return 0\n    }\n}\n\nfn main() {\n    let s = Svc { _x: 0 }\n    let r = s.do_thing() catch 42\n    print(r)\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn error_method_bang_on_infallible_rejected() {
    compile_should_fail_with(
        "class Svc {\n    _x: int\n    fn safe(self) int {\n        return 1\n    }\n}\n\nfn main() {\n    let s = Svc { _x: 0 }\n    let x = s.safe()!\n}",
        "infallible",
    );
}

#[test]
fn error_method_catch_on_infallible_rejected() {
    compile_should_fail_with(
        "class Svc {\n    _x: int\n    fn safe(self) int {\n        return 1\n    }\n}\n\nfn main() {\n    let s = Svc { _x: 0 }\n    let x = s.safe() catch 0\n}",
        "infallible",
    );
}

#[test]
fn error_method_infallible_bare_ok() {
    let out = compile_and_run_stdout(
        "class Svc {\n    _x: int\n    fn safe(self) int {\n        return 42\n    }\n}\n\nfn main() {\n    let s = Svc { _x: 0 }\n    let x = s.safe()\n    print(x)\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn error_method_transitive_propagation() {
    let out = compile_and_run_stdout(
        "error Fail {}\n\nclass Inner {\n    _x: int\n    fn risky(self) int {\n        raise Fail {}\n        return 0\n    }\n}\n\nclass Outer {\n    _x: int\n    fn wrap(self) int {\n        let i = Inner { _x: 0 }\n        let x = i.risky()!\n        return x\n    }\n}\n\nfn main() {\n    let o = Outer { _x: 0 }\n    let r = o.wrap() catch -1\n    print(r)\n}",
    );
    assert_eq!(out, "-1\n");
}

#[test]
fn error_default_trait_method_fallible() {
    compile_should_fail_with(
        "error Fail {}\n\ntrait Greeter {\n    fn greet(self) int {\n        raise Fail {}\n        return 0\n    }\n}\n\nclass A impl Greeter {\n    x: int\n}\n\nfn main() {\n    let a = A { x: 1 }\n    a.greet()\n}",
        "must be handled",
    );
}

#[test]
fn error_trait_dispatch_any_fallible() {
    compile_should_fail_with(
        "error Fail {}\n\ntrait Worker {\n    fn work(self) int\n}\n\nclass Safe impl Worker {\n    fn work(self) int {\n        return 1\n    }\n}\n\nclass Risky impl Worker {\n    fn work(self) int {\n        raise Fail {}\n        return 0\n    }\n}\n\nfn use_worker(w: Worker) int {\n    return w.work()\n}",
        "must be handled",
    );
}

#[test]
fn error_trait_dispatch_all_infallible() {
    let out = compile_and_run_stdout(
        "trait Worker {\n    fn work(self) int\n}\n\nclass A impl Worker {\n    _x: int\n    fn work(self) int {\n        return 1\n    }\n}\n\nclass B impl Worker {\n    _x: int\n    fn work(self) int {\n        return 2\n    }\n}\n\nfn use_worker(w: Worker) int {\n    return w.work()\n}\n\nfn main() {\n    let a = A { _x: 0 }\n    let r = use_worker(a)\n    print(r)\n}",
    );
    assert_eq!(out, "1\n");
}

#[test]
fn error_trait_dispatch_propagate() {
    let out = compile_and_run_stdout(
        "error Fail {}\n\ntrait Worker {\n    fn work(self) int\n}\n\nclass Risky impl Worker {\n    _x: int\n    fn work(self) int {\n        raise Fail {}\n        return 0\n    }\n}\n\nfn use_worker(w: Worker) int {\n    let r = w.work()!\n    return r\n}\n\nfn main() {\n    let r = Risky { _x: 0 }\n    let result = use_worker(r) catch -1\n    print(result)\n}",
    );
    assert_eq!(out, "-1\n");
}

#[test]
fn error_app_method_fallible() {
    compile_should_fail_with(
        "error Fail {}\n\nclass Svc {\n    fn do_thing(self) {\n        raise Fail {}\n    }\n}\n\napp MyApp[svc: Svc] {\n    fn main(self) {\n        self.svc.do_thing()\n    }\n}",
        "must be handled",
    );
}

#[test]
fn error_generic_fn_with_method_call_compiles() {
    let out = compile_and_run_stdout(
        "class Box {\n    val: int\n    fn get(self) int {\n        return self.val\n    }\n}\n\nfn identity<T>(x: T) T {\n    return x\n}\n\nfn main() {\n    let b = Box { val: 42 }\n    let v = b.get()\n    let r = identity(v)\n    print(r)\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn error_builtin_method_bang_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let arr = [1, 2, 3]\n    arr.push(4)!\n}",
        "infallible",
    );
}

#[test]
fn error_builtin_method_catch_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let arr = [1, 2, 3]\n    let x = arr.len() catch 0\n}",
        "infallible",
    );
}

#[test]
fn error_builtin_method_bare_ok() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let arr = [1, 2, 3]\n    arr.push(4)\n    print(arr.len())\n}",
    );
    assert_eq!(out, "4\n");
}

#[test]
fn error_method_in_closure_body() {
    compile_should_fail_with(
        "error Fail {}\n\nclass Svc {\n    _x: int\n    fn risky(self) int {\n        raise Fail {}\n        return 0\n    }\n}\n\nfn main() {\n    let s = Svc { _x: 0 }\n    let f = (x: int) => s.risky()\n}",
        "must be handled",
    );
}

// ── Multi-statement catch blocks ────────────────────────────────────

#[test]
fn catch_wildcard_multi_stmt_with_return() {
    let out = compile_and_run_stdout(
        "error NotFound {\n    code: int\n}\n\nfn find(id: int) int {\n    if id < 0 {\n        raise NotFound { code: id }\n    }\n    return id * 2\n}\n\nfn main() {\n    let result = find(-1) catch err {\n        print(\"caught\")\n        return\n    }\n    print(result)\n}",
    );
    assert_eq!(out, "caught\n");
}

#[test]
fn catch_wildcard_multi_stmt_fallback_value() {
    let out = compile_and_run_stdout(
        "error NotFound {\n    code: int\n}\n\nfn find(id: int) int {\n    if id < 0 {\n        raise NotFound { code: id }\n    }\n    return id * 2\n}\n\nfn main() {\n    let result = find(-1) catch err {\n        let fallback = 42\n        fallback\n    }\n    print(result)\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn catch_wildcard_multi_stmt_with_let_and_return() {
    let out = compile_and_run_stdout(
        "error BadInput {\n    code: int\n}\n\nfn validate(x: int) int {\n    if x < 0 {\n        raise BadInput { code: x }\n    }\n    return x\n}\n\nfn main() {\n    let result = validate(-5) catch err {\n        let msg = \"error occurred\"\n        print(msg)\n        return\n    }\n    print(result)\n}",
    );
    assert_eq!(out, "error occurred\n");
}

#[test]
fn catch_wildcard_single_expr_still_works() {
    let out = compile_and_run_stdout(
        "error NotFound {\n    code: int\n}\n\nfn find(id: int) int {\n    if id < 0 {\n        raise NotFound { code: id }\n    }\n    return id * 2\n}\n\nfn main() {\n    let result = find(-1) catch err { -99 }\n    print(result)\n}",
    );
    assert_eq!(out, "-99\n");
}

#[test]
fn catch_wildcard_multi_stmt_no_error() {
    let out = compile_and_run_stdout(
        "error NotFound {\n    code: int\n}\n\nfn find(id: int) int {\n    if id < 0 {\n        raise NotFound { code: id }\n    }\n    return id * 2\n}\n\nfn main() {\n    let result = find(5) catch err {\n        print(\"should not print\")\n        return\n    }\n    print(result)\n}",
    );
    assert_eq!(out, "10\n");
}

#[test]
fn builtin_network_error() {
    let out = compile_and_run_stdout(
        "fn test_network() {\n    raise NetworkError { message: \"connection failed\" }\n}\n\nfn main() {\n    test_network() catch NetworkError {\n        print(\"caught network error\")\n    }\n}",
    );
    assert_eq!(out, "caught network error\n");
}

#[test]
fn builtin_timeout_error() {
    let out = compile_and_run_stdout(
        "fn test_timeout() {\n    raise TimeoutError { millis: 5000 }\n}\n\nfn main() {\n    test_timeout() catch TimeoutError {\n        print(\"caught timeout\")\n    }\n}",
    );
    assert_eq!(out, "caught timeout\n");
}

#[test]
fn builtin_service_unavailable() {
    let out = compile_and_run_stdout(
        "fn test_service() {\n    raise ServiceUnavailable { service: \"api\" }\n}\n\nfn main() {\n    test_service() catch ServiceUnavailable {\n        print(\"service unavailable\")\n    }\n}",
    );
    assert_eq!(out, "service unavailable\n");
}

// ============================================================
// If-Expression Integration Tests
// ============================================================

#[test]
fn if_expr_with_error_propagation_in_branch() {
    let out = compile_and_run_stdout(
        "error ParseError {\n    msg: string\n}\n\nfn parse(s: string) int {\n    if s == \"bad\" {\n        raise ParseError { msg: \"bad\" }\n    }\n    return 42\n}\n\nfn main() {\n    let result = if true { parse(\"good\")! } else { 0 }\n    print(result)\n}",
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn if_expr_with_catch_in_branch() {
    let out = compile_and_run_stdout(
        "error ParseError {\n    msg: string\n}\n\nfn parse(s: string) int {\n    raise ParseError { msg: \"bad\" }\n}\n\nfn main() {\n    let result = if true {\n        parse(\"bad\") catch ParseError { 99 }\n    } else {\n        0\n    }\n    print(result)\n}",
    );
    assert_eq!(out.trim(), "99");
}

#[test]
fn if_expr_fallible_in_condition() {
    let out = compile_and_run_stdout(
        "error CheckError {\n    code: int\n}\n\nfn check(x: int) bool {\n    if x < 0 {\n        raise CheckError { code: -1 }\n    }\n    return x > 5\n}\n\nfn main() {\n    let x = if check(10)! { 100 } else { 200 }\n    print(x)\n}",
    );
    assert_eq!(out.trim(), "100");
}

#[test]
fn if_expr_error_type_unification() {
    // Both branches fallible with same error type
    let out = compile_and_run_stdout(
        "error MyError {\n    val: int\n}\n\nfn foo() int {\n    if false {\n        raise MyError { val: 1 }\n    }\n    return 10\n}\n\nfn bar() int {\n    if false {\n        raise MyError { val: 2 }\n    }\n    return 20\n}\n\nfn main() {\n    let x = if true { foo()! } else { bar()! }\n    print(x)\n}",
    );
    assert_eq!(out.trim(), "10");
}

#[test]
fn assign_to_immutable_variable() {
    compile_should_fail_with(
        "fn main() {\n    let x = 1\n    x = 2\n}",
        "cannot assign to immutable variable",
    );
}
