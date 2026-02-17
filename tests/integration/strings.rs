mod common;
use common::{compile_and_run_stdout, compile_and_run_output, compile_should_fail, compile_should_fail_with};

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
    compile_should_fail_with("fn main() {\n    let s = \"hello\" + 42\n}", "type mismatch");
}

#[test]
fn string_concat_chain() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let s = \"a\" + \"b\" + \"c\"\n    print(s)\n}",
    );
    assert_eq!(out, "abc\n");
}

// String interpolation

#[test]
fn string_interp_basic() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let name = \"alice\"\n    print(f\"hello {name}\")\n}",
    );
    assert_eq!(out, "hello alice\n");
}

#[test]
fn string_interp_int() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 42\n    print(f\"x is {x}\")\n}",
    );
    assert_eq!(out, "x is 42\n");
}

#[test]
fn string_interp_float() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let pi = 3.14\n    print(f\"pi is {pi}\")\n}",
    );
    assert_eq!(out, "pi is 3.140000\n");
}

#[test]
fn string_interp_bool() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let flag = true\n    print(f\"flag is {flag}\")\n}",
    );
    assert_eq!(out, "flag is true\n");
}

#[test]
fn string_interp_expr() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = 1\n    let b = 2\n    print(f\"sum is {a + b}\")\n}",
    );
    assert_eq!(out, "sum is 3\n");
}

#[test]
fn string_interp_multiple() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = 1\n    let b = 2\n    print(f\"{a} + {b} = {a + b}\")\n}",
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
        "fn main() {\n    print(f\"use {{braces}}\")\n}",
    );
    assert_eq!(out, "use {braces}\n");
}

#[test]
fn string_interp_concat() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let name = \"alice\"\n    print(f\"hi {name}\" + \"!\")\n}",
    );
    assert_eq!(out, "hi alice!\n");
}

#[test]
fn string_interp_class_rejected() {
    compile_should_fail_with(
        "class Foo {\n    x: int\n}\n\nfn main() {\n    let p = Foo { x: 1 }\n    let s = f\"value is {p}\"\n}",
        "cannot interpolate",
    );
}

#[test]
fn string_interp_trailing_tokens_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let a = 1\n    let s = f\"{a b}\"\n}",
        "unexpected tokens",
    );
}

#[test]
fn string_interp_unterminated_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let name = \"alice\"\n    let s = f\"hello {name\"\n}",
        "unterminated",
    );
}

#[test]
fn string_interp_stray_close_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let s = f\"hello }\"\n}",
        "unexpected '}'",
    );
}

// ── Built-in string methods ──

#[test]
fn string_contains() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"hello world\".contains(\"world\"))\n    print(\"hello world\".contains(\"xyz\"))\n}",
    );
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn string_starts_with() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"hello world\".starts_with(\"hello\"))\n    print(\"hello world\".starts_with(\"world\"))\n}",
    );
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn string_ends_with() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"hello world\".ends_with(\"world\"))\n    print(\"hello world\".ends_with(\"hello\"))\n}",
    );
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn string_index_of() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"hello world\".index_of(\"world\"))\n    print(\"hello world\".index_of(\"xyz\"))\n}",
    );
    assert_eq!(out, "6\n-1\n");
}

#[test]
fn string_substring() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"hello world\".substring(6, 5))\n}",
    );
    assert_eq!(out, "world\n");
}

#[test]
fn string_substring_clamp() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"hello\".substring(3, 100))\n    print(\"hello\".substring(10, 5))\n}",
    );
    assert_eq!(out, "lo\n\n");
}

#[test]
fn string_trim() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"  hello  \".trim())\n}",
    );
    assert_eq!(out, "hello\n");
}

#[test]
fn string_to_upper() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"hello\".to_upper())\n}",
    );
    assert_eq!(out, "HELLO\n");
}

#[test]
fn string_to_lower() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"HELLO\".to_lower())\n}",
    );
    assert_eq!(out, "hello\n");
}

#[test]
fn string_replace() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"hello world\".replace(\"world\", \"pluto\"))\n}",
    );
    assert_eq!(out, "hello pluto\n");
}

#[test]
fn string_split() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let parts = \"a,b,c\".split(\",\")\n    print(parts.len())\n    print(parts[0])\n    print(parts[1])\n    print(parts[2])\n}",
    );
    assert_eq!(out, "3\na\nb\nc\n");
}

