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
    compile_should_fail_with(
        "class Box<T> {\n    value: T\n}\n\nfn main() {\n    let b = Box<int, string> { value: 42 }\n}",
        "expects 1 type arguments",
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

// ── Phase B: Type Bounds ────────────────────────────────────────

#[test]
fn type_bound_basic() {
    let out = compile_and_run_stdout(r#"
trait Printable {
    fn show(self) string
}

class MyBox impl Printable {
    value: int

    fn show(self) string {
        return "box"
    }
}

fn process<T: Printable>(x: T) string {
    return x.show()
}

fn main() {
    let b = MyBox { value: 42 }
    print(process(b))
}
"#);
    assert_eq!(out.trim(), "box");
}

#[test]
fn type_bound_violation() {
    compile_should_fail_with(r#"
trait Printable {
    fn show(self) string
}

fn process<T: Printable>(x: T) string {
    return x.show()
}

fn main() {
    process(42)
}
"#,
        "does not satisfy bound",
    );
}

#[test]
fn type_bound_multiple() {
    let out = compile_and_run_stdout(r#"
trait Showable {
    fn show(self) string
}

trait Countable {
    fn count(self) int
}

class Item impl Showable, Countable {
    name: string
    n: int

    fn show(self) string {
        return self.name
    }

    fn count(self) int {
        return self.n
    }
}

fn display<T: Showable + Countable>(x: T) string {
    return x.show()
}

fn main() {
    let item = Item { name: "hello", n: 5 }
    print(display(item))
}
"#);
    assert_eq!(out.trim(), "hello");
}

#[test]
fn type_bound_on_class() {
    let out = compile_and_run_stdout(r#"
trait Printable {
    fn show(self) string
}

class Wrapper impl Printable {
    label: string

    fn show(self) string {
        return self.label
    }
}

class Container<T: Printable> {
    item: T
}

fn main() {
    let w = Wrapper { label: "hi" }
    let c = Container<Wrapper> { item: w }
    print(c.item.show())
}
"#);
    assert_eq!(out.trim(), "hi");
}

#[test]
fn type_bound_on_class_violation() {
    compile_should_fail_with(r#"
trait Printable {
    fn show(self) string
}

class Container<T: Printable> {
    item: T
}

fn main() {
    let c = Container<int> { item: 42 }
}
"#,
        "does not satisfy bound",
    );
}

#[test]
fn type_bound_with_trait_impl() {
    let out = compile_and_run_stdout(r#"
trait Printable {
    fn show(self) string
}

trait Describable {
    fn describe(self) string
}

class Inner impl Printable {
    val: int

    fn show(self) string {
        return "inner"
    }
}

class MyBox<T: Printable> impl Describable {
    item: T

    fn get_label(self) string {
        return self.item.show()
    }

    fn describe(self) string {
        return "described"
    }
}

fn use_describable(d: Describable) string {
    return d.describe()
}

fn main() {
    let i = Inner { val: 1 }
    let b = MyBox<Inner> { item: i }
    print(b.get_label())
    print(use_describable(b))
}
"#);
    assert_eq!(out.trim(), "inner\ndescribed");
}

#[test]
fn type_bound_multiple_violation() {
    compile_should_fail_with(r#"
trait Showable {
    fn show(self) string
}

trait Countable {
    fn count(self) int
}

class Item impl Showable {
    name: string

    fn show(self) string {
        return self.name
    }
}

fn display<T: Showable + Countable>(x: T) string {
    return x.show()
}

fn main() {
    let item = Item { name: "hello" }
    display(item)
}
"#,
        "does not satisfy bound",
    );
}

// ============================================================
// Phase C: Explicit type args on function calls
// ============================================================

#[test]
fn explicit_type_args_basic() {
    let out = compile_and_run_stdout(r#"
fn identity<T>(x: T) T {
    return x
}

fn main() {
    let val = identity<int>(42)
    print(val)
}
"#);
    assert_eq!(out.trim(), "42");
}

#[test]
fn explicit_type_args_multi() {
    let out = compile_and_run_stdout(r#"
class Pair<A, B> {
    first: A
    second: B
}

fn make_pair<A, B>(a: A, b: B) Pair<A, B> {
    return Pair<A, B> { first: a, second: b }
}

fn main() {
    let p = make_pair<int, string>(1, "hello")
    print(p.first)
    print(p.second)
}
"#);
    assert_eq!(out.trim(), "1\nhello");
}

#[test]
fn explicit_type_args_no_inference_needed() {
    // Type args are explicit even though they could be inferred
    let out = compile_and_run_stdout(r#"
fn add<T>(x: T, y: T) T {
    return x
}

fn main() {
    let val = add<string>("hello", "world")
    print(val)
}
"#);
    assert_eq!(out.trim(), "hello");
}

#[test]
fn explicit_type_args_wrong_count() {
    compile_should_fail_with(r#"
fn identity<T>(x: T) T {
    return x
}

fn main() {
    let val = identity<int, string>(42)
}
"#,
        "expects 1 type arguments, got 2",
    );
}

#[test]
fn explicit_type_args_non_generic() {
    compile_should_fail_with(r#"
fn add(x: int, y: int) int {
    return x + y
}

fn main() {
    let val = add<int>(1, 2)
}
"#,
        "is not generic and does not accept type arguments",
    );
}

#[test]
fn explicit_type_args_with_bounds() {
    // Combines Phase B (bounds) with Phase C (explicit type args)
    let out = compile_and_run_stdout(r#"
trait Printable {
    fn show(self) string
}

class Wrapper impl Printable {
    label: string

    fn show(self) string {
        return self.label
    }
}

fn display<T: Printable>(x: T) string {
    return x.show()
}

fn main() {
    let w = Wrapper { label: "test" }
    let result = display<Wrapper>(w)
    print(result)
}
"#);
    assert_eq!(out.trim(), "test");
}

#[test]
fn explicit_type_args_bounds_violation() {
    // Explicit type args that violate bounds
    compile_should_fail_with(r#"
trait Printable {
    fn show(self) string
}

fn display<T: Printable>(x: T) string {
    return "nope"
}

fn main() {
    let val = display<int>(42)
}
"#,
        "does not satisfy bound",
    );
}

// ── Generic DI ─────────────────────────────────────────────────────

#[test]
fn generic_di_basic() {
    let out = compile_and_run_stdout(r#"
class Database {
    fn query(self, table: string) string {
        return "result from " + table
    }
}

class Logger<T>[db: Database] {
    fn log(self, msg: string) string {
        return self.db.query(msg)
    }
}

app MyApp[logger: Logger<int>] {
    fn main(self) {
        print(self.logger.log("users"))
    }
}
"#);
    assert_eq!(out.trim(), "result from users");
}

#[test]
fn generic_di_app_bracket_dep() {
    let out = compile_and_run_stdout(r#"
class Database {
    fn name(self) string {
        return "db"
    }
}

class Service<T>[db: Database] {
    fn info(self) string {
        return self.db.name()
    }
}

app MyApp[svc: Service<int>] {
    fn main(self) {
        print(self.svc.info())
    }
}
"#);
    assert_eq!(out.trim(), "db");
}

#[test]
fn generic_di_chain() {
    let out = compile_and_run_stdout(r#"
class Database {
    fn query(self) string {
        return "data"
    }
}

class Repository<T>[db: Database] {
    fn fetch(self) string {
        return self.db.query()
    }
}

class Service<T>[repo: Repository<T>] {
    fn run(self) string {
        return self.repo.fetch()
    }
}

app MyApp[svc: Service<int>] {
    fn main(self) {
        print(self.svc.run())
    }
}
"#);
    assert_eq!(out.trim(), "data");
}

#[test]
fn generic_di_two_instantiations() {
    let out = compile_and_run_stdout(r#"
class Database {
    fn name(self) string {
        return "shared"
    }
}

class Repo<T>[db: Database] {
    fn info(self) string {
        return self.db.name()
    }
}

app MyApp[users: Repo<int>, orders: Repo<string>] {
    fn main(self) {
        print(self.users.info())
        print(self.orders.info())
    }
}
"#);
    assert_eq!(out.trim(), "shared\nshared");
}

#[test]
fn generic_di_struct_literal_blocked() {
    compile_should_fail_with(r#"
class Database {
    fn name(self) string {
        return "db"
    }
}

class Repo<T>[db: Database] {
    label: string
}

fn main() {
    let db = Database {}
    let r = Repo<int> { label: "test" }
}
"#, "cannot manually construct class");
}

#[test]
fn generic_di_lifecycle() {
    let out = compile_and_run_stdout(r#"
class Database {
    fn query(self) string {
        return "ok"
    }
}

scoped class Handler<T>[db: Database] {
    fn handle(self) string {
        return self.db.query()
    }
}

app MyApp[h: Handler<int>] {
    fn main(self) {
        print(self.h.handle())
    }
}
"#);
    assert_eq!(out.trim(), "ok");
}

// ── Bug Fixes (PR 1.1) ───────────────────────────────────────────

#[test]
fn generic_fn_with_map_lit() {
    // Tests that MapLit works inside generic function bodies
    // Bug: resolve_generic_te_in_expr had _ => {} catch-all that skipped MapLit
    let out = compile_and_run_stdout(r#"
fn create_map<T>(default_val: T) Map<string, T> {
    let mut m = Map<string, T> {}
    m["key"] = default_val
    return m
}

fn main() {
    let m = create_map(42)
    print(m["key"])
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn generic_fn_with_set_lit() {
    // Tests that SetLit works inside generic function bodies
    // Bug: resolve_generic_te_in_expr had _ => {} catch-all that skipped SetLit
    let out = compile_and_run_stdout(r#"
fn create_set<T>(val: T) Set<T> {
    let mut s = Set<T> {}
    s.insert(val)
    return s
}

fn main() {
    let mut s = create_set(42)
    print(s.contains(42))
}
"#);
    assert_eq!(out, "true\n");
}

// Note: StaticTraitCall test blocked on reflection intrinsics for generic functions
// The bug fix ensures type_args are visited, but we need reflection to generate
// intrinsics for monomorphized type parameters. This is a separate issue.

// ============================================================
// If-Expression Integration Tests
// ============================================================

#[test]
fn if_expr_with_generic_types() {
    let out = compile_and_run_stdout(
        r#"
        class Box<T> { value: T }
        fn main() {
            let b = if true { Box<int> { value: 10 } } else { Box<int> { value: 20 } }
            print(b.value)
        }
        "#,
    );
    assert_eq!(out.trim(), "10");
}

#[test]
fn generic_function_returning_if_expr() {
    let out = compile_and_run_stdout(
        r#"
        fn choose<T>(a: T, b: T, first: bool) T {
            return if first { a } else { b }
        }
        fn main() {
            print(choose(10, 20, true))
        }
        "#,
    );
    assert_eq!(out.trim(), "10");
}

#[test]
fn if_expr_type_parameter_unification() {
    let out = compile_and_run_stdout(
        r#"
        enum Option<T> {
            Some { value: T }
            None
        }
        fn main() {
            let opt = if true {
                Option<int>.Some { value: 42 }
            } else {
                Option<int>.None
            }
            match opt {
                Option.Some { value: v } { print(v) }
                Option.None { print("none") }
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "42");
}

// ── Generic methods (#293) ───────────────────────────────────────────────

#[test]
fn generic_method_inferred() {
    let out = compile_and_run_stdout(
        r#"
        class C {
            x: int

            fn echo<T>(self, val: T) T {
                return val
            }
        }
        fn main() {
            let c = C { x: 1 }
            print(c.echo(42))
            print(c.echo("hi"))
        }
        "#,
    );
    assert_eq!(out.trim(), "42\nhi");
}

#[test]
fn generic_method_explicit_type_args() {
    let out = compile_and_run_stdout(
        r#"
        class C {
            x: int

            fn echo<T>(self, val: T) T {
                return val
            }
        }
        fn main() {
            let c = C { x: 1 }
            print(c.echo<int>(42))
            print(c.echo<string>("hi"))
        }
        "#,
    );
    assert_eq!(out.trim(), "42\nhi");
}

#[test]
fn generic_method_reads_self() {
    let out = compile_and_run_stdout(
        r#"
        class C {
            x: int

            fn tagged<T>(self, val: T) int {
                return self.x
            }
        }
        fn main() {
            let c = C { x: 7 }
            print(c.tagged(true))
            print(c.tagged<string>("z"))
        }
        "#,
    );
    assert_eq!(out.trim(), "7\n7");
}

#[test]
fn generic_method_mut_self() {
    let out = compile_and_run_stdout(
        r#"
        class Counter {
            n: int

            fn bump<T>(mut self, val: T) {
                self.n = self.n + 1
            }
        }
        fn main() {
            let mut c = Counter { n: 0 }
            c.bump(1)
            c.bump<string>("x")
            print(c.n)
        }
        "#,
    );
    assert_eq!(out.trim(), "2");
}

#[test]
fn generic_method_two_type_params() {
    let out = compile_and_run_stdout(
        r#"
        class P {
            a: int

            fn second<T, U>(self, x: T, y: U) U {
                return y
            }
        }
        fn main() {
            let p = P { a: 1 }
            print(p.second(1, "s"))
            print(p.second<string, int>("s", 9))
        }
        "#,
    );
    assert_eq!(out.trim(), "s\n9");
}

#[test]
fn generic_method_error_handling() {
    let out = compile_and_run_stdout(
        r#"
        error Boom {}
        class C {
            x: int

            fn risky<T>(self, val: T) T {
                if self.x < 0 {
                    raise Boom {}
                }
                return val
            }
        }
        fn main() {
            let bad = C { x: -1 }
            print(bad.risky(5) catch e { -1 })
            let good = C { x: 1 }
            print(good.risky<int>(6) catch e { -1 })
        }
        "#,
    );
    assert_eq!(out.trim(), "-1\n6");
}

#[test]
fn generic_method_comparison_still_parses() {
    // `c.x < d` must stay a comparison, not a generic-call prefix
    let out = compile_and_run_stdout(
        r#"
        class C {
            x: int
        }
        fn main() {
            let c = C { x: 3 }
            let d = 5
            if c.x < d {
                print(1)
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "1");
}

// ── Typestates: `where` state constraints on methods (rfc-typestates.md) ──

/// The full typestate protocol: state as a phantom type param, transitions as
/// methods returning the new state, state-restricted methods, and an
/// unconstrained method available in every state.
#[test]
fn typestate_protocol_round_trip() {
    let out = compile_and_run_stdout(
        r#"
        class Unowned { tag: int }
        class Owned { tag: int }

        class Partition<S> {
            id: int

            fn acquire(self) Partition<Owned> where S == Unowned {
                return Partition<Owned> { id: self.id }
            }

            fn consume(self) int where S == Owned {
                return self.id * 10
            }

            fn release(self) Partition<Unowned> where S == Owned {
                return Partition<Unowned> { id: self.id }
            }

            fn describe(self) string {
                return f"partition {self.id}"
            }
        }

        fn main() {
            let u = Partition<Unowned> { id: 7 }
            print(u.describe())
            let o = u.acquire()
            print(o.consume())
            let back = o.release()
            print(back.describe())
        }
        "#,
    );
    assert_eq!(out.trim(), "partition 7\n70\npartition 7");
}

/// Calling a state-restricted method in the wrong state is a type error that
/// names the constraint — the method does not exist on that instantiation.
#[test]
fn typestate_wrong_state_rejected() {
    compile_should_fail_with(
        r#"
        class Unowned { tag: int }
        class Owned { tag: int }

        class Partition<S> {
            id: int

            fn consume(self) int where S == Owned {
                return self.id
            }
        }

        fn main() {
            let u = Partition<Unowned> { id: 3 }
            print(u.consume())
        }
        "#,
        "method 'consume' does not exist on 'Partition<Unowned>': on 'Partition' it exists only where S == Owned",
    );
}

/// Multiple constraints on one method target different type params; both must
/// hold. Transitions can change one param while constraining another.
#[test]
fn typestate_multi_param_constraints() {
    let out = compile_and_run_stdout(
        r#"
        class Open { tag: int }
        class Closed { tag: int }
        class Leader { tag: int }
        class Follower { tag: int }

        class Conn<S, R> {
            id: int

            fn write(self, data: int) int where S == Open, R == Leader {
                return self.id + data
            }

            fn promote(self) Conn<Open, Leader> where S == Open, R == Follower {
                return Conn<Open, Leader> { id: self.id }
            }
        }

        fn main() {
            let c = Conn<Open, Follower> { id: 100 }
            let l = c.promote()
            print(l.write(5))
        }
        "#,
    );
    assert_eq!(out.trim(), "105");
}

/// Typestates compose with data-carrying type params: the state param gates
/// methods while other params stay ordinary generics.
#[test]
fn typestate_with_data_type_param() {
    let out = compile_and_run_stdout(
        r#"
        class Ready { tag: int }
        class Draft { tag: int }

        class Doc<S, T> {
            payload: T

            fn publish(self) T where S == Ready {
                return self.payload
            }

            fn finalize(self) Doc<Ready, T> where S == Draft {
                return Doc<Ready, T> { payload: self.payload }
            }
        }

        fn main() {
            let d = Doc<Draft, string> { payload: "hello" }
            let r = d.finalize()
            print(r.publish())
        }
        "#,
    );
    assert_eq!(out.trim(), "hello");
}

/// `where` clauses only make sense on generic-class methods.
#[test]
fn typestate_where_rejected_elsewhere() {
    compile_should_fail_with(
        r#"
        class Owned { tag: int }
        fn f(x: int) int where S == Owned {
            return x
        }
        fn main() {
            print(f(1))
        }
        "#,
        "'where' typestate constraints are only allowed on methods of generic classes",
    );
    compile_should_fail_with(
        r#"
        class Owned { tag: int }
        class C<S> {
            id: int
            fn m(self) int where Q == Owned {
                return self.id
            }
        }
        fn main() {
            let c = C<Owned> { id: 1 }
            print(c.m())
        }
        "#,
        "'Q' is not a type parameter of class 'C'",
    );
}

/// Unknown state types in a constraint are rejected at template checking, not
/// silently never-satisfiable.
#[test]
fn typestate_unknown_state_rejected() {
    compile_should_fail_with(
        r#"
        class C<S> {
            id: int
            fn m(self) int where S == Nonexistent {
                return self.id
            }
        }
        fn main() {
            let x = 1
            print(x)
        }
        "#,
        "unknown state type 'Nonexistent'",
    );
}

// ── Typestates phase 2: transition linearity ──

/// Calling a state-transition method consumes the receiver binding: the
/// old-state alias is unusable afterward.
#[test]
fn typestate_use_after_transition_rejected() {
    compile_should_fail_with(
        r#"
        class Unowned { tag: int }
        class Owned { tag: int }

        class Partition<S> {
            id: int
            fn acquire(self) Partition<Owned> where S == Unowned {
                return Partition<Owned> { id: self.id }
            }
            fn describe(self) string {
                return f"p{self.id}"
            }
        }

        fn main() {
            let u = Partition<Unowned> { id: 1 }
            let o = u.acquire()
            print(u.describe())
        }
        "#,
        "'u' was consumed by the transition '.acquire()' (it is now Partition<Owned>)",
    );
}

/// Reassignment revives a consumed binding; consumption inside a branch is
/// conservative (consumed on any path is consumed after the join); loops
/// catch iteration-two use of a binding consumed in iteration one.
#[test]
fn typestate_linearity_flow_rules() {
    // reassignment revives
    let out = compile_and_run_stdout(
        r#"
        class Unowned { tag: int }
        class Owned { tag: int }

        class Partition<S> {
            id: int
            fn acquire(self) Partition<Owned> where S == Unowned {
                return Partition<Owned> { id: self.id }
            }
            fn describe(self) string {
                return f"p{self.id}"
            }
        }

        fn main() {
            let mut u = Partition<Unowned> { id: 1 }
            let o = u.acquire()
            u = Partition<Unowned> { id: 2 }
            print(u.describe())
            print(o.describe())
        }
        "#,
    );
    assert_eq!(out.trim(), "p2\np1");

    // branch join is conservative
    compile_should_fail_with(
        r#"
        class Unowned { tag: int }
        class Owned { tag: int }

        class Partition<S> {
            id: int
            fn acquire(self) Partition<Owned> where S == Unowned {
                return Partition<Owned> { id: self.id }
            }
            fn describe(self) string {
                return f"p{self.id}"
            }
        }

        fn main() {
            let u = Partition<Unowned> { id: 1 }
            if u.id > 0 {
                let o = u.acquire()
                print(o.describe())
            }
            print(u.describe())
        }
        "#,
        "'u' was consumed by the transition",
    );

    // loop: consumed in iteration one, used in iteration two
    compile_should_fail_with(
        r#"
        class Unowned { tag: int }
        class Owned { tag: int }

        class Partition<S> {
            id: int
            fn acquire(self) Partition<Owned> where S == Unowned {
                return Partition<Owned> { id: self.id }
            }
        }

        fn main() {
            let u = Partition<Unowned> { id: 1 }
            let mut i = 0
            while i < 3 {
                let o = u.acquire()
                i = i + 1
            }
            print(i)
        }
        "#,
        "'u' was consumed by the transition",
    );
}

/// Only STATE parameters trigger consumption. A class with no `where`
/// clauses is a plain data generic: instance-returning transform methods
/// never consume the receiver.
#[test]
fn typestate_linearity_ignores_data_generics() {
    let out = compile_and_run_stdout(
        r#"
        class Box<T> {
            value: T

            fn as_label(self) Box<string> {
                return Box<string> { value: "x" }
            }

            fn get(self) T {
                return self.value
            }
        }

        fn main() {
            let b = Box<int> { value: 5 }
            let mut s = b.as_label()
            print(b.get())
            print(s.get())
        }
        "#,
    );
    assert_eq!(out.trim(), "5\nx");
}

/// In a mixed class the discrimination is per-parameter: changing the state
/// param consumes; a non-transition method on the same class does not.
#[test]
fn typestate_linearity_state_vs_data_params() {
    compile_should_fail_with(
        r#"
        class Ready { tag: int }
        class Draft { tag: int }

        class Doc<S, T> {
            payload: T

            fn finalize(self) Doc<Ready, T> where S == Draft {
                return Doc<Ready, T> { payload: self.payload }
            }

            fn peek(self) T {
                return self.payload
            }
        }

        fn main() {
            let d = Doc<Draft, string> { payload: "hello" }
            let r = d.finalize()
            print(d.peek())
        }
        "#,
        "'d' was consumed by the transition '.finalize()'",
    );
}

// ── Typestates phase 3: must-release linearity (rfc-typestates.md) ──

/// Shared lease protocol for the must-release tests: `Held` is marked
/// must_release; `Idle` and `Revoked` are droppable.
fn lease_src(body: &str) -> String {
    format!(
        r#"
        class Idle {{ tag: int }}
        class Held {{ tag: int }}
        class Revoked {{ tag: int }}

        class Lease<S> {{
            id: int

            must_release Held

            fn acquire(self) Lease<Held> where S == Idle {{
                return Lease<Held> {{ id: self.id }}
            }}

            fn release(self) Lease<Idle> where S == Held {{
                return Lease<Idle> {{ id: self.id }}
            }}

            fn describe(self) string {{
                return f"lease {{self.id}}"
            }}
        }}

        {body}
        "#
    )
}

/// The full lifecycle discharges the obligation: acquire, use, release.
#[test]
fn must_release_full_lifecycle() {
    let out = compile_and_run_stdout(&lease_src(
        r#"
        fn main() {
            let l = Lease<Idle> { id: 7 }
            let h = l.acquire()
            print(h.describe())
            let done = h.release()
            print(done.describe())
        }
        "#,
    ));
    assert_eq!(out.trim(), "lease 7\nlease 7");
}

/// Dropping a binding in a must-release state is a compile error naming the
/// state and suggesting the transition out.
#[test]
fn must_release_dropped_binding_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                let l = Lease<Idle> { id: 7 }
                let h = l.acquire()
                print(h.describe())
            }
            "#,
        ),
        "'h' still holds Lease<Held>, a must_release state, when it goes out of scope; transition it out of 'Held' (e.g. .release())",
    );
}

/// Passing a must-release binding as an argument moves it: the caller is
/// clean, the callee owns (and here discharges) the single obligation.
#[test]
fn must_release_move_transfers_obligation() {
    let out = compile_and_run_stdout(&lease_src(
        r#"
        fn finish(h: Lease<Held>) int {
            let done = h.release()
            return done.id
        }

        fn main() {
            let l = Lease<Idle> { id: 3 }
            let h = l.acquire()
            print(finish(h))
        }
        "#,
    ));
    assert_eq!(out.trim(), "3");
}

/// The moved-to callee must discharge: returning while the parameter still
/// holds the state is a leak in the callee.
#[test]
fn must_release_callee_must_discharge() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn peek(h: Lease<Held>) int {
                return h.id
            }

            fn main() {
                let l = Lease<Idle> { id: 3 }
                let h = l.acquire()
                print(peek(h))
                let done = h.release()
                print(done.id)
            }
            "#,
        ),
        "cannot return while 'h' still holds Lease<Held>",
    );
}

/// Returning the binding moves the obligation to the caller.
#[test]
fn must_release_return_discharges() {
    let out = compile_and_run_stdout(&lease_src(
        r#"
        fn make() Lease<Held> {
            let l = Lease<Idle> { id: 9 }
            return l.acquire()
        }

        fn main() {
            let h = make()
            let done = h.release()
            print(done.id)
        }
        "#,
    ));
    assert_eq!(out.trim(), "9");
}

/// A second use after a move is a use-after-consume naming the move.
#[test]
fn must_release_double_move_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn finish(h: Lease<Held>) int {
                let done = h.release()
                return done.id
            }

            fn main() {
                let l = Lease<Idle> { id: 3 }
                let h = l.acquire()
                print(finish(h))
                print(finish(h))
            }
            "#,
        ),
        "'h' was moved (passed to 'finish()'); a must_release value has a single owner",
    );
}

/// Capturing a must-release binding in a closure would duplicate the
/// obligation — rejected.
#[test]
fn must_release_closure_capture_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                let l = Lease<Idle> { id: 1 }
                let h = l.acquire()
                let f = () => h.describe()
                let done = h.release()
                print(done.id)
            }
            "#,
        ),
        "'h' holds Lease<Held>, a must_release state, and cannot be captured by a closure or spawned task",
    );
}

/// Spawn desugars to a capturing closure: same duplication, same rejection.
#[test]
fn must_release_spawn_capture_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn finish(h: Lease<Held>) int {
                let done = h.release()
                return done.id
            }

            fn main() {
                let l = Lease<Idle> { id: 1 }
                let h = l.acquire()
                let t = spawn finish(h)
                print(t.get())
            }
            "#,
        ),
        "cannot be captured by a closure or spawned task",
    );
}

/// Storing a must-release value into a field would let the obligation escape
/// the analysis — rejected.
#[test]
fn must_release_field_store_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            class Holder {
                kept: Lease<Held>
            }

            fn main() {
                let l = Lease<Idle> { id: 1 }
                let h = l.acquire()
                let holder = Holder { kept: h }
                print(1)
            }
            "#,
        ),
        "a value in the must_release state Lease<Held> may not be stored in a field",
    );
}

/// Container literals are equally untrackable.
#[test]
fn must_release_container_store_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                let l = Lease<Idle> { id: 1 }
                let h = l.acquire()
                let xs = [h]
                print(1)
            }
            "#,
        ),
        "may not be stored in a container literal",
    );
}

