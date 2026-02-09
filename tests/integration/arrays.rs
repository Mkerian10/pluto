mod common;
use common::{compile_and_run_stdout, compile_should_fail};

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
    assert_eq!(out, "1.500000\n2.500000\n");
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
        "fn main() {\n    let a = [0, 0, 0]\n    let i = 0\n    while i < 3 {\n        a[i] = i * 10\n        i = i + 1\n    }\n    print(a[0])\n    print(a[1])\n    print(a[2])\n}",
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
    compile_should_fail("fn main() {\n    let a = [1, true]\n}");
}

#[test]
fn array_index_non_int_rejected() {
    compile_should_fail("fn main() {\n    let a = [1, 2, 3]\n    let x = a[true]\n}");
}

#[test]
fn array_push_wrong_type_rejected() {
    compile_should_fail("fn main() {\n    let a = [1, 2]\n    a.push(\"x\")\n}");
}
