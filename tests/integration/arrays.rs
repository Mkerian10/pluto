mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

#[test]
fn array_literal_and_index() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [10, 20, 30]\n    print(a[0])\n    print(a[1])\n    print(a[2])\n}",
    );
    assert_eq!(out, "10\n20\n30\n");
}

#[test]
fn array_len() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3, 4, 5]\n    print(a.len())\n}",
    );
    assert_eq!(out, "5\n");
}

#[test]
fn array_push_and_len() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3]\n    a.push(4)\n    print(a.len())\n    print(a[3])\n}",
    );
    assert_eq!(out, "4\n4\n");
}

#[test]
fn array_index_assign() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [10, 20, 30]\n    a[1] = 99\n    print(a[0])\n    print(a[1])\n    print(a[2])\n}",
    );
    assert_eq!(out, "10\n99\n30\n");
}

#[test]
fn array_of_strings() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [\"hello\", \"world\"]\n    print(a[0])\n    print(a[1])\n}",
    );
    assert_eq!(out, "hello\nworld\n");
}

#[test]
fn array_of_bools() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [true, false, true]\n    print(a[0])\n    print(a[1])\n    print(a[2])\n}",
    );
    assert_eq!(out, "true\nfalse\ntrue\n");
}

#[test]
fn array_of_floats() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1.5, 2.5]\n    print(a[0])\n    print(a[1])\n}",
    );
    assert_eq!(out, "1.5\n2.5\n");
}

#[test]
fn array_as_function_param() {
    let out = compile_and_run_stdout(
        "fn first(a: [int]) int {\n    return a[0]\n}\n\nfn main() {\n    let a = [42, 99]\n    print(first(a))\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn array_as_return_value() {
    let out = compile_and_run_stdout(
        "fn make() [int] {\n    return [10, 20, 30]\n}\n\nfn main() {\n    let a = make()\n    print(a[1])\n    print(a.len())\n}",
    );
    assert_eq!(out, "20\n3\n");
}

#[test]
fn array_in_while_loop() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut a = [0, 0, 0]\n    let mut i = 0\n    while i < 3 {\n        a[i] = i * 10\n        i = i + 1\n    }\n    print(a[0])\n    print(a[1])\n    print(a[2])\n}",
    );
    assert_eq!(out, "0\n10\n20\n");
}

#[test]
fn array_in_struct_field() {
    let out = compile_and_run_stdout(
        "class Bag {\n    items: [int]\n}\n\nfn main() {\n    let b = Bag { items: [1, 2, 3] }\n    print(b.items[0])\n    print(b.items.len())\n}",
    );
    assert_eq!(out, "1\n3\n");
}

#[test]
fn array_mixed_types_rejected() {
    compile_should_fail_with("fn main() {\n    let a = [1, true]\n}", "array element type mismatch: expected int, found bool");
}

#[test]
fn array_index_non_int_rejected() {
    compile_should_fail_with("fn main() {\n    let a = [1, 2, 3]\n    let x = a[true]\n}", "array index must be int, found bool");
}

#[test]
fn array_push_wrong_type_rejected() {
    compile_should_fail_with("fn main() {\n    let a = [1, 2]\n    a.push(\"x\")\n}", "push(): expected int, found string");
}

// ── pop ──────────────────────────────────────────────────────────────────────

#[test]
fn array_pop_returns_last() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3]\n    let x = a.pop()\n    print(x)\n    print(a.len())\n}",
    );
    assert_eq!(out, "3\n2\n");
}

#[test]
fn array_pop_until_empty() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [10, 20]\n    let b = a.pop()\n    let c = a.pop()\n    print(b)\n    print(c)\n    print(a.len())\n}",
    );
    assert_eq!(out, "20\n10\n0\n");
}

#[test]
fn array_pop_single_element() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [42]\n    let x = a.pop()\n    print(x)\n    print(a.len())\n}",
    );
    assert_eq!(out, "42\n0\n");
}

// ── last / first ─────────────────────────────────────────────────────────────

#[test]
fn array_last() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3]\n    print(a.last())\n}",
    );
    assert_eq!(out, "3\n");
}

#[test]
fn array_first() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3]\n    print(a.first())\n}",
    );
    assert_eq!(out, "1\n");
}

#[test]
fn array_first_last_single() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [99]\n    print(a.first())\n    print(a.last())\n}",
    );
    assert_eq!(out, "99\n99\n");
}

// ── is_empty ─────────────────────────────────────────────────────────────────

#[test]
fn array_is_empty_false() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2]\n    print(a.is_empty())\n}",
    );
    assert_eq!(out, "false\n");
}

#[test]
fn array_is_empty_true() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1]\n    a.pop()\n    print(a.is_empty())\n}",
    );
    assert_eq!(out, "true\n");
}

// ── clear ────────────────────────────────────────────────────────────────────

#[test]
fn array_clear() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3]\n    a.clear()\n    print(a.len())\n    print(a.is_empty())\n}",
    );
    assert_eq!(out, "0\ntrue\n");
}