/// Producing a must-release value at statement position drops it on the
/// floor — rejected.
#[test]
fn must_release_statement_drop_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                let l = Lease<Idle> { id: 1 }
                l.acquire()
                print(1)
            }
            "#,
        ),
        "this expression produces Lease<Held>, a must_release state, and immediately drops it",
    );
}

/// Branch joins are conservative: releasing on only one path leaves the
/// obligation live after the join.
#[test]
fn must_release_branch_join_conservative() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                let l = Lease<Idle> { id: 1 }
                let h = l.acquire()
                if h.id > 0 {
                    let done = h.release()
                    print(done.id)
                } else {
                    print(0)
                }
            }
            "#,
        ),
        "'h' still holds Lease<Held>",
    );
}

/// Releasing on every path discharges the obligation at the join.
#[test]
fn must_release_all_paths_discharge() {
    let out = compile_and_run_stdout(&lease_src(
        r#"
        fn main() {
            let l = Lease<Idle> { id: 1 }
            let h = l.acquire()
            if h.id > 0 {
                let done = h.release()
                print(done.id)
            } else {
                let dropped = h.release()
                print(dropped.id)
            }
        }
        "#,
    ));
    assert_eq!(out.trim(), "1");
}

/// A loop body is a scope: acquiring in an iteration without releasing
/// leaks at the body's end.
#[test]
fn must_release_loop_leak_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                for i in 0..3 {
                    let src = Lease<Idle> { id: i }
                    let h = src.acquire()
                    print(h.id)
                }
            }
            "#,
        ),
        "'h' still holds Lease<Held>",
    );
}

