use std::process::Command;

fn plutoc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_plutoc"))
}

fn compile_and_run(source: &str) -> i32 {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("test.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&src_path, source).unwrap();

    let compile_output = plutoc()
        .arg("compile")
        .arg(&src_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();

    assert!(
        compile_output.status.success(),
        "Compilation failed: {}",
        String::from_utf8_lossy(&compile_output.stderr)
    );

    assert!(bin_path.exists(), "Binary was not created");

    let run_output = Command::new(&bin_path).output().unwrap();
    run_output.status.code().unwrap_or(-1)
}

fn compile_and_run_stdout(source: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("test.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&src_path, source).unwrap();

    let compile_output = plutoc()
        .arg("compile")
        .arg(&src_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();

    assert!(
        compile_output.status.success(),
        "Compilation failed: {}",
        String::from_utf8_lossy(&compile_output.stderr)
    );

    assert!(bin_path.exists(), "Binary was not created");

    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(run_output.status.success(), "Binary exited with non-zero status");
    String::from_utf8_lossy(&run_output.stdout).to_string()
}

fn compile_should_fail_with(source: &str, expected_msg: &str) {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("test.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&src_path, source).unwrap();

    let output = plutoc()
        .arg("compile")
        .arg(&src_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();

    assert!(!output.status.success(), "Compilation should have failed");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected_msg),
        "Expected error containing '{}', got: {}",
        expected_msg,
        stderr
    );
}

fn compile_should_fail(source: &str) {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("test.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&src_path, source).unwrap();

    let output = plutoc()
        .arg("compile")
        .arg(&src_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();

    assert!(!output.status.success(), "Compilation should have failed");
}

#[test]
fn empty_main() {
    let code = compile_and_run("fn main() { }");
    assert_eq!(code, 0);
}

#[test]
fn main_with_let() {
    let code = compile_and_run("fn main() {\n    let x = 42\n}");
    assert_eq!(code, 0);
}

#[test]
fn function_call() {
    let code = compile_and_run(
        "fn add(a: int, b: int) int {\n    return a + b\n}\n\nfn main() {\n    let x = add(1, 2)\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn arithmetic_operations() {
    let code = compile_and_run(
        "fn main() {\n    let a = 10\n    let b = 3\n    let sum = a + b\n    let diff = a - b\n    let prod = a * b\n    let quot = a / b\n    let rem = a % b\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn boolean_operations() {
    let code = compile_and_run(
        "fn main() {\n    let a = true\n    let b = false\n    let c = 1 < 2\n    let d = 3 == 3\n}",
    );
    assert_eq!(code, 0);
}

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
fn nested_function_calls() {
    let code = compile_and_run(
        "fn double(x: int) int {\n    return x * 2\n}\n\nfn add_one(x: int) int {\n    return x + 1\n}\n\nfn main() {\n    let result = add_one(double(5))\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn type_error_rejected() {
    compile_should_fail("fn main() {\n    let x: int = true\n}");
}

#[test]
fn undefined_variable_rejected() {
    compile_should_fail("fn main() {\n    let x = y\n}");
}

#[test]
fn undefined_function_rejected() {
    compile_should_fail("fn main() {\n    let x = foo(1)\n}");
}

#[test]
fn print_int() {
    let out = compile_and_run_stdout("fn main() {\n    print(42)\n}");
    assert_eq!(out, "42\n");
}

#[test]
fn print_int_expression() {
    let out = compile_and_run_stdout(
        "fn add(a: int, b: int) int {\n    return a + b\n}\n\nfn main() {\n    print(add(1, 2))\n}",
    );
    assert_eq!(out, "3\n");
}

#[test]
fn print_float() {
    let out = compile_and_run_stdout("fn main() {\n    print(3.14)\n}");
    assert_eq!(out, "3.140000\n");
}

#[test]
fn print_bool() {
    let out = compile_and_run_stdout("fn main() {\n    print(true)\n    print(false)\n}");
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn print_string() {
    let out = compile_and_run_stdout("fn main() {\n    print(\"hello world\")\n}");
    assert_eq!(out, "hello world\n");
}

#[test]
fn print_multiple() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(1)\n    print(2)\n    print(3)\n}",
    );
    assert_eq!(out, "1\n2\n3\n");
}

#[test]
fn print_in_loop() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let i = 0\n    while i < 3 {\n        print(i)\n        i = i + 1\n    }\n}",
    );
    assert_eq!(out, "0\n1\n2\n");
}

#[test]
fn print_wrong_arg_count() {
    compile_should_fail("fn main() {\n    print(1, 2)\n}");
}

#[test]
fn print_no_args() {
    compile_should_fail("fn main() {\n    print()\n}");
}

// Class tests

#[test]
fn class_construct_and_field_access() {
    let out = compile_and_run_stdout(
        "class Point {\n    x: int\n    y: int\n}\n\nfn main() {\n    let p = Point { x: 3, y: 4 }\n    print(p.x)\n    print(p.y)\n}",
    );
    assert_eq!(out, "3\n4\n");
}

#[test]
fn class_method_call() {
    let out = compile_and_run_stdout(
        "class Point {\n    x: int\n    y: int\n\n    fn sum(self) int {\n        return self.x + self.y\n    }\n}\n\nfn main() {\n    let p = Point { x: 3, y: 4 }\n    print(p.sum())\n}",
    );
    assert_eq!(out, "7\n");
}

#[test]
fn class_field_mutation() {
    let out = compile_and_run_stdout(
        "class Counter {\n    val: int\n\n    fn get(self) int {\n        return self.val\n    }\n}\n\nfn main() {\n    let c = Counter { val: 0 }\n    c.val = 42\n    print(c.get())\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn class_as_function_param() {
    let out = compile_and_run_stdout(
        "class Point {\n    x: int\n    y: int\n}\n\nfn get_x(p: Point) int {\n    return p.x\n}\n\nfn main() {\n    let p = Point { x: 99, y: 0 }\n    print(get_x(p))\n}",
    );
    assert_eq!(out, "99\n");
}

#[test]
fn class_method_with_params() {
    let out = compile_and_run_stdout(
        "class Adder {\n    base: int\n\n    fn add(self, n: int) int {\n        return self.base + n\n    }\n}\n\nfn main() {\n    let a = Adder { base: 10 }\n    print(a.add(5))\n}",
    );
    assert_eq!(out, "15\n");
}

#[test]
fn function_returning_class() {
    let out = compile_and_run_stdout(
        "class Point {\n    x: int\n    y: int\n}\n\nfn make_point(x: int, y: int) Point {\n    return Point { x: x, y: y }\n}\n\nfn main() {\n    let p = make_point(10, 20)\n    print(p.x)\n    print(p.y)\n}",
    );
    assert_eq!(out, "10\n20\n");
}

// String tests

#[test]
fn string_concatenation() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let s = \"hello \" + \"world\"\n    print(s)\n}",
    );
    assert_eq!(out, "hello world\n");
}

#[test]
fn string_len() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"hello\".len())\n}",
    );
    assert_eq!(out, "5\n");
}

#[test]
fn string_equality() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"foo\" == \"foo\")\n    print(\"foo\" == \"bar\")\n    print(\"foo\" != \"bar\")\n    print(\"foo\" != \"foo\")\n}",
    );
    assert_eq!(out, "true\nfalse\ntrue\nfalse\n");
}

