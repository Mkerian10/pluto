mod common;
use common::{compile_and_run_stdout, compile_should_fail, compile_should_fail_with};

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
        "class Counter {\n    val: int\n\n    fn get(self) int {\n        return self.val\n    }\n}\n\nfn main() {\n    let mut c = Counter { val: 0 }\n    c.val = 42\n    print(c.get())\n}",
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

#[test]
fn class_multiple_methods() {
    let out = compile_and_run_stdout(
        "class Rect {\n    w: int\n    h: int\n\n    fn area(self) int {\n        return self.w * self.h\n    }\n\n    fn perimeter(self) int {\n        return 2 * (self.w + self.h)\n    }\n}\n\nfn main() {\n    let r = Rect { w: 3, h: 4 }\n    print(r.area())\n    print(r.perimeter())\n}",
    );
    assert_eq!(out, "12\n14\n");
}

#[test]
fn class_duplicate_field_rejected() {
    compile_should_fail_with(
        "class Bad {\n    x: int\n    x: int\n}\n\nfn main() {\n    let b = Bad { x: 1 }\n}",
        "duplicate field 'x'",
    );
}

#[test]
fn class_methods_only() {
    let out = compile_and_run_stdout(
        "class Greeter {\n    name: string\n\n    fn greet(self) string {\n        return \"hello \" + self.name\n    }\n\n    fn shout(self) string {\n        return \"HEY \" + self.name\n    }\n}\n\nfn main() {\n    let g = Greeter { name: \"world\" }\n    print(g.greet())\n    print(g.shout())\n}",
    );
    assert_eq!(out, "hello world\nHEY world\n");
}

#[test]
fn class_unknown_field_type_rejected() {
    compile_should_fail(
        "class Bad {\n    x: NonExistent\n}\n\nfn main() {\n    let b = Bad { x: 1 }\n}",
    );
}

#[test]
fn class_duplicate_method_rejected() {
    compile_should_fail_with(
        "class Bad {\n    x: int\n\n    fn foo(self) int {\n        return 1\n    }\n\n    fn foo(self) int {\n        return 2\n    }\n}\n\nfn main() {\n    let b = Bad { x: 1 }\n}",
        "duplicate method 'foo'",
    );
}

// ── mut self enforcement ─────────────────────────────────────

#[test]
fn mut_self_field_assign_ok() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    val: int

    fn inc(mut self) {
        self.val = self.val + 1
    }

    fn get(self) int {
        return self.val
    }
}

fn main() {
    let mut c = Counter { val: 0 }
    c.inc()
    c.inc()
    print(c.get())
}
"#,
    );
    assert_eq!(out, "2\n");
}

#[test]
fn non_mut_self_field_assign_rejected() {
    compile_should_fail_with(
        r#"
class Counter {
    val: int

    fn inc(self) {
        self.val = self.val + 1
    }
}

fn main() {
    let c = Counter { val: 0 }
    c.inc()
}
"#,
        "cannot assign to 'self.val' in a non-mut method",
    );
}

#[test]
fn non_mut_calling_mut_method_rejected() {
    compile_should_fail_with(
        r#"
class Counter {
    val: int

    fn inc(mut self) {
        self.val = self.val + 1
    }

    fn wrapper(self) {
        self.inc()
    }
}

fn main() {
    let c = Counter { val: 0 }
    c.wrapper()
}
"#,
        "cannot call 'mut self' method 'inc' on self in a non-mut method",
    );
}

#[test]
fn trait_mut_self_mismatch_rejected() {
    compile_should_fail_with(
        r#"
trait Incrementable {
    fn inc(mut self)
}

class Counter impl Incrementable {
    val: int

    fn inc(self) {
        let x = self.val
    }
}

fn main() {
}
"#,
        "declares 'mut self', but class 'Counter' does not",
    );
}

#[test]
fn trait_mut_self_reverse_mismatch_rejected() {
    compile_should_fail_with(
        r#"
trait Readable {
    fn read(self) int
}

class Counter impl Readable {
    val: int

    fn read(mut self) int {
        return self.val
    }
}

fn main() {
}
"#,
        "declares 'self', but class 'Counter' declares 'mut self'",
    );
}

#[test]
fn non_mut_self_read_only_ok() {
    let out = compile_and_run_stdout(
        r#"
class Point {
    x: int
    y: int

    fn sum(self) int {
        return self.x + self.y
    }
}

fn main() {
    let p = Point { x: 3, y: 4 }
    print(p.sum())
}
"#,
    );
    assert_eq!(out, "7\n");
}

// ── let mut enforcement tests ──────────────────────────────────────────────

#[test]
fn let_mut_field_assign_ok() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    val: int
}

fn main() {
    let mut c = Counter { val: 0 }
    c.val = 42
    print(c.val)
}
"#,
    );
    assert_eq!(out, "42\n");
}

#[test]
fn let_immutable_field_assign_rejected() {
    compile_should_fail_with(
        r#"
class Counter {
    val: int
}

fn main() {
    let c = Counter { val: 0 }
    c.val = 42
}
"#,
        "cannot assign to field of immutable variable 'c'",
    );
}

#[test]
fn let_immutable_mut_method_rejected() {
    compile_should_fail_with(
        r#"
class Counter {
    val: int

    fn inc(mut self) {
        self.val = self.val + 1
    }
}

fn main() {
    let c = Counter { val: 0 }
    c.inc()
}
"#,
        "cannot call mutating method 'inc' on immutable variable 'c'",
    );
}

#[test]
fn let_mut_mut_method_ok() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    val: int

    fn inc(mut self) {
        self.val = self.val + 1
    }

    fn get(self) int {
        return self.val
    }
}

fn main() {
    let mut c = Counter { val: 0 }
    c.inc()
    c.inc()
    print(c.get())
}
"#,
    );
    assert_eq!(out, "2\n");
}

#[test]
fn let_immutable_read_only_ok() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    val: int
}

fn main() {
    let c = Counter { val: 42 }
    print(c.val)
}
"#,
    );
    assert_eq!(out, "42\n");
}

#[test]
fn let_immutable_non_mut_method_ok() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    val: int

    fn get(self) int {
        return self.val
    }
}

fn main() {
    let c = Counter { val: 42 }
    print(c.get())
}
"#,
    );
    assert_eq!(out, "42\n");
}

#[test]
fn fn_param_is_mutable() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    val: int

    fn inc(mut self) {
        self.val = self.val + 1
    }

    fn get(self) int {
        return self.val
    }
}

fn bump(c: Counter) {
    c.val = c.val + 10
    c.inc()
}

fn main() {
    let mut c = Counter { val: 0 }
    bump(c)
    print(c.get())
}
"#,
    );
    assert_eq!(out, "11\n");
}

#[test]
fn let_immutable_field_assign_inner_rejected() {
    compile_should_fail_with(
        r#"
class Inner {
    val: int
}

class Outer {
    inner: Inner
}

fn main() {
    let o = Outer { inner: Inner { val: 0 } }
    o.inner = Inner { val: 1 }
}
"#,
        "cannot assign to field of immutable variable 'o'",
    );
}

#[test]
fn method_name_no_collision_with_underscore_function() {
    // Regression: top-level fn `A_b` and class `A` method `b` must not collide.
    // With `_` separator they both mangled to `A_b`; with `$` they're distinct.
    let out = compile_and_run_stdout(
        r#"
class A {
    x: int

    fn b(self) int {
        return self.x + 1
    }
}

fn A_b() int {
    return 99
}

fn main() {
    let a = A { x: 10 }
    print(a.b())
    print(A_b())
}
"#,
    );
    assert_eq!(out, "11\n99\n");
}