/// `break` exits the loop body's scope: bindings acquired since the body
/// began must be discharged first.
#[test]
fn must_release_break_with_live_obligation_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                while true {
                    let src = Lease<Idle> { id: 1 }
                    let h = src.acquire()
                    break
                }
                print(1)
            }
            "#,
        ),
        "cannot break while 'h' still holds Lease<Held>",
    );
}

/// `raise` is a definite exit: leaving while holding is a leak (the error
/// does not carry the value).
#[test]
fn must_release_raise_with_live_obligation_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            error Boom { code: int }

            fn run() int {
                let l = Lease<Idle> { id: 1 }
                let h = l.acquire()
                raise Boom { code: 1 }
            }

            fn main() {
                print(run() catch 0)
            }
            "#,
        ),
        "cannot raise 'Boom' while 'h' still holds Lease<Held>",
    );
}

/// Moving the binding into a raised error's payload discharges it — the
/// obligation rides in the error.
#[test]
fn must_release_raise_payload_discharges() {
    let out = compile_and_run_stdout(&lease_src(
        r#"
        error Interrupted { lease: Lease<Held> }

        fn run(flag: int) int {
            let l = Lease<Idle> { id: 5 }
            let h = l.acquire()
            if flag > 0 {
                raise Interrupted { lease: h }
            }
            let done = h.release()
            return done.id
        }

        fn main() {
            let n = run(1) catch e: Interrupted {
                let back = e.lease
                let done = back.release()
                done.id * 100
            }
            print(n)
        }
        "#,
    ));
    assert_eq!(out.trim(), "500");
}

