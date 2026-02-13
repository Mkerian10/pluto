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
    let m = Map<string, T> {}
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
    let s = Set<T> {}
    s.insert(val)
    return s
}

fn main() {
    let s = create_set(42)
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