#[test]
fn string_let_binding_and_print() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let s = \"hello world\"\n    print(s)\n}",
    );
    assert_eq!(out, "hello world\n");
}

#[test]
fn string_as_function_param() {
    let out = compile_and_run_stdout(
        "fn greet(name: string) string {\n    return \"hello \" + name\n}\n\nfn main() {\n    print(greet(\"world\"))\n}",
    );
    assert_eq!(out, "hello world\n");
}

#[test]
fn string_function_return() {
    let out = compile_and_run_stdout(
        "fn get_msg() string {\n    return \"hi\"\n}\n\nfn main() {\n    print(get_msg())\n}",
    );
    assert_eq!(out, "hi\n");
}

#[test]
fn string_in_struct_field() {
    let out = compile_and_run_stdout(
        "class Person {\n    name: string\n    age: int\n}\n\nfn main() {\n    let p = Person { name: \"alice\", age: 30 }\n    print(p.name)\n    print(p.age)\n}",
    );
    assert_eq!(out, "alice\n30\n");
}

#[test]
fn string_concat_len() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let s = \"ab\" + \"cde\"\n    print(s.len())\n}",
    );
    assert_eq!(out, "5\n");
}

#[test]
fn string_empty() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"\".len())\n}",
    );
    assert_eq!(out, "0\n");
}