/// The general form names the state parameter on a multi-param class and is
/// enforced the same way.
#[test]
fn must_release_general_form_multi_param() {
    compile_should_fail_with(
        r#"
        class Open { tag: int }
        class Closed { tag: int }
        class Leader { tag: int }
        class Follower { tag: int }

        class Conn<S, R> {
            id: int

            must_release S == Open

            fn close(self) Conn<Closed, R> where S == Open {
                return Conn<Closed, R> { id: self.id }
            }

            fn promote(self) Conn<S, Leader> where R == Follower {
                return Conn<S, Leader> { id: self.id }
            }
        }

        fn main() {
            let c = Conn<Open, Follower> { id: 1 }
            print(c.id)
        }
        "#,
        "'c' still holds Conn<Open, Follower>, a must_release state",
    );
}

/// The simple form is only legal with exactly one state parameter.
#[test]
fn must_release_simple_form_needs_single_state_param() {
    compile_should_fail_with(
        r#"
        class Open { tag: int }
        class Closed { tag: int }
        class A { t: int }
        class B { t: int }

        class Conn<S, R> {
            id: int

            must_release Open

            fn close(self) Conn<Closed, R> where S == Open {
                return Conn<Closed, R> { id: self.id }
            }

            fn flip(self) Conn<S, B> where R == A {
                return Conn<S, B> { id: self.id }
            }
        }

        fn main() {
            print(1)
        }
        "#,
        "'must_release Open': class 'Conn' has 2 state parameters (S, R); name one with the general form `must_release <Param> == Open`",
    );
}