#[test]
fn string_split_empty_delim() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let chars = \"abc\".split(\"\")\n    print(chars.len())\n    print(chars[0])\n    print(chars[1])\n    print(chars[2])\n}",
    );
    assert_eq!(out, "3\na\nb\nc\n");
}

#[test]
fn string_char_at() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"hello\".char_at(0))\n    print(\"hello\".char_at(4))\n}",
    );
    assert_eq!(out, "h\no\n");
}

// ── String indexing ──

#[test]
fn string_index_first() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let s = \"hello\"\n    print(s[0])\n}",
    );
    assert_eq!(out, "h\n");
}

#[test]
fn string_index_last() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let s = \"hello\"\n    print(s[s.len() - 1])\n}",
    );
    assert_eq!(out, "o\n");
}

#[test]
fn string_index_let_binding() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let s = \"hello\"\n    let c = s[1]\n    print(c)\n}",
    );
    assert_eq!(out, "e\n");
}

// ── String iteration ──

#[test]
fn string_for_loop() {
    let out = compile_and_run_stdout(
        "fn main() {\n    for c in \"abc\" {\n        print(c)\n    }\n}",
    );
    assert_eq!(out, "a\nb\nc\n");
}

#[test]
#[ignore]
fn string_for_loop_accumulate() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let result = \"\"\n    for c in \"hello\" {\n        result = result + c + \"-\"\n    }\n    print(result)\n}",
    );
    assert_eq!(out, "h-e-l-l-o-\n");
}

#[test]
fn string_for_loop_count() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut count = 0\n    for c in \"hello world\" {\n        count = count + 1\n    }\n    print(count)\n}",
    );
    assert_eq!(out, "11\n");
}

#[test]
fn string_for_loop_empty() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut count = 0\n    for c in \"\" {\n        count = count + 1\n    }\n    print(count)\n}",
    );
    assert_eq!(out, "0\n");
}

// ── Method chaining ──

#[test]
fn string_method_chain() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"  Hello World  \".trim().to_lower().contains(\"hello\"))\n}",
    );
    assert_eq!(out, "true\n");
}

#[test]
fn string_replace_chain() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"aXbXc\".replace(\"X\", \",\").split(\",\").len())\n}",
    );
    assert_eq!(out, "3\n");
}

// ── Edge cases ──

#[test]
fn string_contains_empty() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"\".contains(\"\"))\n    print(\"hello\".contains(\"\"))\n}",
    );
    assert_eq!(out, "true\ntrue\n");
}

#[test]
fn string_index_of_not_found() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"hello\".index_of(\"xyz\"))\n}",
    );
    assert_eq!(out, "-1\n");
}

#[test]
fn string_trim_empty() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"   \".trim().len())\n}",
    );
    assert_eq!(out, "0\n");
}

// ── Runtime abort tests (OOB) ──

#[test]
fn string_index_oob_aborts() {
    let (_, _, code) = compile_and_run_output(
        "fn main() {\n    let s = \"hello\"\n    print(s[10])\n}",
    );
    assert_ne!(code, 0, "OOB index should abort");
}

#[test]
fn string_char_at_oob_aborts() {
    let (_, _, code) = compile_and_run_output(
        "fn main() {\n    print(\"hello\".char_at(100))\n}",
    );
    assert_ne!(code, 0, "OOB char_at should abort");
}

// ── Compile error tests ──

#[test]
fn string_index_wrong_type() {
    compile_should_fail_with(
        "fn main() {\n    let s = \"hello\"\n    print(s[true])\n}",
        "string index must be int",
    );
}

#[test]
fn string_contains_wrong_arg_type() {
    compile_should_fail_with(
        "fn main() {\n    print(\"hello\".contains(42))\n}",
        "expected string, found int",
    );
}

#[test]
fn string_unknown_method() {
    compile_should_fail_with(
        "fn main() {\n    print(\"hello\".fake())\n}",
        "string has no method 'fake'",
    );
}

#[test]
fn string_index_assign_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let s = \"hello\"\n    s[0] = \"x\"\n}",
        "index assignment on non-indexable type string",
    );
}