#[test]
fn string_concat_not_int() {
    compile_should_fail("fn main() {\n    let s = \"hello\" + 42\n}");
}

// Array tests

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

// Trait tests

#[test]
fn trait_basic_impl() {
    let out = compile_and_run_stdout(
        "trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n\n    fn bar(self) int {\n        return self.val\n    }\n}\n\nfn main() {\n    let x = X { val: 42 }\n    print(x.bar())\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn trait_as_function_param() {
    let out = compile_and_run_stdout(
        "trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n\n    fn bar(self) int {\n        return self.val\n    }\n}\n\nfn process(f: Foo) int {\n    return f.bar()\n}\n\nfn main() {\n    let x = X { val: 99 }\n    print(process(x))\n}",
    );
    assert_eq!(out, "99\n");
}

#[test]
fn trait_default_method() {
    let out = compile_and_run_stdout(
        "trait Greetable {\n    fn greet(self) int {\n        return 0\n    }\n}\n\nclass X impl Greetable {\n    val: int\n}\n\nfn main() {\n    let x = X { val: 5 }\n    print(x.greet())\n}",
    );
    assert_eq!(out, "0\n");
}

#[test]
fn trait_default_method_override() {
    let out = compile_and_run_stdout(
        "trait Greetable {\n    fn greet(self) int {\n        return 0\n    }\n}\n\nclass X impl Greetable {\n    val: int\n\n    fn greet(self) int {\n        return self.val\n    }\n}\n\nfn main() {\n    let x = X { val: 77 }\n    print(x.greet())\n}",
    );
    assert_eq!(out, "77\n");
}

#[test]
fn trait_multiple_classes() {
    let out = compile_and_run_stdout(
        "trait HasVal {\n    fn get(self) int\n}\n\nclass A impl HasVal {\n    x: int\n\n    fn get(self) int {\n        return self.x\n    }\n}\n\nclass B impl HasVal {\n    y: int\n\n    fn get(self) int {\n        return self.y * 2\n    }\n}\n\nfn show(v: HasVal) {\n    print(v.get())\n}\n\nfn main() {\n    let a = A { x: 10 }\n    let b = B { y: 20 }\n    show(a)\n    show(b)\n}",
    );
    assert_eq!(out, "10\n40\n");
}

#[test]
fn trait_multiple_traits() {
    let out = compile_and_run_stdout(
        "trait HasX {\n    fn get_x(self) int\n}\n\ntrait HasY {\n    fn get_y(self) int\n}\n\nclass Point impl HasX, HasY {\n    x: int\n    y: int\n\n    fn get_x(self) int {\n        return self.x\n    }\n\n    fn get_y(self) int {\n        return self.y\n    }\n}\n\nfn show_x(h: HasX) {\n    print(h.get_x())\n}\n\nfn show_y(h: HasY) {\n    print(h.get_y())\n}\n\nfn main() {\n    let p = Point { x: 3, y: 7 }\n    show_x(p)\n    show_y(p)\n}",
    );
    assert_eq!(out, "3\n7\n");
}

#[test]
fn trait_missing_method_rejected() {
    compile_should_fail(
        "trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n}\n\nfn main() {\n}",
    );
}

#[test]
fn trait_wrong_return_type_rejected() {
    compile_should_fail(
        "trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n\n    fn bar(self) bool {\n        return true\n    }\n}\n\nfn main() {\n}",
    );
}

#[test]
fn trait_unknown_trait_rejected() {
    compile_should_fail(
        "class X impl NonExistent {\n    val: int\n}\n\nfn main() {\n}",
    );
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
        "fn main() {\n    let a = [10, 20, 30]\n    let total = 0\n    for x in a {\n        total = total + x\n    }\n    print(total)\n}",
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
    // len is captured at loop start, so pushed elements should NOT be visited
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3]\n    let count = 0\n    for x in a {\n        count = count + 1\n        a.push(x * 10)\n    }\n    print(count)\n    print(a.len())\n}",
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