/// Typos in the state name are caught at the declaration, with the known
/// states listed.
#[test]
fn must_release_unknown_state_rejected() {
    compile_should_fail_with(
        r#"
        class Idle { tag: int }
        class Held { tag: int }

        class Lease<S> {
            id: int

            must_release Helb

            fn acquire(self) Lease<Held> where S == Idle {
                return Lease<Held> { id: self.id }
            }
        }

        fn main() {
            print(1)
        }
        "#,
        "'Helb' is not a state of 'Lease' for parameter 'S'",
    );
}

/// must_release needs a typestate class: no type params, no states.
#[test]
fn must_release_requires_typestate_class() {
    compile_should_fail_with(
        r#"
        class Thing {
            id: int

            must_release Held
        }

        fn main() {
            print(1)
        }
        "#,
        "'must_release' requires a typestate class",
    );
}

/// Non-must-release states keep the relaxed rules: droppable states can go
/// out of scope freely, and classes without `where` clauses are untouched.
#[test]
fn must_release_droppable_states_stay_relaxed() {
    let out = compile_and_run_stdout(&lease_src(
        r#"
        fn main() {
            let a = Lease<Idle> { id: 1 }
            let b = Lease<Revoked> { id: 2 }
            print(a.id + b.id)
        }
        "#,
    ));
    assert_eq!(out.trim(), "3");
}