// ── to_int / to_float (returns int? / float?) ─────────────────

#[test]
fn string_to_int_basic() {
    let out = compile_and_run_stdout(
        r#"fn main() int? {
    let v = "42".to_int()?
    print(v)
    return none
}"#,
    );
    assert_eq!(out, "42\n");
}

#[test]
fn string_to_int_negative() {
    let out = compile_and_run_stdout(
        r#"fn main() int? {
    let v = "-7".to_int()?
    print(v)
    return none
}"#,
    );
    assert_eq!(out, "-7\n");
}

#[test]
fn string_to_int_whitespace() {
    let out = compile_and_run_stdout(
        r#"fn main() int? {
    let v = "  123  ".to_int()?
    print(v)
    return none
}"#,
    );
    assert_eq!(out, "123\n");
}

#[test]
fn string_to_int_invalid() {
    let out = compile_and_run_stdout(
        r#"fn try_parse() int? {
    let v = "abc".to_int()?
    print("should not reach")
    return v
}

fn main() {
    let result = try_parse()
    print("done")
}"#,
    );
    assert_eq!(out, "done\n");
}

#[test]
fn string_to_int_empty() {
    let out = compile_and_run_stdout(
        r#"fn try_parse() int? {
    let v = "".to_int()?
    print("should not reach")
    return v
}

fn main() {
    let result = try_parse()
    print("done")
}"#,
    );
    assert_eq!(out, "done\n");
}

#[test]
fn string_to_int_mixed_content() {
    let out = compile_and_run_stdout(
        r#"fn try_parse() int? {
    let v = "42abc".to_int()?
    print("should not reach")
    return v
}

fn main() {
    let result = try_parse()
    print("done")
}"#,
    );
    assert_eq!(out, "done\n");
}

#[test]
fn string_to_int_zero() {
    let out = compile_and_run_stdout(
        r#"fn main() int? {
    let v = "0".to_int()?
    print(v)
    return none
}"#,
    );
    assert_eq!(out, "0\n");
}

#[test]
fn string_to_int_variable() {
    let out = compile_and_run_stdout(
        r#"fn main() int? {
    let s = "999"
    let v = s.to_int()?
    print(v)
    return none
}"#,
    );
    assert_eq!(out, "999\n");
}

#[test]
fn string_to_int_bare_call_allowed() {
    let out = compile_and_run_stdout(
        r#"fn main() int? {
    let result = "42".to_int()
    let v = result?
    print(v)
    return none
}"#,
    );
    assert_eq!(out, "42\n");
}

#[test]
fn string_to_int_chained() {
    let out = compile_and_run_stdout(
        r#"fn parse_and_double(s: string) int? {
    let v = s.to_int()?
    return v * 2
}

fn main() {
    let a = parse_and_double("21")
    let b = parse_and_double("bad")
    print(0)
}"#,
    );
    assert_eq!(out, "0\n");
}

#[test]
fn string_to_float_basic() {
    let out = compile_and_run_stdout(
        r#"fn main() float? {
    let v = "3.14".to_float()?
    print(v)
    return none
}"#,
    );
    assert_eq!(out, "3.140000\n");
}

#[test]
fn string_to_float_integer_string() {
    let out = compile_and_run_stdout(
        r#"fn main() float? {
    let v = "42".to_float()?
    print(v)
    return none
}"#,
    );
    assert_eq!(out, "42.000000\n");
}

#[test]
fn string_to_float_negative() {
    let out = compile_and_run_stdout(
        r#"fn main() float? {
    let v = "-2.5".to_float()?
    print(v)
    return none
}"#,
    );
    assert_eq!(out, "-2.500000\n");
}

#[test]
fn string_to_float_invalid() {
    let out = compile_and_run_stdout(
        r#"fn try_parse() float? {
    let v = "not_a_number".to_float()?
    print("should not reach")
    return v
}

fn main() {
    let result = try_parse()
    print("done")
}"#,
    );
    assert_eq!(out, "done\n");
}

#[test]
fn string_to_float_whitespace() {
    let out = compile_and_run_stdout(
        r#"fn main() float? {
    let v = "  1.5  ".to_float()?
    print(v)
    return none
}"#,
    );
    assert_eq!(out, "1.500000\n");
}