#[test]
fn trait_method_with_params() {
    let out = compile_and_run_stdout(
        "trait Adder {\n    fn add(self, x: int) int\n}\n\nclass MyAdder impl Adder {\n    base: int\n\n    fn add(self, x: int) int {\n        return self.base + x\n    }\n}\n\nfn do_add(a: Adder, val: int) int {\n    return a.add(val)\n}\n\nfn main() {\n    let a = MyAdder { base: 100 }\n    print(do_add(a, 23))\n}",
    );
    assert_eq!(out, "123\n");
}

// Trait handle tests (heap-allocated trait values)

#[test]
fn trait_typed_local() {
    let out = compile_and_run_stdout(
        "trait HasVal {\n    fn get(self) int\n}\n\nclass X impl HasVal {\n    val: int\n\n    fn get(self) int {\n        return self.val\n    }\n}\n\nfn main() {\n    let x: HasVal = X { val: 42 }\n    print(x.get())\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn trait_return_value() {
    let out = compile_and_run_stdout(
        "trait HasVal {\n    fn get(self) int\n}\n\nclass X impl HasVal {\n    val: int\n\n    fn get(self) int {\n        return self.val\n    }\n}\n\nfn make() HasVal {\n    return X { val: 77 }\n}\n\nfn main() {\n    print(make().get())\n}",
    );
    assert_eq!(out, "77\n");
}

#[test]
fn trait_forward() {
    let out = compile_and_run_stdout(
        "trait HasVal {\n    fn get(self) int\n}\n\nclass X impl HasVal {\n    val: int\n\n    fn get(self) int {\n        return self.val\n    }\n}\n\nfn show(v: HasVal) {\n    print(v.get())\n}\n\nfn forward(v: HasVal) {\n    show(v)\n}\n\nfn main() {\n    let x = X { val: 55 }\n    forward(x)\n}",
    );
    assert_eq!(out, "55\n");
}

#[test]
fn trait_method_on_call_result() {
    let out = compile_and_run_stdout(
        "trait HasVal {\n    fn get(self) int\n}\n\nclass X impl HasVal {\n    val: int\n\n    fn get(self) int {\n        return self.val\n    }\n}\n\nfn make_val() HasVal {\n    return X { val: 88 }\n}\n\nfn main() {\n    print(make_val().get())\n}",
    );
    assert_eq!(out, "88\n");
}

#[test]
fn trait_local_reassignment() {
    let out = compile_and_run_stdout(
        "trait HasVal {\n    fn get(self) int\n}\n\nclass A impl HasVal {\n    x: int\n\n    fn get(self) int {\n        return self.x\n    }\n}\n\nclass B impl HasVal {\n    y: int\n\n    fn get(self) int {\n        return self.y * 2\n    }\n}\n\nfn main() {\n    let v: HasVal = A { x: 10 }\n    print(v.get())\n    v = B { y: 20 }\n    print(v.get())\n}",
    );
    assert_eq!(out, "10\n40\n");
}

#[test]
fn trait_polymorphic_dispatch() {
    let out = compile_and_run_stdout(
        "trait Animal {\n    fn speak(self) int\n}\n\nclass Dog impl Animal {\n    volume: int\n\n    fn speak(self) int {\n        return self.volume\n    }\n}\n\nclass Cat impl Animal {\n    volume: int\n\n    fn speak(self) int {\n        return self.volume * 3\n    }\n}\n\nfn make_sound(a: Animal) {\n    print(a.speak())\n}\n\nfn main() {\n    let d = Dog { volume: 10 }\n    let c = Cat { volume: 5 }\n    make_sound(d)\n    make_sound(c)\n}",
    );
    assert_eq!(out, "10\n15\n");
}

#[test]
fn trait_default_method_via_handle() {
    let out = compile_and_run_stdout(
        "trait Greeter {\n    fn greet(self) int {\n        return 0\n    }\n}\n\nclass X impl Greeter {\n    val: int\n}\n\nfn show(g: Greeter) {\n    print(g.greet())\n}\n\nfn main() {\n    let x = X { val: 5 }\n    show(x)\n}",
    );
    assert_eq!(out, "0\n");
}

// Arithmetic output verification tests

#[test]
fn arithmetic_add_output() {
    let out = compile_and_run_stdout("fn main() {\n    print(10 + 3)\n}");
    assert_eq!(out, "13\n");
}

#[test]
fn arithmetic_sub_output() {
    let out = compile_and_run_stdout("fn main() {\n    print(10 - 3)\n}");
    assert_eq!(out, "7\n");
}

#[test]
fn arithmetic_mul_output() {
    let out = compile_and_run_stdout("fn main() {\n    print(10 * 3)\n}");
    assert_eq!(out, "30\n");
}

#[test]
fn arithmetic_div_output() {
    let out = compile_and_run_stdout("fn main() {\n    print(10 / 3)\n}");
    assert_eq!(out, "3\n");
}

#[test]
fn arithmetic_mod_output() {
    let out = compile_and_run_stdout("fn main() {\n    print(10 % 3)\n}");
    assert_eq!(out, "1\n");
}

#[test]
fn float_arithmetic() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(1.5 + 2.5)\n    print(5.0 - 1.5)\n    print(2.0 * 3.0)\n    print(7.0 / 2.0)\n}",
    );
    assert_eq!(out, "4.000000\n3.500000\n6.000000\n3.500000\n");
}