// ── Contracts on generic classes interact with monomorphization ─────────────

#[test]
fn generic_contracts_not_reproven_per_instantiation() {
    // The proof runs once on the template; multiple instantiations (plus the
    // transitive one from the generic function) compile and run — the
    // monomorphized copies are not re-proven, and every construction site of
    // every instantiation still carries its obligation.
    let out = compile_and_run_stdout(
        r#"
class Gauge<T> {
    tag: T?
    v: int

    invariant self.v >= 0

    fn add(mut self, d: int)
        requires d >= 0
        ensures self.v == old(self.v) + d {
        self.v = self.v + d
    }
}

fn fresh<T>() Gauge<T> {
    return Gauge<T> { tag: none, v: 0 }
}

fn main() {
    let mut a = fresh<int>()
    let mut b = fresh<string>()
    let mut c = Gauge<float> { tag: none, v: 7 }
    a.add(1)
    b.add(2)
    c.add(3)
    print(a.v + b.v + c.v)
}
"#,
    );
    assert_eq!(out.trim(), "13");
}

#[test]
fn generic_template_contract_checked_without_instantiation() {
    // Skolem template checking carries the obligations even when no code
    // ever instantiates the class.
    compile_should_fail_with(
        r#"
class Gauge<T> {
    tag: T?
    v: int

    invariant self.v >= 0

    fn drain(mut self) {
        self.v = self.v - 10
    }
}

fn main() {
    print(1)
}
"#,
        "cannot prove invariant 'self.v >= 0' of class 'Gauge<T>'",
    );
}

// ── Must-release default-deny: moves discharge only into obligation-carrying
// positions (issues #404–#412, #420). Every laundering boundary rejects,
// naming the boundary. ──

