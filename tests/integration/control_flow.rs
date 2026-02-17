mod common;
use common::{compile_and_run, compile_and_run_stdout, compile_should_fail, compile_should_fail_with};

#[test]
fn if_else() {
    let code = compile_and_run(
        "fn main() {\n    if true {\n        let x = 1\n    } else {\n        let x = 2\n    }\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn while_loop() {
    let code = compile_and_run(
        "fn main() {\n    let x = 0\n    while x < 10 {\n        x = x + 1\n    }\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn if_else_output() {
    let out = compile_and_run_stdout(
        "fn main() {\n    if true {\n        print(1)\n    } else {\n        print(2)\n    }\n    if false {\n        print(3)\n    } else {\n        print(4)\n    }\n}",
    );
    assert_eq!(out, "1\n4\n");
}

#[test]
fn if_without_else() {
    let out = compile_and_run_stdout(
        "fn main() {\n    if true {\n        print(1)\n    }\n    if false {\n        print(2)\n    }\n    print(3)\n}",
    );
    assert_eq!(out, "1\n3\n");
}

#[test]
fn nested_if_else() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 15\n    if x > 10 {\n        if x > 20 {\n            print(1)\n        } else {\n            print(2)\n        }\n    } else {\n        print(3)\n    }\n}",
    );
    assert_eq!(out, "2\n");
}

// For loop tests

#[test]
fn for_loop_basic() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3]\n    for x in a {\n        print(x)\n    }\n}",
    );
    assert_eq!(out, "1\n2\n3\n");
}

#[test]
fn for_loop_sum() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [10, 20, 30]\n    let mut total = 0\n    for x in a {\n        total = total + x\n    }\n    print(total)\n}",
    );
    assert_eq!(out, "60\n");
}

#[test]
fn for_loop_nested() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2]\n    let b = [10, 20]\n    for x in a {\n        for y in b {\n            print(x + y)\n        }\n    }\n}",
    );
    assert_eq!(out, "11\n21\n12\n22\n");
}

#[test]
fn for_loop_empty_body() {
    let code = compile_and_run(
        "fn main() {\n    let a = [1, 2, 3]\n    for x in a {\n    }\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn for_loop_non_array_rejected() {
    compile_should_fail(
        "fn main() {\n    for x in 42 {\n    }\n}",
    );
}

#[test]
fn for_loop_bools() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [true, false, true]\n    for b in a {\n        print(b)\n    }\n}",
    );
    assert_eq!(out, "true\nfalse\ntrue\n");
}

#[test]
fn for_loop_floats() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1.5, 2.5, 3.5]\n    for f in a {\n        print(f)\n    }\n}",
    );
    assert_eq!(out, "1.500000\n2.500000\n3.500000\n");
}

#[test]
fn for_loop_over_function_result() {
    let out = compile_and_run_stdout(
        "fn nums() [int] {\n    return [5, 10, 15]\n}\n\nfn main() {\n    for n in nums() {\n        print(n)\n    }\n}",
    );
    assert_eq!(out, "5\n10\n15\n");
}

#[test]
fn for_loop_var_shadows_outer() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 999\n    for x in [1, 2, 3] {\n        print(x)\n    }\n    print(x)\n}",
    );
    assert_eq!(out, "1\n2\n3\n999\n");
}

#[test]
fn for_loop_early_return() {
    let out = compile_and_run_stdout(
        "fn find_first_positive(a: [int]) int {\n    for x in a {\n        if x > 0 {\n            return x\n        }\n    }\n    return 0\n}\n\nfn main() {\n    print(find_first_positive([-1, -2, 5, 10]))\n}",
    );
    assert_eq!(out, "5\n");
}

#[test]
fn for_loop_inside_while() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let i = 0\n    while i < 2 {\n        for x in [10, 20] {\n            print(x + i)\n        }\n        i = i + 1\n    }\n}",
    );
    assert_eq!(out, "10\n20\n11\n21\n");
}

#[test]
fn for_loop_method_call_on_element() {
    let out = compile_and_run_stdout(
        "class Pair {\n    a: int\n    b: int\n\n    fn sum(self) int {\n        return self.a + self.b\n    }\n}\n\nfn main() {\n    let pairs = [Pair { a: 1, b: 2 }, Pair { a: 3, b: 4 }]\n    for p in pairs {\n        print(p.sum())\n    }\n}",
    );
    assert_eq!(out, "3\n7\n");
}

#[test]
fn for_loop_push_during_iteration() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3]\n    let mut count = 0\n    for x in a {\n        count = count + 1\n        a.push(x * 10)\n    }\n    print(count)\n    print(a.len())\n}",
    );
    assert_eq!(out, "3\n6\n");
}

#[test]
fn for_loop_nested_same_array() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2]\n    for x in a {\n        for y in a {\n            print(x * 10 + y)\n        }\n    }\n}",
    );
    assert_eq!(out, "11\n12\n21\n22\n");
}

// ── break tests ──

#[test]
fn while_break() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let i = 0\n    while true {\n        if i == 3 {\n            break\n        }\n        print(i)\n        i = i + 1\n    }\n    print(99)\n}",
    );
    assert_eq!(out, "0\n1\n2\n99\n");
}

#[test]
fn for_break() {
    let out = compile_and_run_stdout(
        "fn main() {\n    for x in [10, 20, 30, 40, 50] {\n        if x == 30 {\n            break\n        }\n        print(x)\n    }\n    print(99)\n}",
    );
    assert_eq!(out, "10\n20\n99\n");
}