// Comparison operator tests

#[test]
fn comparison_greater_than() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(5 > 3)\n    print(3 > 5)\n    print(3 > 3)\n}",
    );
    assert_eq!(out, "true\nfalse\nfalse\n");
}

#[test]
fn comparison_less_than_eq() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(3 <= 5)\n    print(5 <= 5)\n    print(6 <= 5)\n}",
    );
    assert_eq!(out, "true\ntrue\nfalse\n");
}

#[test]
fn comparison_greater_than_eq() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(5 >= 3)\n    print(5 >= 5)\n    print(4 >= 5)\n}",
    );
    assert_eq!(out, "true\ntrue\nfalse\n");
}

#[test]
fn int_equality() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(42 == 42)\n    print(42 == 43)\n    print(42 != 43)\n    print(42 != 42)\n}",
    );
    assert_eq!(out, "true\nfalse\ntrue\nfalse\n");
}

// Logical operator tests

#[test]
fn logical_and() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(true && true)\n    print(true && false)\n    print(false && true)\n    print(false && false)\n}",
    );
    assert_eq!(out, "true\nfalse\nfalse\nfalse\n");
}

#[test]
fn logical_or() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(true || true)\n    print(true || false)\n    print(false || true)\n    print(false || false)\n}",
    );
    assert_eq!(out, "true\ntrue\ntrue\nfalse\n");
}

// Unary operator tests

#[test]
fn unary_negation() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 5\n    print(-x)\n    print(-10)\n}",
    );
    assert_eq!(out, "-5\n-10\n");
}

#[test]
fn unary_not() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(!true)\n    print(!false)\n}",
    );
    assert_eq!(out, "false\ntrue\n");
}

// Control flow with output verification

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

// Return value tests

#[test]
fn void_return() {
    let out = compile_and_run_stdout(
        "fn early(x: int) {\n    if x > 0 {\n        print(1)\n        return\n    }\n    print(2)\n}\n\nfn main() {\n    early(5)\n    early(-1)\n}",
    );
    assert_eq!(out, "1\n2\n");
}

#[test]
fn multiple_return_paths() {
    let out = compile_and_run_stdout(
        "fn classify(x: int) string {\n    if x > 0 {\n        return \"positive\"\n    }\n    if x < 0 {\n        return \"negative\"\n    }\n    return \"zero\"\n}\n\nfn main() {\n    print(classify(5))\n    print(classify(-3))\n    print(classify(0))\n}",
    );
    assert_eq!(out, "positive\nnegative\nzero\n");
}

// Comments

#[test]
fn comments_ignored() {
    let out = compile_and_run_stdout(
        "// this is a comment\nfn main() {\n    // another comment\n    let x = 42 // inline comment\n    print(x)\n}",
    );
    assert_eq!(out, "42\n");
}

// Parenthesized expressions

#[test]
fn parenthesized_expressions() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print((2 + 3) * 4)\n    print(2 + 3 * 4)\n}",
    );
    assert_eq!(out, "20\n14\n");
}

// Recursive function

#[test]
fn recursive_function() {
    let out = compile_and_run_stdout(
        "fn factorial(n: int) int {\n    if n <= 1 {\n        return 1\n    }\n    return n * factorial(n - 1)\n}\n\nfn main() {\n    print(factorial(5))\n}",
    );
    assert_eq!(out, "120\n");
}