/// #404 repro 1: a generic `fn sink<T>(x: T)` cannot swallow a must-release
/// value — the skolem-checked template body never re-imposes the obligation.
#[test]
fn must_release_generic_param_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn sink<T>(x: T) {
                print("sunk")
            }

            fn main() {
                let l = Lease<Idle> { id: 1 }
                let h = l.acquire()
                sink(h)
            }
            "#,
        ),
        "may not be moved into the generic type parameter 'T'",
    );
}

/// #404 repro 2: the double-release driver — passing the value at T to a
/// higher-order generic — is cut off at the same boundary.
#[test]
fn must_release_generic_twice_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn twice<T>(g: fn(T), x: T) {
                g(x)
                g(x)
            }

            fn rel(h: Lease<Held>) {
                let done = h.release()
            }

            fn main() {
                let l = Lease<Idle> { id: 7 }
                let h = l.acquire()
                twice(rel, h)
            }
            "#,
        ),
        "may not be moved into the generic type parameter 'T'",
    );
}

/// #405: returning a must-release value at trait type discharges nothing —
/// the trait handle would carry no obligation.
#[test]
fn must_release_trait_upcast_return_rejected() {
    compile_should_fail_with(
        r#"
        class Unlocked { tag: int }
        class Locked { tag: int }

        trait Peeker {
            fn peek(self) int
        }

        class Lock<S> impl Peeker {
            id: int

            must_release Locked

            fn acquire(self) Lock<Locked> where S == Unlocked {
                return Lock<Locked> { id: self.id }
            }

            fn release(self) Lock<Unlocked> where S == Locked {
                return Lock<Unlocked> { id: self.id }
            }

            fn peek(self) int {
                return self.id
            }
        }

        fn launder(l: Lock<Locked>) Peeker {
            return l
        }

        fn main() {
            let u = Lock<Unlocked> { id: 3 }
            let l = u.acquire()
            let t = launder(l)
            print(t.peek())
        }
        "#,
        "may not be moved into the trait type 'Peeker' — upcasting erases the typestate",
    );
}

/// #406 route 1: a nullable-typed parameter swallows the obligation.
#[test]
fn must_release_nullable_param_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn swallow(x: Lease<Held>?) {
                print("swallowed")
            }

            fn main() {
                let l = Lease<Idle> { id: 14 }
                let h = l.acquire()
                swallow(h)
            }
            "#,
        ),
        "may not be moved into a nullable type — a release obligation does not survive nullable wrapping",
    );
}

/// #406 route 2: a nullable return type launders toward the caller.
#[test]
fn must_release_nullable_return_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn make_opt() Lease<Held>? {
                let l = Lease<Idle> { id: 5 }
                let h = l.acquire()
                return h
            }

            fn main() {
                let f = make_opt()
                print(1)
            }
            "#,
        ),
        "may not be moved into a nullable type",
    );
}

/// #406 route 3: binding a must-release value at a nullable annotation (the
/// `??`-aliasing driver) is rejected at the binding.
#[test]
fn must_release_nullable_binding_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                let l: Lease<Held>? = Lease<Idle> { id: 6 }.acquire()
                print(1)
            }
            "#,
        ),
        "may not be moved into a nullable binding",
    );
}

/// #407 repro 1: `.push()` is not a carrying destination — the stored
/// element would come back untracked via indexing.
#[test]
fn must_release_container_push_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                let mut arr: [Lease<Held>] = []
                let l = Lease<Idle> { id: 9 }
                let h = l.acquire()
                arr.push(h)
            }
            "#,
        ),
        "may not be moved into the builtin method '.push()'",
    );
}

/// #407 repro 2: a channel send launders the obligation into the buffer.
#[test]
fn must_release_channel_send_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                let (tx, rx) = chan<Lease<Held>>(2)
                let l = Lease<Idle> { id: 18 }
                let h = l.acquire()
                tx.send(h)!
                print(1)
            }
            "#,
        ),
        "a release obligation cannot be sent through a channel",
    );
}

/// #408: an if-expression yielding the same binding from both arms is a
/// consume-once — the original binding is moved and unusable.
#[test]
fn must_release_if_expression_yield_consumes() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                let l = Lease<Idle> { id: 8 }
                let h = l.acquire()
                let g = if true { h } else { h }
                let x = g.release()
                let y = h.release()
            }
            "#,
        ),
        "'h' was moved (yielded from an if-expression)",
    );
}

/// #408: the consume-once form is legal — the result carries the obligation
/// and a single release discharges it.
#[test]
fn must_release_if_expression_consume_once_ok() {
    let out = compile_and_run_stdout(&lease_src(
        r#"
        fn main() {
            let l = Lease<Idle> { id: 8 }
            let h = l.acquire()
            let g = if h.id > 0 { h } else { h }
            let x = g.release()
            print(x.id)
        }
        "#,
    ));
    assert_eq!(out.trim(), "8");
}

/// #408: arms that disagree on consumption are rejected outright.
#[test]
fn must_release_if_expression_arm_disagreement_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                let a = Lease<Idle> { id: 1 }
                let h = a.acquire()
                let b = Lease<Idle> { id: 2 }
                let other = b.acquire()
                let g = if h.id > 0 { h } else { other }
                let x = g.release()
                let y = other.release()
                let z = h.release()
            }
            "#,
        ),
        "the arms of this if-expression disagree about",
    );
}

/// #408: the match-expression form is the same door.
#[test]
fn must_release_match_expression_yield_consumes() {
    compile_should_fail_with(
        &lease_src(
            r#"
            enum Pick { A B }

            fn main() {
                let l = Lease<Idle> { id: 12 }
                let h = l.acquire()
                let g = match Pick.A {
                    Pick.A => h,
                    Pick.B => h
                }
                let x = g.release()
                let y = h.release()
            }
            "#,
        ),
        "'h' was moved (yielded from a match expression)",
    );
}

