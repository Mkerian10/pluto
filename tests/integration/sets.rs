mod common;
use common::*;

#[test]
fn set_literal_and_contains() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let s = Set<int> { 1, 2, 3 }
    print(s.contains(1))
    print(s.contains(4))
    return 0
}
"#);
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn set_empty_literal() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let s = Set<int> {}
    print(s.len())
    return 0
}
"#);
    assert_eq!(out, "0\n");
}

#[test]
fn set_insert() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let s = Set<int> {}
    s.insert(42)
    print(s.contains(42))
    print(s.len())
    return 0
}
"#);
    assert_eq!(out, "true\n1\n");
}

#[test]
fn set_remove() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let s = Set<int> { 1, 2, 3 }
    s.remove(2)
    print(s.len())
    print(s.contains(2))
    print(s.contains(1))
    return 0
}
"#);
    assert_eq!(out, "2\nfalse\ntrue\n");
}

#[test]
fn set_len() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let s = Set<string> { "a", "b", "c" }
    print(s.len())
    return 0
}
"#);
    assert_eq!(out, "3\n");
}

#[test]
fn set_duplicate_insert() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let s = Set<int> { 1, 2, 3 }
    s.insert(2)
    print(s.len())
    return 0
}
"#);
    assert_eq!(out, "3\n");
}

#[test]
fn set_string_elements() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let s = Set<string> { "hello", "world" }
    print(s.contains("hello"))
    print(s.contains("foo"))
    return 0
}
"#);
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn set_to_array() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let s = Set<int> { 10, 20, 30 }
    let arr = s.to_array()
    print(arr.len())
    return 0
}
"#);
    assert_eq!(out, "3\n");
}

#[test]
fn set_iterate() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let s = Set<int> { 1, 2, 3 }
    let total = 0
    for x in s.to_array() {
        total = total + x
    }
    print(total)
    return 0
}
"#);
    assert_eq!(out, "6\n");
}

#[test]
fn set_as_function_param() {
    let out = compile_and_run_stdout(r#"
fn count(s: Set<int>) int {
    return s.len()
}

fn main() int {
    let s = Set<int> { 1, 2, 3 }
    print(count(s))
    return 0
}
"#);
    assert_eq!(out, "3\n");
}

#[test]
fn set_non_hashable_element_fails() {
    compile_should_fail_with(r#"
fn main() int {
    let s = Set<[int]> {}
    return 0
}
"#, "cannot be used as a map/set key");
}

#[test]
fn set_wrong_element_type_fails() {
    compile_should_fail_with(r#"
fn main() int {
    let s = Set<int> { 1, "hello" }
    return 0
}
"#, "set element type mismatch");
}
