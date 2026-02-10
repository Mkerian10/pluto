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
    compile_should_fail("fn main() {\n    let s = \"hello\" + 42\n}");
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
fn string_for_loop_accumulate() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let result = \"\"\n    for c in \"hello\" {\n        result = result + c + \"-\"\n    }\n    print(result)\n}",
    );
    assert_eq!(out, "h-e-l-l-o-\n");
}

#[test]
fn string_for_loop_count() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let count = 0\n    for c in \"hello world\" {\n        count = count + 1\n    }\n    print(count)\n}",
    );
    assert_eq!(out, "11\n");
}

#[test]
fn string_for_loop_empty() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let count = 0\n    for c in \"\" {\n        count = count + 1\n    }\n    print(count)\n}",
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

// ── to_int / to_float (returns Option<int> / Option<float>) ─────────────────

#[test]
fn string_to_int_basic() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    match "42".to_int() {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("fail")
        }
    }
}"#,
    );
    assert_eq!(out, "42\n");
}

#[test]
fn string_to_int_negative() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    match "-7".to_int() {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("fail")
        }
    }
}"#,
    );
    assert_eq!(out, "-7\n");
}

#[test]
fn string_to_int_whitespace() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    match "  123  ".to_int() {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("fail")
        }
    }
}"#,
    );
    assert_eq!(out, "123\n");
}

#[test]
fn string_to_int_invalid() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    match "abc".to_int() {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("none")
        }
    }
}"#,
    );
    assert_eq!(out, "none\n");
}

#[test]
fn string_to_int_empty() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    match "".to_int() {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("none")
        }
    }
}"#,
    );
    assert_eq!(out, "none\n");
}

#[test]
fn string_to_int_mixed_content() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    match "42abc".to_int() {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("none")
        }
    }
}"#,
    );
    assert_eq!(out, "none\n");
}

#[test]
fn string_to_int_zero() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    match "0".to_int() {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("fail")
        }
    }
}"#,
    );
    assert_eq!(out, "0\n");
}

#[test]
fn string_to_int_variable() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    let s = "999"
    match s.to_int() {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("fail")
        }
    }
}"#,
    );
    assert_eq!(out, "999\n");
}

#[test]
fn string_to_int_bare_call_allowed() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    let result = "42".to_int()
    match result {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("fail")
        }
    }
}"#,
    );
    assert_eq!(out, "42\n");
}

#[test]
fn string_to_int_default_helper() {
    let out = compile_and_run_stdout(
        r#"fn unwrap_or(opt: Option<int>, fallback: int) int {
    match opt {
        Option.Some { value } {
            return value
        }
        Option.None {
            return fallback
        }
    }
}

fn main() {
    let a = unwrap_or("42".to_int(), 0)
    let b = unwrap_or("bad".to_int(), -1)
    print(a)
    print(b)
}"#,
    );
    assert_eq!(out, "42\n-1\n");
}

#[test]
fn string_to_float_basic() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    match "3.14".to_float() {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("fail")
        }
    }
}"#,
    );
    assert_eq!(out, "3.140000\n");
}

#[test]
fn string_to_float_integer_string() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    match "42".to_float() {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("fail")
        }
    }
}"#,
    );
    assert_eq!(out, "42.000000\n");
}

#[test]
fn string_to_float_negative() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    match "-2.5".to_float() {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("fail")
        }
    }
}"#,
    );
    assert_eq!(out, "-2.500000\n");
}

#[test]
fn string_to_float_invalid() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    match "not_a_number".to_float() {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("none")
        }
    }
}"#,
    );
    assert_eq!(out, "none\n");
}

#[test]
fn string_to_float_whitespace() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    match "  1.5  ".to_float() {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("fail")
        }
    }
}"#,
    );
    assert_eq!(out, "1.500000\n");
}

#[test]
fn string_to_float_scientific() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    match "1.5e2".to_float() {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("fail")
        }
    }
}"#,
    );
    assert_eq!(out, "150.000000\n");
}

#[test]
fn string_to_float_bare_call_allowed() {
    let out = compile_and_run_stdout(
        r#"fn main() {
    let result = "3.14".to_float()
    match result {
        Option.Some { value } {
            print(value)
        }
        Option.None {
            print("fail")
        }
    }
}"#,
    );
    assert_eq!(out, "3.140000\n");
}

#[test]
fn string_to_int_pass_option_to_function() {
    let out = compile_and_run_stdout(
        r#"fn describe(opt: Option<int>) {
    match opt {
        Option.Some { value } {
            print("got {value}")
        }
        Option.None {
            print("nothing")
        }
    }
}

fn main() {
    describe("42".to_int())
    describe("bad".to_int())
}"#,
    );
    assert_eq!(out, "got 42\nnothing\n");
}

#[test]
fn string_to_float_pass_option_to_function() {
    let out = compile_and_run_stdout(
        r#"fn describe(opt: Option<float>) {
    match opt {
        Option.Some { value } {
            print("got {value}")
        }
        Option.None {
            print("nothing")
        }
    }
}

fn main() {
    describe("3.14".to_float())
    describe("bad".to_float())
}"#,
    );
    assert_eq!(out, "got 3.140000\nnothing\n");
}