/// #409: stashing a must-release binding in an enum variant payload is a
/// store the analysis cannot track — rejected like a field store.
#[test]
fn must_release_enum_payload_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            enum Stash {
                Hold { kept: Lease<Held> }
                Empty
            }

            fn main() {
                let l = Lease<Idle> { id: 4 }
                let h = l.acquire()
                let mut s = Stash.Hold { kept: h }
                let x = h.release()
            }
            "#,
        ),
        "may not be stored in an enum variant payload",
    );
}

/// #410: aliasing a caught error binding whose payload is must-release would
/// let the alias double-extract the payload.
#[test]
fn must_release_error_binding_alias_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            error Oops { lease: Lease<Held> }

            fn risky() {
                let u = Lease<Idle> { id: 2 }
                raise Oops { lease: u.acquire() }
            }

            fn main() {
                risky() catch e: Oops {
                    let e2 = e
                    let a = e.lease
                    let b = e2.lease
                    let x = a.release()
                    let y = b.release()
                }
            }
            "#,
        ),
        "cannot copy the caught error binding 'e'",
    );
}

/// Calling a method directly on an un-extracted error payload would bypass
/// the extraction tracking — the payload must be bound first.
#[test]
fn must_release_error_payload_direct_call_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            error Oops { lease: Lease<Held> }

            fn risky() {
                let u = Lease<Idle> { id: 2 }
                raise Oops { lease: u.acquire() }
            }

            fn main() {
                risky() catch e: Oops {
                    let x = e.lease.release()
                    let a = e.lease
                    let y = a.release()
                }
            }
            "#,
        ),
        "cannot call '.release()' directly on the error payload 'e.lease'",
    );
}

/// #411: a closure parameter typed at a must-release state carries the
/// obligation into the body — dropping it there is the usual leak error.
#[test]
fn must_release_closure_param_carries_obligation() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                let cl = (h: Lease<Held>) => {
                    print(h.id)
                }
                print(1)
            }
            "#,
        ),
        "'h' still holds Lease<Held>, a must_release state, when it goes out of scope",
    );
}

/// #411: a call through a function value is not a carrying destination — the
/// analysis cannot see through fn-typed values.
#[test]
fn must_release_fn_value_call_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                let cl = (h: Lease<Held>) => {
                    let done = h.release()
                }
                let l = Lease<Idle> { id: 11 }
                let h = l.acquire()
                cl(h)
            }
            "#,
        ),
        "not a named function",
    );
}

/// #412: a nested generic DATA argument before the state position must not
/// corrupt state parsing — the obligation stays attached and the leak is
/// caught (structural instance args, not mangled-string re-parsing).
#[test]
fn must_release_nested_generic_data_arg_still_enforced() {
    compile_should_fail_with(
        r#"
        class Box<T> { v: T }
        class Free { tag: int }
        class Held { tag: int }

        class R<T, S> {
            v: T
            must_release S == Held
            fn grab(self) R<T, Held> where S == Free {
                return R<T, Held> { v: self.v }
            }
            fn drop_it(self) R<T, Free> where S == Held {
                return R<T, Free> { v: self.v }
            }
        }

        fn main() {
            let r = R<Box<int>, Free> { v: Box<int> { v: 1 } }
            let h = r.grab()
            print(1)
        }
        "#,
        "'h' still holds R<Box<int>, Held>, a must_release state",
    );
}

/// #414: the leak hint only suggests transitions that exist on the actual
/// instantiation — here no transition out of 'A' exists on M2<A, Q>, so the
/// hint names the state without a bogus method suggestion.
#[test]
fn must_release_leak_hint_respects_instantiation() {
    compile_should_fail_with(
        r#"
        class A { tag: int }
        class B { tag: int }
        class P { tag: int }
        class Q { tag: int }

        class M2<X, Y> {
            id: int
            must_release X == A

            fn go(self) M2<B, Q> where X == A, Y == P { return M2<B, Q> { id: self.id } }
            fn slide(self) M2<X, Q> where Y == P { return M2<X, Q> { id: self.id } }
            fn back(self) M2<A, P> where X == B { return M2<A, P> { id: self.id } }
        }

        fn main() {
            let m = M2<A, P> { id: 16 }
            let mut s = m.slide()
        }
        "#,
        "transition it out of 'A', move it onward, or return it",
    );
}

/// #420: a `where`-constrained method whose body transitions `self` without
/// declaring the transition in its signature is rejected at the declaration —
/// otherwise the checker would force callers into a double release.
#[test]
fn must_release_hidden_self_transition_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                print(1)
            }
            "#,
        )
        .replace(
            "fn describe(self) string {",
            r#"fn sneaky(self) int where S == Held {
                let a = self.release()
                return 0
            }

            fn describe(self) string {"#,
        ),
        "method 'sneaky' calls the transition '.release()' on 'self' but its signature does not declare a transition",
    );
}

/// #420 companion: declaring the transition honestly (same body, transition
/// signature) compiles and consumes the caller's binding exactly once.
#[test]
fn must_release_declared_self_transition_ok() {
    let out = compile_and_run_stdout(&lease_src(
        r#"
        fn main() {
            let l = Lease<Idle> { id: 23 }
            let h = l.acquire()
            let done = h.release()
            print(done.id)
        }
        "#,
    ));
    assert_eq!(out.trim(), "23");
}

/// Default-deny on temporaries: a non-transition method on an unbound
/// must-release temporary would drop the value after the call.
#[test]
fn must_release_unbound_temporary_receiver_rejected() {
    compile_should_fail_with(
        &lease_src(
            r#"
            fn main() {
                let n = Lease<Idle> { id: 3 }.acquire().describe()
                print(n)
            }
            "#,
        ),
        "is an unbound value in the must_release state",
    );
}

/// Chained transitions on temporaries stay legal: each call consumes the
/// previous temporary and the final droppable state needs no release.
#[test]
fn must_release_chained_transition_temporaries_ok() {
    let out = compile_and_run_stdout(&lease_src(
        r#"
        fn main() {
            let idle = Lease<Idle> { id: 5 }.acquire().release()
            print(idle.id)
        }
        "#,
    ));
    assert_eq!(out.trim(), "5");
}