// Variable reassignment

#[test]
fn variable_reassignment() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 1\n    print(x)\n    x = 2\n    print(x)\n    x = x + 10\n    print(x)\n}",
    );
    assert_eq!(out, "1\n2\n12\n");
}

// Additional error rejection tests

#[test]
fn wrong_arg_count_rejected() {
    compile_should_fail(
        "fn add(a: int, b: int) int {\n    return a + b\n}\n\nfn main() {\n    let x = add(1)\n}",
    );
}

#[test]
fn return_type_mismatch_rejected() {
    compile_should_fail(
        "fn foo() int {\n    return true\n}\n\nfn main() {\n    foo()\n}",
    );
}

#[test]
fn arg_type_mismatch_rejected() {
    compile_should_fail(
        "fn foo(x: int) int {\n    return x\n}\n\nfn main() {\n    foo(\"hello\")\n}",
    );
}

#[test]
fn assign_type_mismatch_rejected() {
    compile_should_fail(
        "fn main() {\n    let x = 42\n    x = true\n}",
    );
}

// Class with multiple methods

#[test]
fn class_multiple_methods() {
    let out = compile_and_run_stdout(
        "class Rect {\n    w: int\n    h: int\n\n    fn area(self) int {\n        return self.w * self.h\n    }\n\n    fn perimeter(self) int {\n        return 2 * (self.w + self.h)\n    }\n}\n\nfn main() {\n    let r = Rect { w: 3, h: 4 }\n    print(r.area())\n    print(r.perimeter())\n}",
    );
    assert_eq!(out, "12\n14\n");
}

// Chained string concatenation

#[test]
fn string_concat_chain() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let s = \"a\" + \"b\" + \"c\"\n    print(s)\n}",
    );
    assert_eq!(out, "abc\n");
}

// Bool equality

#[test]
fn bool_equality() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(true == true)\n    print(true == false)\n    print(false != true)\n}",
    );
    assert_eq!(out, "true\nfalse\ntrue\n");
}

// String interpolation

#[test]
fn string_interp_basic() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let name = \"alice\"\n    print(\"hello {name}\")\n}",
    );
    assert_eq!(out, "hello alice\n");
}

#[test]
fn string_interp_int() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 42\n    print(\"x is {x}\")\n}",
    );
    assert_eq!(out, "x is 42\n");
}

#[test]
fn string_interp_float() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let pi = 3.14\n    print(\"pi is {pi}\")\n}",
    );
    assert_eq!(out, "pi is 3.140000\n");
}

#[test]
fn string_interp_bool() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let flag = true\n    print(\"flag is {flag}\")\n}",
    );
    assert_eq!(out, "flag is true\n");
}

#[test]
fn string_interp_expr() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = 1\n    let b = 2\n    print(\"sum is {a + b}\")\n}",
    );
    assert_eq!(out, "sum is 3\n");
}

#[test]
fn string_interp_multiple() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = 1\n    let b = 2\n    print(\"{a} + {b} = {a + b}\")\n}",
    );
    assert_eq!(out, "1 + 2 = 3\n");
}

#[test]
fn string_interp_no_interp() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"plain string\")\n}",
    );
    assert_eq!(out, "plain string\n");
}

#[test]
fn string_interp_escaped_braces() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"use {{braces}}\")\n}",
    );
    assert_eq!(out, "use {braces}\n");
}

#[test]
fn string_interp_concat() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let name = \"alice\"\n    print(\"hi {name}\" + \"!\")\n}",
    );
    assert_eq!(out, "hi alice!\n");
}

#[test]
fn string_interp_class_rejected() {
    compile_should_fail_with(
        "class Foo {\n    x: int\n}\n\nfn main() {\n    let p = Foo { x: 1 }\n    let s = \"value is {p}\"\n}",
        "cannot interpolate",
    );
}

#[test]
fn string_interp_trailing_tokens_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let a = 1\n    let s = \"{a b}\"\n}",
        "unexpected tokens",
    );
}

#[test]
fn string_interp_unterminated_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let name = \"alice\"\n    let s = \"hello {name\"\n}",
        "unterminated",
    );
}

#[test]
fn string_interp_stray_close_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let s = \"hello }\"\n}",
        "unexpected '}'",
    );
}