// ── remove_at ────────────────────────────────────────────────────────────────

#[test]
fn array_remove_at_middle() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [10, 20, 30, 40]\n    let x = a.remove_at(1)\n    print(x)\n    print(a.len())\n    print(a[0])\n    print(a[1])\n    print(a[2])\n}",
    );
    assert_eq!(out, "20\n3\n10\n30\n40\n");
}

#[test]
fn array_remove_at_first() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [10, 20, 30]\n    let x = a.remove_at(0)\n    print(x)\n    print(a[0])\n}",
    );
    assert_eq!(out, "10\n20\n");
}

#[test]
fn array_remove_at_last() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [10, 20, 30]\n    let x = a.remove_at(2)\n    print(x)\n    print(a.len())\n}",
    );
    assert_eq!(out, "30\n2\n");
}

// ── insert_at ────────────────────────────────────────────────────────────────

#[test]
fn array_insert_at_beginning() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [2, 3]\n    a.insert_at(0, 1)\n    print(a[0])\n    print(a[1])\n    print(a[2])\n}",
    );
    assert_eq!(out, "1\n2\n3\n");
}

#[test]
fn array_insert_at_middle() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 3]\n    a.insert_at(1, 2)\n    print(a[0])\n    print(a[1])\n    print(a[2])\n}",
    );
    assert_eq!(out, "1\n2\n3\n");
}

#[test]
fn array_insert_at_end() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2]\n    a.insert_at(2, 3)\n    print(a[0])\n    print(a[1])\n    print(a[2])\n}",
    );
    assert_eq!(out, "1\n2\n3\n");
}

// ── slice ────────────────────────────────────────────────────────────────────

#[test]
fn array_slice_middle() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3, 4, 5]\n    let b = a.slice(1, 4)\n    print(b.len())\n    print(b[0])\n    print(b[1])\n    print(b[2])\n}",
    );
    assert_eq!(out, "3\n2\n3\n4\n");
}

#[test]
fn array_slice_full_copy() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3]\n    let b = a.slice(0, 3)\n    print(b.len())\n    print(b[0])\n    print(b[2])\n}",
    );
    assert_eq!(out, "3\n1\n3\n");
}

#[test]
fn array_slice_empty() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3]\n    let b = a.slice(2, 2)\n    print(b.len())\n}",
    );
    assert_eq!(out, "0\n");
}

// ── reverse ──────────────────────────────────────────────────────────────────

#[test]
fn array_reverse() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3]\n    a.reverse()\n    print(a[0])\n    print(a[1])\n    print(a[2])\n}",
    );
    assert_eq!(out, "3\n2\n1\n");
}

#[test]
fn array_reverse_single() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [42]\n    a.reverse()\n    print(a[0])\n}",
    );
    assert_eq!(out, "42\n");
}

// ── contains ─────────────────────────────────────────────────────────────────

#[test]
fn array_contains_int_found() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3]\n    print(a.contains(2))\n    print(a.contains(99))\n}",
    );
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn array_contains_string() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [\"hello\", \"world\"]\n    print(a.contains(\"hello\"))\n    print(a.contains(\"nope\"))\n}",
    );
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn array_contains_bool() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [true, false]\n    print(a.contains(true))\n}",
    );
    assert_eq!(out, "true\n");
}

// ── index_of ─────────────────────────────────────────────────────────────────

#[test]
fn array_index_of_found() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [10, 20, 30]\n    print(a.index_of(20))\n}",
    );
    assert_eq!(out, "1\n");
}

#[test]
fn array_index_of_not_found() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [10, 20, 30]\n    print(a.index_of(99))\n}",
    );
    assert_eq!(out, "-1\n");
}

#[test]
fn array_index_of_first_occurrence() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3, 2, 1]\n    print(a.index_of(2))\n}",
    );
    assert_eq!(out, "1\n");
}

#[test]
fn array_index_of_string() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [\"a\", \"b\", \"c\"]\n    print(a.index_of(\"b\"))\n    print(a.index_of(\"z\"))\n}",
    );
    assert_eq!(out, "1\n-1\n");
}

// ── type errors ──────────────────────────────────────────────────────────────

#[test]
fn array_contains_wrong_type_rejected() {
    compile_should_fail_with("fn main() {\n    let a = [1, 2]\n    a.contains(\"x\")\n}", "contains(): expected int, found string");
}

#[test]
fn array_index_of_wrong_type_rejected() {
    compile_should_fail_with("fn main() {\n    let a = [1, 2]\n    a.index_of(\"x\")\n}", "index_of(): expected int, found string");
}

#[test]
fn array_remove_at_wrong_type_rejected() {
    compile_should_fail_with("fn main() {\n    let a = [1, 2]\n    a.remove_at(\"x\")\n}", "remove_at(): expected int index, found string");
}

#[test]
fn array_insert_at_wrong_value_type_rejected() {
    compile_should_fail_with("fn main() {\n    let a = [1, 2]\n    a.insert_at(0, \"x\")\n}", "insert_at(): expected int, found string");
}
