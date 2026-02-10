mod common;
use common::{compile_and_run_stdout, compile_should_fail, compile_should_fail_with};

// ── Generic Functions ────────────────────────────────────────────

#[test]
fn generic_fn_identity_int() {
    let out = compile_and_run_stdout(
        "fn identity<T>(x: T) T {\n    return x\n}\n\nfn main() {\n    print(identity(42))\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn generic_fn_identity_string() {
    let out = compile_and_run_stdout(
        "fn identity<T>(x: T) T {\n    return x\n}\n\nfn main() {\n    print(identity(\"hello\"))\n}",
    );
    assert_eq!(out, "hello\n");
}

#[test]
fn generic_fn_identity_both() {
    let out = compile_and_run_stdout(
        "fn identity<T>(x: T) T {\n    return x\n}\n\nfn main() {\n    print(identity(42))\n    print(identity(\"hello\"))\n}",
    );
    assert_eq!(out, "42\nhello\n");
}

#[test]
fn generic_fn_two_params() {
    let out = compile_and_run_stdout(
        "fn first<A, B>(a: A, b: B) A {\n    return a\n}\n\nfn main() {\n    print(first(42, \"hello\"))\n}",
    );
    assert_eq!(out, "42\n");
}

// ── Generic Classes ──────────────────────────────────────────────

#[test]
fn generic_class_basic() {
    let out = compile_and_run_stdout(
        "class Box<T> {\n    value: T\n}\n\nfn main() {\n    let b = Box<int> { value: 42 }\n    print(b.value)\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn generic_class_string() {
    let out = compile_and_run_stdout(
        "class Box<T> {\n    value: T\n}\n\nfn main() {\n    let b = Box<string> { value: \"hello\" }\n    print(b.value)\n}",
    );
    assert_eq!(out, "hello\n");
}

#[test]
fn generic_class_two_params() {
    let out = compile_and_run_stdout(
        "class Pair<A, B> {\n    first: A\n    second: B\n}\n\nfn main() {\n    let p = Pair<int, string> { first: 42, second: \"hello\" }\n    print(p.first)\n    print(p.second)\n}",
    );
    assert_eq!(out, "42\nhello\n");
}

#[test]
fn generic_class_method() {
    let out = compile_and_run_stdout(
        "class Box<T> {\n    value: T\n\n    fn get(self) T {\n        return self.value\n    }\n}\n\nfn main() {\n    let b = Box<int> { value: 99 }\n    print(b.get())\n}",
    );
    assert_eq!(out, "99\n");
}

// ── Generic Enums ────────────────────────────────────────────────

#[test]
fn generic_enum_option() {
    let out = compile_and_run_stdout(
        "enum MyOption<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n    let o = MyOption<int>.Some { value: 42 }\n    match o {\n        MyOption.Some { value: v } {\n            print(v)\n        }\n        MyOption.None {\n            print(0)\n        }\n    }\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn generic_enum_option_none() {
    let out = compile_and_run_stdout(
        "enum MyOption<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n    let o = MyOption<int>.None\n    match o {\n        MyOption.Some { value: v } {\n            print(v)\n        }\n        MyOption.None {\n            print(0)\n        }\n    }\n}",
    );
    assert_eq!(out, "0\n");
}

// ── Multiple Instantiations ──────────────────────────────────────

#[test]
fn generic_multiple_instantiations() {
    let out = compile_and_run_stdout(
        "class Box<T> {\n    value: T\n}\n\nfn main() {\n    let a = Box<int> { value: 42 }\n    let b = Box<string> { value: \"hi\" }\n    print(a.value)\n    print(b.value)\n}",
    );
    assert_eq!(out, "42\nhi\n");
}

#[test]
fn generic_fn_with_generic_class() {
    let out = compile_and_run_stdout(
        "class Box<T> {\n    value: T\n}\n\nfn get_value(b: Box<int>) int {\n    return b.value\n}\n\nfn main() {\n    let b = Box<int> { value: 42 }\n    print(get_value(b))\n}",
    );
    assert_eq!(out, "42\n");
}

// ── Additional Generic Tests ─────────────────────────────────────

#[test]
fn generic_nested_box() {
    let out = compile_and_run_stdout(
        "class Box<T> {\n    value: T\n}\n\nfn main() {\n    let inner = Box<int> { value: 99 }\n    let outer = Box<Box<int>> { value: inner }\n    let unwrapped = outer.value\n    print(unwrapped.value)\n}",
    );
    assert_eq!(out, "99\n");
}

#[test]
fn generic_enum_data_variant_match() {
    let out = compile_and_run_stdout(
        "enum Result<T> {\n    Ok { value: T }\n    Err { msg: string }\n}\n\nfn main() {\n    let r = Result<int>.Ok { value: 42 }\n    match r {\n        Result.Ok { value: v } {\n            print(v)\n        }\n        Result.Err { msg: m } {\n            print(m)\n        }\n    }\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn generic_class_method_operates_on_t() {
    let out = compile_and_run_stdout(
        "class Wrapper<T> {\n    value: T\n\n    fn get(self) T {\n        return self.value\n    }\n\n    fn set(mut self, v: T) {\n        self.value = v\n    }\n}\n\nfn main() {\n    let mut w = Wrapper<string> { value: \"hello\" }\n    print(w.get())\n    w.set(\"world\")\n    print(w.get())\n}",
    );
    assert_eq!(out, "hello\nworld\n");
}

#[test]
fn generic_wrong_type_arg_count_rejected() {
    compile_should_fail(
        "class Box<T> {\n    value: T\n}\n\nfn main() {\n    let b = Box<int, string> { value: 42 }\n}",
    );
}

#[test]
fn generic_mangling_no_collision_with_user_class() {
    // Regression: generic id<T>(x: T) T with T=int? mangles to nullable$int,
    // which must not collide with a user class named "nullable_int".
    // With `_` separator both produced `id__nullable_int`; with `$` they're distinct.
    let out = compile_and_run_stdout(
        r#"
class nullable_int {
    v: int
}

fn id<T>(x: T) T {
    return x
}

fn main() {
    let a: int? = 42
    let b = id(a)
    let c = nullable_int { v: 7 }
    let d = id(c)
    print(d.v)
}
"#,
    );
    assert_eq!(out, "7\n");
}

// ── Generic Classes with Trait Impls (Phase A) ─────────────────

#[test]
fn generic_class_impl_trait() {
    let out = compile_and_run_stdout(
        r#"
trait Printable {
    fn show(self) string
}

class Box<T> impl Printable {
    value: T

    fn show(self) string {
        return "box"
    }
}

fn use_printable(p: Printable) string {
    return p.show()
}

fn main() {
    let b = Box<int> { value: 42 }
    print(use_printable(b))
}
"#,
    );
    assert_eq!(out, "box\n");
}

#[test]
fn generic_class_trait_dispatch() {
    let out = compile_and_run_stdout(
        r#"
trait Describable {
    fn describe(self) string
}

class Wrapper<T> impl Describable {
    inner: T

    fn describe(self) string {
        return "wrapper"
    }
}

fn print_description(d: Describable) {
    print(d.describe())
}

fn main() {
    let w1 = Wrapper<int> { inner: 10 }
    let w2 = Wrapper<string> { inner: "hello" }
    print_description(w1)
    print_description(w2)
}
"#,
    );
    assert_eq!(out, "wrapper\nwrapper\n");
}

#[test]
fn generic_class_multiple_traits() {
    let out = compile_and_run_stdout(
        r#"
trait Showable {
    fn show(self) string
}

trait Countable {
    fn count(self) int
}

class Container<T> impl Showable, Countable {
    item: T
    size: int

    fn show(self) string {
        return "container"
    }

    fn count(self) int {
        return self.size
    }
}

fn display(s: Showable) {
    print(s.show())
}

fn get_count(c: Countable) int {
    return c.count()
}

fn main() {
    let c = Container<string> { item: "hello", size: 3 }
    display(c)
    print(get_count(c))
}
"#,
    );
    assert_eq!(out, "container\n3\n");
}

#[test]
fn generic_class_trait_default_method() {
    let out = compile_and_run_stdout(
        r#"
trait Greetable {
    fn name(self) string

    fn greet(self) string {
        return "Hello, " + self.name() + "!"
    }
}

class Holder<T> impl Greetable {
    value: T

    fn name(self) string {
        return "holder"
    }
}

fn main() {
    let h = Holder<int> { value: 42 }
    print(h.greet())
}
"#,
    );
    assert_eq!(out, "Hello, holder!\n");
}

#[test]
fn generic_class_trait_conformance_fail() {
    compile_should_fail_with(
        r#"
trait Showable {
    fn show(self) string
}

class Bad<T> impl Showable {
    value: T

    fn show(self) int {
        return 42
    }
}

fn main() {
    let b = Bad<int> { value: 1 }
}
"#,
        "return type",
    );
}
