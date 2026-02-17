mod common;
use common::*;

#[test]
fn map_literal_and_get() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let m = Map<string, int> { "a": 1, "b": 2 }
    print(m["a"])
    print(m["b"])
    return 0
}
"#);
    assert_eq!(out, "1\n2\n");
}

#[test]
fn map_empty_literal() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let m = Map<string, int> {}
    print(m.len())
    return 0
}
"#);
    assert_eq!(out, "0\n");
}

#[test]
fn map_insert_and_get() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let m = Map<string, int> {}
    m.insert("x", 42)
    print(m["x"])
    return 0
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn map_contains() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let m = Map<string, int> { "a": 1 }
    print(m.contains("a"))
    print(m.contains("b"))
    return 0
}
"#);
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn map_remove() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let m = Map<string, int> { "a": 1, "b": 2 }
    m.remove("a")
    print(m.len())
    print(m.contains("a"))
    print(m.contains("b"))
    return 0
}
"#);
    assert_eq!(out, "1\nfalse\ntrue\n");
}

#[test]
fn map_len() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let m = Map<string, int> { "a": 1, "b": 2, "c": 3 }
    print(m.len())
    return 0
}
"#);
    assert_eq!(out, "3\n");
}

#[test]
fn map_overwrite() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let m = Map<string, int> { "a": 1 }
    m.insert("a", 99)
    print(m["a"])
    print(m.len())
    return 0
}
"#);
    assert_eq!(out, "99\n1\n");
}

#[test]
fn map_index_assign() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let m = Map<string, int> {}
    m["hello"] = 42
    print(m["hello"])
    m["hello"] = 100
    print(m["hello"])
    print(m.len())
    return 0
}
"#);
    assert_eq!(out, "42\n100\n1\n");
}

#[test]
fn map_int_keys() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let m = Map<int, string> { 1: "one", 2: "two" }
    print(m[1])
    print(m[2])
    return 0
}
"#);
    assert_eq!(out, "one\ntwo\n");
}

#[test]
fn map_bool_keys() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let m = Map<bool, int> { true: 1, false: 0 }
    print(m[true])
    print(m[false])
    return 0
}
"#);
    assert_eq!(out, "1\n0\n");
}

#[test]
fn map_keys_method() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let m = Map<int, string> { 10: "ten", 20: "twenty" }
    let keys = m.keys()
    print(keys.len())
    return 0
}
"#);
    assert_eq!(out, "2\n");
}

#[test]
fn map_values_method() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let m = Map<int, string> { 10: "ten", 20: "twenty" }
    let vals = m.values()
    print(vals.len())
    return 0
}
"#);
    assert_eq!(out, "2\n");
}

#[test]
fn map_iterate_keys() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let m = Map<int, int> { 1: 10, 2: 20, 3: 30 }
    let mut total = 0
    for k in m.keys() {
        total = total + m[k]
    }
    print(total)
    return 0
}
"#);
    assert_eq!(out, "60\n");
}

#[test]
fn map_as_function_param() {
    let out = compile_and_run_stdout(r#"
fn sum_values(m: Map<string, int>) int {
    let mut total = 0
    for v in m.values() {
        total = total + v
    }
    return total
}

fn main() int {
    let m = Map<string, int> { "a": 10, "b": 20 }
    print(sum_values(m))
    return 0
}
"#);
    assert_eq!(out, "30\n");
}

#[test]
fn map_as_function_return() {
    let out = compile_and_run_stdout(r#"
fn make_map() Map<string, int> {
    return Map<string, int> { "x": 42 }
}

fn main() int {
    let m = make_map()
    print(m["x"])
    return 0
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn map_wrong_key_type_fails() {
    compile_should_fail_with(r#"
fn main() int {
    let m = Map<string, int> { "a": 1 }
    print(m[42])
    return 0
}
"#, "map key type mismatch");
}

#[test]
fn map_wrong_value_type_fails() {
    compile_should_fail_with(r#"
fn main() int {
    let m = Map<string, int> { "a": "hello" }
    return 0
}
"#, "map value type mismatch");
}

#[test]
fn map_non_hashable_key_fails() {
    compile_should_fail_with(r#"
fn main() int {
    let m = Map<[int], int> {}
    return 0
}
"#, "cannot be used as a map/set key");
}

#[test]
fn map_grow_rehash() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let m = Map<int, int> {}
    let i = 0
    while i < 100 {
        m.insert(i, i * 2)
        i = i + 1
    }
    print(m.len())
    print(m[0])
    print(m[50])
    print(m[99])
    return 0
}
"#);
    assert_eq!(out, "100\n0\n100\n198\n");
}