#[test]
fn nested_loops_inner_break() {
    let out = compile_and_run_stdout(
        "fn main() {\n    for x in [1, 2] {\n        for y in [10, 20, 30] {\n            if y == 20 {\n                break\n            }\n            print(x * 100 + y)\n        }\n    }\n}",
    );
    assert_eq!(out, "110\n210\n");
}

#[test]
fn break_outside_loop_rejected() {
    compile_should_fail_with(
        "fn main() {\n    break\n}",
        "can only be used inside a loop",
    );
}

// ── continue tests ──

#[test]
fn while_continue() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let i = 0\n    while i < 5 {\n        i = i + 1\n        if i == 3 {\n            continue\n        }\n        print(i)\n    }\n}",
    );
    assert_eq!(out, "1\n2\n4\n5\n");
}

#[test]
fn for_continue() {
    let out = compile_and_run_stdout(
        "fn main() {\n    for x in [1, 2, 3, 4, 5] {\n        if x == 3 {\n            continue\n        }\n        print(x)\n    }\n}",
    );
    assert_eq!(out, "1\n2\n4\n5\n");
}

#[test]
fn continue_outside_loop_rejected() {
    compile_should_fail_with(
        "fn main() {\n    continue\n}",
        "can only be used inside a loop",
    );
}

// ── range tests ──

#[test]
fn range_exclusive() {
    let out = compile_and_run_stdout(
        "fn main() {\n    for i in 0..5 {\n        print(i)\n    }\n}",
    );
    assert_eq!(out, "0\n1\n2\n3\n4\n");
}

#[test]
fn range_inclusive() {
    let out = compile_and_run_stdout(
        "fn main() {\n    for i in 0..=5 {\n        print(i)\n    }\n}",
    );
    assert_eq!(out, "0\n1\n2\n3\n4\n5\n");
}

#[test]
fn range_variable_endpoints() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let start = 2\n    let end = 6\n    for i in start..end {\n        print(i)\n    }\n}",
    );
    assert_eq!(out, "2\n3\n4\n5\n");
}

#[test]
fn range_empty() {
    let out = compile_and_run_stdout(
        "fn main() {\n    for i in 5..3 {\n        print(i)\n    }\n    print(99)\n}",
    );
    assert_eq!(out, "99\n");
}

#[test]
fn range_expression_endpoints() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let n = 3\n    for i in 0..(n * 2) {\n        print(i)\n    }\n}",
    );
    assert_eq!(out, "0\n1\n2\n3\n4\n5\n");
}

#[test]
fn range_sum() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut total = 0\n    for i in 1..=10 {\n        total = total + i\n    }\n    print(total)\n}",
    );
    assert_eq!(out, "55\n");
}

// ── else if tests ──

#[test]
fn else_if_basic() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 5\n    if x > 10 {\n        print(1)\n    } else if x > 3 {\n        print(2)\n    }\n    print(99)\n}",
    );
    assert_eq!(out, "2\n99\n");
}

#[test]
fn else_if_multi_branch() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 2\n    if x == 1 {\n        print(1)\n    } else if x == 2 {\n        print(2)\n    } else if x == 3 {\n        print(3)\n    } else if x == 4 {\n        print(4)\n    }\n}",
    );
    assert_eq!(out, "2\n");
}

#[test]
fn else_if_with_else() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 100\n    if x < 0 {\n        print(1)\n    } else if x < 10 {\n        print(2)\n    } else if x < 50 {\n        print(3)\n    } else {\n        print(4)\n    }\n}",
    );
    assert_eq!(out, "4\n");
}

#[test]
fn else_if_nested() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 5\n    let y = 10\n    if x > 10 {\n        print(1)\n    } else if x > 3 {\n        if y > 20 {\n            print(2)\n        } else if y > 5 {\n            print(3)\n        } else {\n            print(4)\n        }\n    } else {\n        print(5)\n    }\n}",
    );
    assert_eq!(out, "3\n");
}

// ── combined tests ──

#[test]
fn break_in_range_for() {
    let out = compile_and_run_stdout(
        "fn main() {\n    for i in 0..100 {\n        if i == 4 {\n            break\n        }\n        print(i)\n    }\n}",
    );
    assert_eq!(out, "0\n1\n2\n3\n");
}

#[test]
fn continue_in_range_for() {
    let out = compile_and_run_stdout(
        "fn main() {\n    for i in 0..6 {\n        if i % 2 == 0 {\n            continue\n        }\n        print(i)\n    }\n}",
    );
    assert_eq!(out, "1\n3\n5\n");
}

#[test]
fn nested_range_with_break() {
    let out = compile_and_run_stdout(
        "fn main() {\n    for i in 0..3 {\n        for j in 0..10 {\n            if j == 2 {\n                break\n            }\n            print(i * 10 + j)\n        }\n    }\n}",
    );
    assert_eq!(out, "0\n1\n10\n11\n20\n21\n");
}

// ── closure edge cases ──

#[test]
fn break_in_closure_rejected() {
    compile_should_fail_with(
        "fn main() {\n    while true {\n        let f = () => { break }\n    }\n}",
        "can only be used inside a loop",
    );
}

#[test]
fn continue_in_closure_rejected() {
    compile_should_fail_with(
        "fn main() {\n    while true {\n        let f = () => { continue }\n    }\n}",
        "can only be used inside a loop",
    );
}