#[test]
fn string_to_float_scientific() {
    let out = compile_and_run_stdout(
        r#"fn main() float? {
    let v = "1.5e2".to_float()?
    print(v)
    return none
}"#,
    );
    assert_eq!(out, "150.000000\n");
}

#[test]
fn string_to_float_bare_call_allowed() {
    let out = compile_and_run_stdout(
        r#"fn main() float? {
    let result = "3.14".to_float()
    let v = result?
    print(v)
    return none
}"#,
    );
    assert_eq!(out, "3.140000\n");
}

#[test]
fn string_to_int_pass_nullable_to_function() {
    let out = compile_and_run_stdout(
        r#"fn describe(val: int?) int? {
    let v = val?
    print(f"got {v}")
    return none
}

fn main() {
    describe("42".to_int())
    describe("bad".to_int())
    print("done")
}"#,
    );
    assert_eq!(out, "got 42\ndone\n");
}

#[test]
fn string_to_float_pass_nullable_to_function() {
    let out = compile_and_run_stdout(
        r#"fn describe(val: float?) float? {
    let v = val?
    print(f"got {v}")
    return none
}

fn main() {
    describe("3.14".to_float())
    describe("bad".to_float())
    print("done")
}"#,
    );
    assert_eq!(out, "got 3.140000\ndone\n");
}

// ── String Slice Tests ──────────────────────────────────────────────────────

#[test]
fn string_slice_substring() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let s = "hello world"
    let sub = s.substring(0, 5)
    print(sub)
    print(sub.len())
}
"#);
    assert_eq!(out, "hello\n5\n");
}

#[test]
fn string_slice_trim() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let s = "  hello  "
    print(s.trim())
    print(s.trim_start())
    print(s.trim_end())
}
"#);
    assert_eq!(out, "hello\nhello  \n  hello\n");
}

#[test]
fn string_slice_of_slice() {
    // Nested slicing: substring of a substring should flatten correctly
    let out = compile_and_run_stdout(r#"
fn main() {
    let s = "hello world"
    let sub1 = s.substring(0, 7)
    let sub2 = sub1.substring(2, 5)
    print(sub2)
    print(sub2.len())
}
"#);
    assert_eq!(out, "llo w\n5\n");
}

#[test]
fn string_slice_equality() {
    // Slice should equal an equivalent owned string
    let out = compile_and_run_stdout(r#"
fn main() {
    let s = "hello world"
    let sub = s.substring(0, 5)
    print(sub == "hello")
    print(sub != "world")
}
"#);
    assert_eq!(out, "true\ntrue\n");
}

#[test]
fn string_slice_escape_on_return() {
    // Returned slices should be materialized to owned strings
    let out = compile_and_run_stdout(r#"
fn get_first_word(s: string) string {
    return s.substring(0, 5)
}

fn main() {
    let result = get_first_word("hello world")
    print(result)
    print(result.len())
}
"#);
    assert_eq!(out, "hello\n5\n");
}

#[test]
fn string_slice_escape_in_struct() {
    // Slices stored in struct fields should be materialized
    let out = compile_and_run_stdout(r#"
class Wrapper {
    value: string
}

fn main() {
    let s = "hello world"
    let w = Wrapper { value: s.substring(6, 11) }
    print(w.value)
}
"#);
    assert_eq!(out, "world\n");
}

#[test]
fn string_slice_escape_in_array() {
    // Slices stored in array should be materialized
    let out = compile_and_run_stdout(r#"
fn main() {
    let s = "hello world"
    let arr = [s.substring(0, 5), s.substring(6, 11)]
    print(arr[0])
    print(arr[1])
}
"#);
    assert_eq!(out, "hello\nworld\n");
}

#[test]
fn string_slice_escape_in_closure() {
    // Slices captured by closures should be materialized
    let out = compile_and_run_stdout(r#"
fn main() {
    let s = "hello world"
    let sub = s.substring(0, 5)
    let f = () => { print(sub) }
    f()
}
"#);
    assert_eq!(out, "hello\n");
}

#[test]
fn string_slice_split() {
    // Split should return correct values (now backed by slices)
    let out = compile_and_run_stdout(r#"
fn main() {
    let parts = "a,b,c".split(",")
    print(parts[0])
    print(parts[1])
    print(parts[2])
    print(parts.len())
}
"#);
    assert_eq!(out, "a\nb\nc\n3\n");
}

#[test]
fn string_slice_concat() {
    // Concatenating slices should work correctly
    let out = compile_and_run_stdout(r#"
fn main() {
    let s = "hello world"
    let a = s.substring(0, 5)
    let b = s.substring(5, 11)
    print(a + b)
}
"#);
    assert_eq!(out, "hello world\n");
}

#[test]
fn string_slice_contains() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let s = "hello world"
    let sub = s.substring(0, 5)
    print(sub.contains("ell"))
    print(sub.contains("xyz"))
}
"#);
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn string_slice_replace() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let s = "hello world"
    let sub = s.substring(0, 5)
    print(sub.replace("l", "r"))
}
"#);
    assert_eq!(out, "herro\n");
}

#[test]
fn string_slice_len() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let s = "hello world"
    print(s.substring(0, 5).len())
    print(s.trim().len())
}
"#);
    assert_eq!(out, "5\n11\n");
}

#[test]
fn string_slice_index_of() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let s = "hello world"
    let sub = s.substring(0, 5)
    print(sub.index_of("ll"))
    print(sub.index_of("xyz"))
}
"#);
    assert_eq!(out, "2\n-1\n");
}

#[test]
fn string_slice_split_of_slice() {
    // Split a substring (slice of slice via split)
    let out = compile_and_run_stdout(r#"
fn main() {
    let s = "hello world foo bar"
    let sub = s.substring(6, 19)
    let parts = sub.split(" ")
    print(parts[0])
    print(parts[1])
    print(parts.len())
}
"#);
    assert_eq!(out, "world\nfoo\n3\n");
}

#[test]
fn string_slice_escape_in_map() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let s = "hello"
    let key = s.substring(0, 3)
    let m = Map<string, int> { key: 42 }
    print(m.contains("hel"))
}
"#);
    assert_eq!(out, "true\n");
}

#[test]
fn string_slice_array_push() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let s = "hello world"
    let arr: [string] = []
    arr.push(s.substring(0, 5))
    arr.push(s.substring(6, 11))
    print(arr[0])
    print(arr[1])
}
"#);
    assert_eq!(out, "hello\nworld\n");
}

// ── Escape sequences ──

#[test]
fn string_escape_null() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    let s = "a\0b"
    print(s.len())
}"#,
    );
    assert_eq!(out, "3\n");
}

#[test]
fn string_escape_hex() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    print("\x48\x65\x6C\x6C\x6F")
}"#,
    );
    assert_eq!(out, "Hello\n");
}

#[test]
fn string_escape_hex_ff() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    let s = "\xFF"
    print(s.len())
}"#,
    );
    // \xFF is U+00FF (ÿ), which is 2 bytes in UTF-8
    assert_eq!(out, "2\n");
}

#[test]
fn string_escape_unicode_ascii() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    print("\u{41}")
}"#,
    );
    assert_eq!(out, "A\n");
}

#[test]
fn string_escape_unicode_emoji() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    print("\u{1F680}")
}"#,
    );
    assert_eq!(out, "\u{1F680}\n");
}

#[test]
fn string_escape_unicode_len() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    let s = "\u{1F680}"
    print(s.len())
}"#,
    );
    // Rocket emoji is 4 bytes in UTF-8
    assert_eq!(out, "4\n");
}

#[test]
fn string_escape_mixed() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    print("tab:\there\nnewline")
}"#,
    );
    assert_eq!(out, "tab:\there\nnewline\n");
}

#[test]
fn string_escape_hex_invalid() {
    compile_should_fail_with(
        r#"fn main() {
    let s = "\xGG"
}"#,
        "invalid hex escape",
    );
}

#[test]
fn string_escape_unknown() {
    compile_should_fail_with(
        r#"fn main() {
    let s = "\k"
}"#,
        "unknown escape sequence",
    );
}

#[test]
fn string_escape_unicode_invalid_surrogate() {
    compile_should_fail_with(
        r#"fn main() {
    let s = "\u{D800}"
}"#,
        "surrogate",
    );
}

#[test]
fn string_escape_unicode_unclosed() {
    compile_should_fail_with(
        r#"fn main() {
    let s = "\u{41"
}"#,
        "missing closing",
    );
}
