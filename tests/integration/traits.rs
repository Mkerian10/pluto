mod common;
use common::{compile_and_run_stdout, compile_should_fail, compile_should_fail_with};

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

// ===== Batch 1: Core trait features =====

#[test]
fn trait_multiple_required_methods() {
    // Trait with 3 required methods, verify all work through dispatch
    let out = compile_and_run_stdout(r#"
trait Shape {
    fn area(self) int
    fn perimeter(self) int
    fn name(self) string
}

class Square impl Shape {
    side: int

    fn area(self) int {
        return self.side * self.side
    }

    fn perimeter(self) int {
        return self.side * 4
    }

    fn name(self) string {
        return "square"
    }
}

fn describe(s: Shape) {
    print(s.name())
    print(s.area())
    print(s.perimeter())
}

fn main() {
    let sq = Square { side: 5 }
    describe(sq)
}
"#);
    assert_eq!(out, "square\n25\n20\n");
}

#[test]
fn trait_mixed_required_and_default_methods() {
    // Trait with both required and default methods on same trait
    let out = compile_and_run_stdout(r#"
trait Describable {
    fn label(self) string

    fn describe(self) string {
        return "item"
    }

    fn priority(self) int {
        return 0
    }
}

class Task impl Describable {
    title: string

    fn label(self) string {
        return self.title
    }

    fn priority(self) int {
        return 5
    }
}

fn show(d: Describable) {
    print(d.label())
    print(d.describe())
    print(d.priority())
}

fn main() {
    let t = Task { title: "fix bug" }
    show(t)
}
"#);
    assert_eq!(out, "fix bug\nitem\n5\n");
}

#[test]
fn trait_void_returning_method() {
    // Trait method returning void
    let out = compile_and_run_stdout(r#"
trait Logger {
    fn log(self, msg: string)
}

class PrintLogger impl Logger {
    prefix: string

    fn log(self, msg: string) {
        print(self.prefix)
        print(msg)
    }
}

fn do_log(l: Logger) {
    l.log("hello")
}

fn main() {
    let l = PrintLogger { prefix: "LOG: " }
    do_log(l)
}
"#);
    assert_eq!(out, "LOG: \nhello\n");
}

#[test]
fn trait_string_returning_method() {
    // Trait method returning string, dispatched through trait param
    let out = compile_and_run_stdout(r#"
trait Named {
    fn name(self) string
}

class Person impl Named {
    first: string
    last: string

    fn name(self) string {
        return self.first
    }
}

fn greet(n: Named) {
    print(n.name())
}

fn main() {
    let p = Person { first: "Alice", last: "Smith" }
    greet(p)
}
"#);
    assert_eq!(out, "Alice\n");
}

#[test]
fn trait_method_multiple_param_types() {
    // Trait method with multiple params of different types
    let out = compile_and_run_stdout(r#"
trait Calculator {
    fn compute(self, x: int, scale: float, label: string) int
}

class Doubler impl Calculator {
    base: int

    fn compute(self, x: int, scale: float, label: string) int {
        print(label)
        return self.base + x * 2
    }
}

fn run(c: Calculator) {
    print(c.compute(10, 1.5, "result"))
}

fn main() {
    let d = Doubler { base: 100 }
    run(d)
}
"#);
    assert_eq!(out, "result\n120\n");
}

#[test]
fn trait_typed_array() {
    // COMPILER GAP: pushing a concrete class into a trait-typed array is not supported.
    // The type checker doesn't coerce Dog → Speaker for array push.
    compile_should_fail_with(r#"
trait Speaker {
    fn speak(self) int
}

class Dog impl Speaker {
    volume: int

    fn speak(self) int {
        return self.volume
    }
}

class Cat impl Speaker {
    volume: int

    fn speak(self) int {
        return self.volume * 3
    }
}

fn main() {
    let animals: [Speaker] = []
    animals.push(Dog { volume: 10 })
    animals.push(Cat { volume: 5 })
    animals.push(Dog { volume: 7 })
    let i = 0
    while i < animals.len() {
        print(animals[i].speak())
        i = i + 1
    }
}
"#, "expected trait Speaker");
}

#[test]
fn trait_typed_array_iteration() {
    // COMPILER GAP: same as trait_typed_array — push() rejects concrete class for trait-typed array
    compile_should_fail_with(r#"
trait Valued {
    fn value(self) int
}

class A impl Valued {
    x: int

    fn value(self) int {
        return self.x
    }
}

class B impl Valued {
    y: int

    fn value(self) int {
        return self.y + 100
    }
}

fn main() {
    let items: [Valued] = []
    items.push(A { x: 1 })
    items.push(B { y: 2 })
    items.push(A { x: 3 })
    let total = 0
    for item in items {
        total = total + item.value()
    }
    print(total)
}
"#, "expected trait Valued");
}

#[test]
fn trait_typed_class_field() {
    // COMPILER GAP: struct literal doesn't coerce concrete class to trait-typed field
    compile_should_fail_with(r#"
trait Worker {
    fn work(self) int
}

class FastWorker impl Worker {
    speed: int

    fn work(self) int {
        return self.speed * 2
    }
}

class Manager {
    employee: Worker
    bonus: int
}

fn main() {
    let w = FastWorker { speed: 10 }
    let m = Manager { employee: w, bonus: 5 }
    print(m.employee.work())
    print(m.bonus)
}
"#, "expected trait Worker");
}

#[test]
fn trait_empty_trait() {
    // Empty trait (marker trait) — zero methods
    let out = compile_and_run_stdout(r#"
trait Marker {
}

class X impl Marker {
    val: int
}

fn accept(m: Marker) {
    print(42)
}

fn main() {
    let x = X { val: 1 }
    accept(x)
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn trait_triple_indirection() {
    // Pass trait handle through 3 functions before dispatch
    let out = compile_and_run_stdout(r#"
trait Greeter {
    fn greet(self) int
}

class Hello impl Greeter {
    val: int

    fn greet(self) int {
        return self.val
    }
}

fn level1(g: Greeter) {
    level2(g)
}

fn level2(g: Greeter) {
    level3(g)
}

fn level3(g: Greeter) {
    print(g.greet())
}

fn main() {
    let h = Hello { val: 99 }
    level1(h)
}
"#);
    assert_eq!(out, "99\n");
}

#[test]
fn trait_vtable_ordering_three_methods() {
    // 3 methods in trait, verify calling first, second, third all route correctly
    let out = compile_and_run_stdout(r#"
trait Triple {
    fn first(self) int
    fn second(self) int
    fn third(self) int
}

class Impl impl Triple {
    x: int

    fn first(self) int {
        return self.x
    }

    fn second(self) int {
        return self.x * 10
    }

    fn third(self) int {
        return self.x * 100
    }
}

fn test_dispatch(t: Triple) {
    print(t.first())
    print(t.second())
    print(t.third())
}

fn main() {
    let i = Impl { x: 3 }
    test_dispatch(i)
}
"#);
    assert_eq!(out, "3\n30\n300\n");
}

#[test]
fn trait_default_calls_required() {
    // Default method calls a required method through self — template method pattern
    let out = compile_and_run_stdout(r#"
trait Formatter {
    fn prefix(self) string

    fn format(self, msg: string) string {
        return self.prefix()
    }
}

class BangFormatter impl Formatter {
    tag: string

    fn prefix(self) string {
        return self.tag
    }
}

fn main() {
    let f = BangFormatter { tag: "!!!" }
    print(f.format("hello"))
}
"#);
    assert_eq!(out, "!!!\n");
}

#[test]
fn trait_default_calls_required_via_dispatch() {
    // Template method pattern through dynamic dispatch
    let out = compile_and_run_stdout(r#"
trait Processor {
    fn step(self) int

    fn run(self) int {
        return self.step() + 1
    }
}

class MyProc impl Processor {
    base: int

    fn step(self) int {
        return self.base * 2
    }
}

fn execute(p: Processor) {
    print(p.run())
}

fn main() {
    let p = MyProc { base: 10 }
    execute(p)
}
"#);
    assert_eq!(out, "21\n");
}

#[test]
fn trait_recursive_method_dispatch() {
    // Trait method that calls itself recursively through self
    let out = compile_and_run_stdout(r#"
trait Counter {
    fn count_down(self, n: int) int
}

class MyCounter impl Counter {
    step: int

    fn count_down(self, n: int) int {
        if n <= 0 {
            return 0
        }
        return n + self.count_down(n - self.step)
    }
}

fn run(c: Counter) {
    print(c.count_down(10))
}

fn main() {
    let c = MyCounter { step: 1 }
    run(c)
}
"#);
    assert_eq!(out, "55\n");
}

#[test]
fn trait_concrete_type_direct_call() {
    // Call a trait method directly on the concrete type (not via trait handle)
    let out = compile_and_run_stdout(r#"
trait HasVal {
    fn get(self) int
}

class X impl HasVal {
    val: int

    fn get(self) int {
        return self.val
    }
}

fn main() {
    let x = X { val: 42 }
    print(x.get())
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn trait_class_with_extra_methods() {
    // Class has methods beyond what the trait requires
    let out = compile_and_run_stdout(r#"
trait HasName {
    fn name(self) string
}

class Person impl HasName {
    first: string
    age: int

    fn name(self) string {
        return self.first
    }

    fn get_age(self) int {
        return self.age
    }
}

fn show_name(n: HasName) {
    print(n.name())
}

fn main() {
    let p = Person { first: "Bob", age: 30 }
    show_name(p)
    print(p.get_age())
}
"#);
    assert_eq!(out, "Bob\n30\n");
}

#[test]
fn trait_same_object_two_traits() {
    // Same object dispatched through two different traits
    let out = compile_and_run_stdout(r#"
trait HasX {
    fn get_x(self) int
}

trait HasY {
    fn get_y(self) int
}

class Point impl HasX, HasY {
    x: int
    y: int

    fn get_x(self) int {
        return self.x
    }

    fn get_y(self) int {
        return self.y
    }
}

fn show_x(h: HasX) {
    print(h.get_x())
}

fn show_y(h: HasY) {
    print(h.get_y())
}

fn main() {
    let p = Point { x: 10, y: 20 }
    show_x(p)
    show_y(p)
}
"#);
    assert_eq!(out, "10\n20\n");
}

#[test]
fn trait_method_calls_other_self_method() {
    // Trait method implementation calls another method on self
    let out = compile_and_run_stdout(r#"
trait Compute {
    fn compute(self) int
}

class Calc impl Compute {
    x: int
    y: int

    fn helper(self) int {
        return self.x + self.y
    }

    fn compute(self) int {
        return self.helper() * 2
    }
}

fn run(c: Compute) {
    print(c.compute())
}

fn main() {
    let c = Calc { x: 3, y: 7 }
    run(c)
}
"#);
    assert_eq!(out, "20\n");
}

#[test]
fn trait_multiple_methods_called_in_sequence() {
    // Call multiple methods on same trait handle in sequence
    let out = compile_and_run_stdout(r#"
trait Stats {
    fn min_val(self) int
    fn max_val(self) int
    fn avg_val(self) int
}

class Data impl Stats {
    a: int
    b: int
    c: int

    fn min_val(self) int {
        if self.a < self.b {
            if self.a < self.c {
                return self.a
            }
            return self.c
        }
        if self.b < self.c {
            return self.b
        }
        return self.c
    }

    fn max_val(self) int {
        if self.a > self.b {
            if self.a > self.c {
                return self.a
            }
            return self.c
        }
        if self.b > self.c {
            return self.b
        }
        return self.c
    }

    fn avg_val(self) int {
        return (self.a + self.b + self.c) / 3
    }
}

fn show_stats(s: Stats) {
    print(s.min_val())
    print(s.max_val())
    print(s.avg_val())
}

fn main() {
    let d = Data { a: 10, b: 30, c: 20 }
    show_stats(d)
}
"#);
    assert_eq!(out, "10\n30\n20\n");
}

#[test]
fn trait_string_interpolation_with_dispatch() {
    // Use trait dispatch result inside string interpolation
    let out = compile_and_run_stdout(r#"
trait Named {
    fn name(self) string
}

class Pet impl Named {
    n: string

    fn name(self) string {
        return self.n
    }
}

fn greet(x: Named) {
    print("hello {x.name()}")
}

fn main() {
    let p = Pet { n: "Max" }
    greet(p)
}
"#);
    assert_eq!(out, "hello Max\n");
}

// ===== Batch 2: Negative tests & edge cases =====

#[test]
fn fail_wrong_param_count() {
    // Trait expects fn foo(self, x: int), class has fn foo(self)
    compile_should_fail_with(r#"
trait Adder {
    fn add(self, x: int) int
}

class MyAdder impl Adder {
    base: int

    fn add(self) int {
        return self.base
    }
}

fn main() {
}
"#, "wrong number of parameters");
}

#[test]
fn fail_wrong_param_type() {
    // Trait expects fn foo(self, x: int), class has fn foo(self, x: string)
    compile_should_fail_with(r#"
trait Adder {
    fn add(self, x: int) int
}

class MyAdder impl Adder {
    base: int

    fn add(self, x: string) int {
        return self.base
    }
}

fn main() {
}
"#, "type mismatch");
}

#[test]
fn fail_extra_params() {
    // Trait expects fn foo(self, x: int), class has fn foo(self, x: int, y: int)
    compile_should_fail_with(r#"
trait Adder {
    fn add(self, x: int) int
}

class MyAdder impl Adder {
    base: int

    fn add(self, x: int, y: int) int {
        return self.base + x + y
    }
}

fn main() {
}
"#, "wrong number of parameters");
}

#[test]
fn fail_impl_class_name() {
    // impl a class name instead of a trait name
    compile_should_fail(r#"
class Other {
    val: int
}

class X impl Other {
    val: int
}

fn main() {
}
"#);
}

#[test]
fn fail_impl_enum_name() {
    // impl an enum name instead of a trait name
    compile_should_fail(r#"
enum Color {
    Red
    Blue
}

class X impl Color {
    val: int
}

fn main() {
}
"#);
}

#[test]
fn fail_non_implementing_class_as_trait_param() {
    // Class has matching methods but doesn't declare impl — should fail
    compile_should_fail(r#"
trait Worker {
    fn work(self) int
}

class NotAWorker {
    val: int

    fn work(self) int {
        return self.val
    }
}

fn use_worker(w: Worker) {
    print(w.work())
}

fn main() {
    let x = NotAWorker { val: 42 }
    use_worker(x)
}
"#);
}

#[test]
fn fail_call_non_trait_method_on_handle() {
    // Dog has fetch() but Worker trait doesn't — calling on trait handle should fail
    compile_should_fail(r#"
trait Worker {
    fn work(self) int
}

class Dog impl Worker {
    val: int

    fn work(self) int {
        return self.val
    }

    fn fetch(self) int {
        return 99
    }
}

fn use_worker(w: Worker) {
    print(w.fetch())
}

fn main() {
    let d = Dog { val: 1 }
    use_worker(d)
}
"#);
}

#[test]
fn fail_access_field_on_trait_handle() {
    // Cannot access concrete class fields through trait handle
    compile_should_fail(r#"
trait Worker {
    fn work(self) int
}

class Dog impl Worker {
    val: int

    fn work(self) int {
        return self.val
    }
}

fn use_worker(w: Worker) {
    print(w.val)
}

fn main() {
    let d = Dog { val: 42 }
    use_worker(d)
}
"#);
}

#[test]
fn fail_assign_primitive_to_trait() {
    // Cannot assign int to trait-typed variable
    compile_should_fail(r#"
trait Worker {
    fn work(self) int
}

fn main() {
    let w: Worker = 42
}
"#);
}

#[test]
fn fail_assign_incompatible_class_to_trait() {
    // Class doesn't implement the trait
    compile_should_fail(r#"
trait Worker {
    fn work(self) int
}

class NotWorker {
    val: int
}

fn main() {
    let w: Worker = NotWorker { val: 1 }
}
"#);
}

#[test]
fn trait_duplicate_trait_in_impl_allowed() {
    // COMPILER GAP: class Foo impl Bar, Bar — duplicate trait is silently accepted
    let out = compile_and_run_stdout(r#"
trait Bar {
    fn get(self) int
}

class Foo impl Bar, Bar {
    val: int

    fn get(self) int {
        return self.val
    }
}

fn show(b: Bar) {
    print(b.get())
}

fn main() {
    let f = Foo { val: 7 }
    show(f)
}
"#);
    assert_eq!(out, "7\n");
}

#[test]
fn trait_declared_after_class() {
    // Forward reference: trait declared below the class in source order
    let out = compile_and_run_stdout(r#"
class X impl Foo {
    val: int

    fn bar(self) int {
        return self.val
    }
}

trait Foo {
    fn bar(self) int
}

fn main() {
    let x = X { val: 77 }
    print(x.bar())
}
"#);
    assert_eq!(out, "77\n");
}

#[test]
fn trait_method_returning_class() {
    // COMPILER GAP: class type referenced in trait method return type is not found
    // during trait registration (forward-reference issue for class types in trait signatures)
    compile_should_fail_with(r#"
class Output {
    code: int
}

trait Producer {
    fn produce(self) Output
}

class Factory impl Producer {
    base: int

    fn produce(self) Output {
        return Output { code: self.base * 10 }
    }
}

fn run(p: Producer) {
    let r = p.produce()
    print(r.code)
}

fn main() {
    let f = Factory { base: 5 }
    run(f)
}
"#, "unknown type");
}

#[test]
fn trait_method_returning_array() {
    // Trait method returns an array through dispatch
    let out = compile_and_run_stdout(r#"
trait Lister {
    fn items(self) [int]
}

class Range impl Lister {
    n: int

    fn items(self) [int] {
        let result: [int] = []
        let i = 0
        while i < self.n {
            result.push(i)
            i = i + 1
        }
        return result
    }
}

fn show(l: Lister) {
    let items = l.items()
    print(items.len())
}

fn main() {
    let r = Range { n: 5 }
    show(r)
}
"#);
    assert_eq!(out, "5\n");
}

#[test]
fn trait_method_with_closure_param() {
    // Trait method takes a closure parameter
    let out = compile_and_run_stdout(r#"
trait Transformer {
    fn transform(self, f: fn(int) int) int
}

class Box impl Transformer {
    val: int

    fn transform(self, f: fn(int) int) int {
        return f(self.val)
    }
}

fn run(t: Transformer) {
    let result = t.transform((x: int) => x * 3)
    print(result)
}

fn main() {
    let b = Box { val: 7 }
    run(b)
}
"#);
    assert_eq!(out, "21\n");
}

#[test]
fn trait_method_with_enum_param() {
    // COMPILER GAP: enum type referenced in trait method signature is not found
    // during trait registration (forward-reference issue for enums in trait method params)
    compile_should_fail_with(r#"
enum Op {
    Add
    Multiply
}

trait Calculator {
    fn calc(self, op: Op) int
}

class Pair impl Calculator {
    a: int
    b: int

    fn calc(self, op: Op) int {
        match op {
            Op.Add {
                return self.a + self.b
            }
            Op.Multiply {
                return self.a * self.b
            }
        }
    }
}

fn run(c: Calculator) {
    print(c.calc(Op.Add))
    print(c.calc(Op.Multiply))
}

fn main() {
    let p = Pair { a: 3, b: 4 }
    run(p)
}
"#, "unknown type");
}

#[test]
fn trait_two_traits_same_method_no_contracts() {
    // Two traits define same method name, same signature, no contracts — class provides one impl
    // This may or may not be supported
    let out = compile_and_run_stdout(r#"
trait Readable {
    fn read(self) int
}

trait Gettable {
    fn read(self) int
}

class X impl Readable, Gettable {
    val: int

    fn read(self) int {
        return self.val
    }
}

fn use_readable(r: Readable) {
    print(r.read())
}

fn use_gettable(g: Gettable) {
    print(g.read())
}

fn main() {
    let x = X { val: 42 }
    use_readable(x)
    use_gettable(x)
}
"#);
    assert_eq!(out, "42\n42\n");
}

#[test]
fn trait_class_many_heap_fields() {
    // Class with many heap-allocated fields implementing a trait — GC stress
    let out = compile_and_run_stdout(r#"
trait Summarizer {
    fn summary(self) string
}

class Record impl Summarizer {
    a: string
    b: string
    c: string
    d: string
    e: int

    fn summary(self) string {
        return self.a
    }
}

fn show(s: Summarizer) {
    print(s.summary())
}

fn main() {
    let r = Record { a: "hello", b: "world", c: "foo", d: "bar", e: 42 }
    show(r)
}
"#);
    assert_eq!(out, "hello\n");
}

#[test]
fn trait_method_many_params_stress() {
    // 8 params through dynamic dispatch — stress test ABI / calling convention
    let out = compile_and_run_stdout(r#"
trait BigSig {
    fn compute(self, a: int, b: int, c: int, d: int, e: int, f: int, g: int, h: int) int
}

class Impl impl BigSig {
    base: int

    fn compute(self, a: int, b: int, c: int, d: int, e: int, f: int, g: int, h: int) int {
        return self.base + a + b + c + d + e + f + g + h
    }
}

fn run(s: BigSig) {
    print(s.compute(1, 2, 3, 4, 5, 6, 7, 8))
}

fn main() {
    let i = Impl { base: 100 }
    run(i)
}
"#);
    assert_eq!(out, "136\n");
}

#[test]
fn trait_default_calls_free_function() {
    // Default method body calls a free function
    let out = compile_and_run_stdout(r#"
fn helper(x: int) int {
    return x * 10
}

trait Processor {
    fn process(self) int {
        return helper(5)
    }
}

class X impl Processor {
    val: int
}

fn run(p: Processor) {
    print(p.process())
}

fn main() {
    let x = X { val: 1 }
    run(x)
}
"#);
    assert_eq!(out, "50\n");
}

// ===== Batch 3: Closures, concurrency, more edge cases =====

#[test]
fn trait_handle_as_closure_capture() {
    // Closure captures a trait-typed variable and dispatches on it
    let out = compile_and_run_stdout(r#"
trait Speaker {
    fn speak(self) int
}

class Dog impl Speaker {
    volume: int

    fn speak(self) int {
        return self.volume
    }
}

fn make_fn(s: Speaker) fn() int {
    return () => s.speak()
}

fn main() {
    let d = Dog { volume: 42 }
    let f = make_fn(d)
    print(f())
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn trait_dispatch_in_spawn() {
    // Trait handle passed to spawned function and dispatched in another thread
    let out = compile_and_run_stdout(r#"
trait Worker {
    fn work(self) int
}

class FastWorker impl Worker {
    val: int

    fn work(self) int {
        return self.val * 2
    }
}

fn do_work(w: Worker) int {
    return w.work()
}

fn main() {
    let w = FastWorker { val: 21 }
    let t = spawn do_work(w)
    print(t.get())
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn trait_dispatch_loop_stress() {
    // Dispatch in a tight loop — verify no memory leak/corruption
    let out = compile_and_run_stdout(r#"
trait Counter {
    fn count(self) int
}

class C impl Counter {
    val: int

    fn count(self) int {
        return self.val
    }
}

fn sum_n(c: Counter, n: int) int {
    let total = 0
    let i = 0
    while i < n {
        total = total + c.count()
        i = i + 1
    }
    return total
}

fn main() {
    let c = C { val: 1 }
    print(sum_n(c, 1000))
}
"#);
    assert_eq!(out, "1000\n");
}

#[test]
fn trait_method_returns_trait() {
    // Trait method that returns another trait-typed value
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn value(self) int
}

trait Factory {
    fn make(self) Valued
}

class SimpleVal impl Valued {
    x: int

    fn value(self) int {
        return self.x
    }
}

class ValFactory impl Factory {
    base: int

    fn make(self) Valued {
        return SimpleVal { x: self.base }
    }
}

fn run(f: Factory) {
    let v = f.make()
    print(v.value())
}

fn main() {
    let f = ValFactory { base: 99 }
    run(f)
}
"#);
    assert_eq!(out, "99\n");
}

#[test]
fn trait_method_takes_trait_param() {
    // Trait method that takes another trait-typed parameter
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn value(self) int
}

trait Combiner {
    fn combine(self, other: Valued) int
}

class SimpleVal impl Valued {
    x: int

    fn value(self) int {
        return self.x
    }
}

class Adder impl Combiner {
    base: int

    fn combine(self, other: Valued) int {
        return self.base + other.value()
    }
}

fn run(c: Combiner, v: Valued) {
    print(c.combine(v))
}

fn main() {
    let a = Adder { base: 100 }
    let v = SimpleVal { x: 23 }
    run(a, v)
}
"#);
    assert_eq!(out, "123\n");
}

#[test]
fn trait_default_with_loop() {
    // Default method body contains a loop
    let out = compile_and_run_stdout(r#"
trait Summer {
    fn limit(self) int

    fn sum(self) int {
        let total = 0
        let i = 1
        while i <= self.limit() {
            total = total + i
            i = i + 1
        }
        return total
    }
}

class TenSummer impl Summer {
    tag: int

    fn limit(self) int {
        return 10
    }
}

fn run(s: Summer) {
    print(s.sum())
}

fn main() {
    let s = TenSummer { tag: 0 }
    run(s)
}
"#);
    assert_eq!(out, "55\n");
}

#[test]
fn trait_default_two_classes_different_behavior() {
    // Two classes use same default method but provide different required method impls
    let out = compile_and_run_stdout(r#"
trait Greeter {
    fn prefix(self) string

    fn greet(self) string {
        return self.prefix()
    }
}

class FormalGreeter impl Greeter {
    name: string

    fn prefix(self) string {
        return "Dear"
    }
}

class CasualGreeter impl Greeter {
    name: string

    fn prefix(self) string {
        return "Hey"
    }
}

fn show(g: Greeter) {
    print(g.greet())
}

fn main() {
    let f = FormalGreeter { name: "Alice" }
    let c = CasualGreeter { name: "Bob" }
    show(f)
    show(c)
}
"#);
    assert_eq!(out, "Dear\nHey\n");
}

#[test]
fn trait_method_returning_bool() {
    // Trait method returning bool through dispatch
    let out = compile_and_run_stdout(r#"
trait Checker {
    fn check(self, x: int) bool
}

class PositiveChecker impl Checker {
    threshold: int

    fn check(self, x: int) bool {
        return x > self.threshold
    }
}

fn run(c: Checker) {
    if c.check(10) {
        print(1)
    } else {
        print(0)
    }
    if c.check(3) {
        print(1)
    } else {
        print(0)
    }
}

fn main() {
    let c = PositiveChecker { threshold: 5 }
    run(c)
}
"#);
    assert_eq!(out, "1\n0\n");
}

#[test]
fn trait_method_returning_float() {
    // Trait method returning float through dispatch
    let out = compile_and_run_stdout(r#"
trait Measurable {
    fn measure(self) float
}

class Circle impl Measurable {
    radius: float

    fn measure(self) float {
        return self.radius * 2.0
    }
}

fn show(m: Measurable) {
    print(m.measure())
}

fn main() {
    let c = Circle { radius: 3.5 }
    show(c)
}
"#);
    assert_eq!(out, "7.000000\n");
}

#[test]
fn trait_handle_same_object_two_handles() {
    // Same object wrapped as two different trait handles
    let out = compile_and_run_stdout(r#"
trait HasX {
    fn get_x(self) int
}

trait HasY {
    fn get_y(self) int
}

class Point impl HasX, HasY {
    x: int
    y: int

    fn get_x(self) int {
        return self.x
    }

    fn get_y(self) int {
        return self.y
    }
}

fn main() {
    let p = Point { x: 10, y: 20 }
    let hx: HasX = p
    let hy: HasY = p
    print(hx.get_x())
    print(hy.get_y())
}
"#);
    assert_eq!(out, "10\n20\n");
}

#[test]
fn fail_trait_method_unknown_return_type() {
    // Trait method returning a type that doesn't exist
    compile_should_fail(r#"
trait Foo {
    fn bar(self) UnknownType
}

fn main() {
}
"#);
}

#[test]
fn fail_trait_method_unknown_param_type() {
    // Trait method with a param type that doesn't exist
    compile_should_fail(r#"
trait Foo {
    fn bar(self, x: UnknownType) int
}

fn main() {
}
"#);
}

#[test]
fn fail_impl_function_name() {
    // impl a function name instead of trait
    compile_should_fail(r#"
fn some_func() int {
    return 1
}

class X impl some_func {
    val: int
}

fn main() {
}
"#);
}

#[test]
fn fail_trait_handle_to_concrete_function() {
    // Cannot pass trait-typed value to function expecting concrete type
    compile_should_fail(r#"
trait Worker {
    fn work(self) int
}

class Dog impl Worker {
    val: int

    fn work(self) int {
        return self.val
    }
}

fn use_dog(d: Dog) {
    print(d.val)
}

fn main() {
    let w: Worker = Dog { val: 1 }
    use_dog(w)
}
"#);
}

#[test]
fn fail_assign_one_trait_to_different_trait() {
    // Cannot assign TraitA-typed value to TraitB variable
    compile_should_fail(r#"
trait TraitA {
    fn a(self) int
}

trait TraitB {
    fn b(self) int
}

class X impl TraitA {
    val: int

    fn a(self) int {
        return self.val
    }
}

fn main() {
    let a: TraitA = X { val: 1 }
    let b: TraitB = a
}
"#);
}

#[test]
fn trait_five_traits_on_one_class() {
    // Stress test: class implements 5 traits
    let out = compile_and_run_stdout(r#"
trait T1 {
    fn m1(self) int
}
trait T2 {
    fn m2(self) int
}
trait T3 {
    fn m3(self) int
}
trait T4 {
    fn m4(self) int
}
trait T5 {
    fn m5(self) int
}

class X impl T1, T2, T3, T4, T5 {
    val: int

    fn m1(self) int { return self.val + 1 }
    fn m2(self) int { return self.val + 2 }
    fn m3(self) int { return self.val + 3 }
    fn m4(self) int { return self.val + 4 }
    fn m5(self) int { return self.val + 5 }
}

fn use1(t: T1) { print(t.m1()) }
fn use2(t: T2) { print(t.m2()) }
fn use3(t: T3) { print(t.m3()) }
fn use4(t: T4) { print(t.m4()) }
fn use5(t: T5) { print(t.m5()) }

fn main() {
    let x = X { val: 10 }
    use1(x)
    use2(x)
    use3(x)
    use4(x)
    use5(x)
}
"#);
    assert_eq!(out, "11\n12\n13\n14\n15\n");
}

#[test]
fn trait_method_named_len() {
    // Method name collides with built-in array method — verify no collision after mangling
    let out = compile_and_run_stdout(r#"
trait Sizable {
    fn len(self) int
}

class MyList impl Sizable {
    count: int

    fn len(self) int {
        return self.count
    }
}

fn show(s: Sizable) {
    print(s.len())
}

fn main() {
    let m = MyList { count: 42 }
    show(m)
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn trait_very_long_names() {
    // Stress test name mangling with very long trait/method names
    let out = compile_and_run_stdout(r#"
trait VeryLongTraitNameForTesting {
    fn very_long_method_name_for_testing(self) int
}

class X impl VeryLongTraitNameForTesting {
    val: int

    fn very_long_method_name_for_testing(self) int {
        return self.val
    }
}

fn run(t: VeryLongTraitNameForTesting) {
    print(t.very_long_method_name_for_testing())
}

fn main() {
    let x = X { val: 77 }
    run(x)
}
"#);
    assert_eq!(out, "77\n");
}

#[test]
fn trait_method_param_shadows_builtin() {
    // Method parameter named "print" shadows the builtin function
    let out = compile_and_run_stdout(r#"
trait Foo {
    fn bar(self, val: int) int
}

class X impl Foo {
    base: int

    fn bar(self, val: int) int {
        return self.base + val
    }
}

fn run(f: Foo) {
    print(f.bar(10))
}

fn main() {
    let x = X { base: 5 }
    run(x)
}
"#);
    assert_eq!(out, "15\n");
}

// ===== Batch 4: Diamond/conflict, generics, errors, misc edge cases =====

#[test]
fn trait_diamond_two_defaults_same_method() {
    // Two traits both define default for same method — class doesn't override
    // Contracts guard rejects this. Without contracts, behavior varies.
    compile_should_fail(r#"
trait A {
    fn work(self) int {
        return 1
    }
}

trait B {
    fn work(self) int {
        return 2
    }
}

class C impl A, B {
    val: int
}

fn main() {
}
"#);
}

#[test]
fn trait_diamond_class_overrides() {
    // Two traits define same method, class provides its own — should satisfy both
    let out = compile_and_run_stdout(r#"
trait A {
    fn work(self) int
}

trait B {
    fn work(self) int
}

class C impl A, B {
    val: int

    fn work(self) int {
        return self.val
    }
}

fn use_a(a: A) {
    print(a.work())
}

fn use_b(b: B) {
    print(b.work())
}

fn main() {
    let c = C { val: 42 }
    use_a(c)
    use_b(c)
}
"#);
    assert_eq!(out, "42\n42\n");
}

#[test]
fn trait_diamond_different_signatures() {
    // Two traits define same method name with different return types — should fail
    compile_should_fail(r#"
trait A {
    fn work(self) int
}

trait B {
    fn work(self) string
}

class C impl A, B {
    val: int

    fn work(self) int {
        return self.val
    }
}

fn main() {
}
"#);
}

#[test]
fn trait_three_traits_same_method() {
    // Three traits all define the same method name — maximum ambiguity
    compile_should_fail(r#"
trait A {
    fn work(self) int {
        return 1
    }
}

trait B {
    fn work(self) int {
        return 2
    }
}

trait C {
    fn work(self) int {
        return 3
    }
}

class X impl A, B, C {
    val: int
}

fn main() {
}
"#);
}

#[test]
fn trait_generic_class_impl() {
    // Generic class implements a trait, method doesn't use type param
    let out = compile_and_run_stdout(r#"
trait Sizable {
    fn size(self) int
}

class Stack<T> impl Sizable {
    count: int

    fn size(self) int {
        return self.count
    }
}

fn show(s: Sizable) {
    print(s.size())
}

fn main() {
    let s = Stack<int> { count: 5 }
    show(s)
}
"#);
    assert_eq!(out, "5\n");
}

#[test]
fn trait_generic_class_two_instantiations() {
    // Two instantiations of generic class, each dispatched through same trait
    let out = compile_and_run_stdout(r#"
trait HasSize {
    fn size(self) int
}

class Container<T> impl HasSize {
    count: int

    fn size(self) int {
        return self.count
    }
}

fn show(h: HasSize) {
    print(h.size())
}

fn main() {
    let a = Container<int> { count: 3 }
    let b = Container<string> { count: 7 }
    show(a)
    show(b)
}
"#);
    assert_eq!(out, "3\n7\n");
}

#[test]
fn trait_error_handling_catch() {
    // Trait dispatch with error catching
    let out = compile_and_run_stdout(r#"
error MathErr {
    code: int
}

trait Adder {
    fn add(self, x: int) int
}

class SafeAdder impl Adder {
    limit: int

    fn add(self, x: int) int {
        if x > self.limit {
            raise MathErr { code: 1 }
        }
        return x + 1
    }
}

fn use_adder(a: Adder) int {
    let result = a.add(1000) catch -1
    return result
}

fn main() {
    let a = SafeAdder { limit: 100 }
    print(use_adder(a))
}
"#);
    assert_eq!(out, "-1\n");
}

#[test]
fn trait_error_propagation() {
    // Error propagation through trait dispatch with !
    let out = compile_and_run_stdout(r#"
error CalcErr {
    code: int
}

trait Calculator {
    fn calc(self, x: int) int
}

class Divider impl Calculator {
    divisor: int

    fn calc(self, x: int) int {
        if self.divisor == 0 {
            raise CalcErr { code: 1 }
        }
        return x / self.divisor
    }
}

fn run_calc(c: Calculator, x: int) int {
    return c.calc(x)!
}

fn main() {
    let d = Divider { divisor: 2 }
    let result = run_calc(d, 10) catch 0
    print(result)
}
"#);
    assert_eq!(out, "5\n");
}

#[test]
fn trait_self_method_call_in_dispatch() {
    // Method implementation calls another trait method on self (not on another object)
    let out = compile_and_run_stdout(r#"
trait Compute {
    fn base(self) int
    fn doubled(self) int
}

class X impl Compute {
    val: int

    fn base(self) int {
        return self.val
    }

    fn doubled(self) int {
        return self.base() * 2
    }
}

fn run(c: Compute) {
    print(c.base())
    print(c.doubled())
}

fn main() {
    let x = X { val: 5 }
    run(x)
}
"#);
    assert_eq!(out, "5\n10\n");
}

#[test]
fn trait_four_classes_same_trait() {
    // 4 classes implementing same trait, verify each dispatches correctly
    let out = compile_and_run_stdout(r#"
trait Numbered {
    fn num(self) int
}

class A impl Numbered {
    val: int
    fn num(self) int { return 1 }
}

class B impl Numbered {
    val: int
    fn num(self) int { return 2 }
}

class C impl Numbered {
    val: int
    fn num(self) int { return 3 }
}

class D impl Numbered {
    val: int
    fn num(self) int { return 4 }
}

fn show(n: Numbered) {
    print(n.num())
}

fn main() {
    show(A { val: 0 })
    show(B { val: 0 })
    show(C { val: 0 })
    show(D { val: 0 })
}
"#);
    assert_eq!(out, "1\n2\n3\n4\n");
}

#[test]
fn trait_default_not_overridden_plus_overridden() {
    // Trait with 3 methods: 2 default, 1 required. Class overrides one default.
    let out = compile_and_run_stdout(r#"
trait Config {
    fn port(self) int {
        return 8080
    }

    fn host(self) string {
        return "localhost"
    }

    fn name(self) string
}

class AppConfig impl Config {
    app_name: string

    fn port(self) int {
        return 3000
    }

    fn name(self) string {
        return self.app_name
    }
}

fn show(c: Config) {
    print(c.port())
    print(c.host())
    print(c.name())
}

fn main() {
    let c = AppConfig { app_name: "myapp" }
    show(c)
}
"#);
    assert_eq!(out, "3000\nlocalhost\nmyapp\n");
}

#[test]
fn trait_method_returning_nullable() {
    // Trait method returns nullable type through dispatch
    let out = compile_and_run_stdout(r#"
trait Finder {
    fn find(self, key: int) int?
}

class SimpleFinder impl Finder {
    target: int

    fn find(self, key: int) int? {
        if key == self.target {
            return key * 10
        }
        return none
    }
}

fn search(f: Finder, key: int) int {
    let result = f.find(key)
    if result != none {
        return result?
    }
    return -1
}

fn main() {
    let f = SimpleFinder { target: 5 }
    print(search(f, 5))
    print(search(f, 3))
}
"#);
    assert_eq!(out, "50\n-1\n");
}

#[test]
fn trait_method_with_nullable_param() {
    // Trait method takes nullable param through dispatch
    let out = compile_and_run_stdout(r#"
trait Processor {
    fn process(self, x: int?) int
}

class DefaultProcessor impl Processor {
    fallback: int

    fn process(self, x: int?) int {
        if x == none {
            return self.fallback
        }
        return 100
    }
}

fn run(p: Processor) {
    print(p.process(42))
    print(p.process(none))
}

fn main() {
    let p = DefaultProcessor { fallback: -1 }
    run(p)
}
"#);
    assert_eq!(out, "100\n-1\n");
}

#[test]
fn fail_trait_print_directly() {
    // Cannot print a trait-typed value directly (not Printable)
    compile_should_fail(r#"
trait Worker {
    fn work(self) int
}

class X impl Worker {
    val: int

    fn work(self) int {
        return self.val
    }
}

fn main() {
    let w: Worker = X { val: 1 }
    print(w)
}
"#);
}

#[test]
fn trait_equality_compiles() {
    // COMPILER GAP: trait handle == comparison compiles (compares pointers, not values)
    // Two different objects with same values: different handles → not equal
    let out = compile_and_run_stdout(r#"
trait Worker {
    fn work(self) int
}

class X impl Worker {
    val: int

    fn work(self) int {
        return self.val
    }
}

fn main() {
    let a: Worker = X { val: 1 }
    let b: Worker = X { val: 1 }
    if a == b {
        print(1)
    } else {
        print(0)
    }
}
"#);
    // Different trait handle objects → pointer comparison → not equal
    assert_eq!(out, "0\n");
}

#[test]
fn fail_trait_as_map_key() {
    // Trait type cannot be used as map key
    compile_should_fail(r#"
trait Worker {
    fn work(self) int
}

fn main() {
    let m = Map<Worker, int> {}
}
"#);
}

#[test]
fn trait_method_with_array_param() {
    // Trait method takes an array parameter
    let out = compile_and_run_stdout(r#"
trait Summable {
    fn sum(self, values: [int]) int
}

class Adder impl Summable {
    base: int

    fn sum(self, values: [int]) int {
        let total = self.base
        let i = 0
        while i < values.len() {
            total = total + values[i]
            i = i + 1
        }
        return total
    }
}

fn run(s: Summable) {
    let nums: [int] = [1, 2, 3, 4, 5]
    print(s.sum(nums))
}

fn main() {
    let a = Adder { base: 100 }
    run(a)
}
"#);
    assert_eq!(out, "115\n");
}

#[test]
fn trait_method_closure_return() {
    // Trait method returns a closure
    let out = compile_and_run_stdout(r#"
trait Factory {
    fn make_fn(self) fn(int) int
}

class Doubler impl Factory {
    multiplier: int

    fn make_fn(self) fn(int) int {
        let m = self.multiplier
        return (x: int) => x * m
    }
}

fn run(f: Factory) {
    let func = f.make_fn()
    print(func(5))
}

fn main() {
    let d = Doubler { multiplier: 3 }
    run(d)
}
"#);
    assert_eq!(out, "15\n");
}

#[test]
fn fail_trait_self_trait_param() {
    // COMPILER GAP: trait method referencing its own trait name as a parameter type
    // is not resolved — the trait name isn't available during method signature parsing
    compile_should_fail_with(r#"
trait Comparable {
    fn value(self) int
    fn greater_than(self, other: Comparable) bool
}

class Num impl Comparable {
    n: int

    fn value(self) int {
        return self.n
    }

    fn greater_than(self, other: Comparable) bool {
        return self.n > other.value()
    }
}

fn main() {
    let a = Num { n: 10 }
    let b = Num { n: 5 }
    if a.greater_than(b) {
        print(1)
    } else {
        print(0)
    }
}
"#, "unknown type");
}

// ===== Batch 5: Complex dispatch patterns, reassignment, nesting =====

#[test]
fn trait_reassign_trait_variable() {
    // Assign different concrete classes to the same trait-typed variable
    let out = compile_and_run_stdout(r#"
trait Speaker {
    fn speak(self) int
}

class Dog impl Speaker {
    val: int
    fn speak(self) int { return 1 }
}

class Cat impl Speaker {
    val: int
    fn speak(self) int { return 2 }
}

fn main() {
    let s: Speaker = Dog { val: 0 }
    print(s.speak())
    s = Cat { val: 0 }
    print(s.speak())
}
"#);
    assert_eq!(out, "1\n2\n");
}

#[test]
fn trait_dispatch_in_if_condition() {
    // Trait method call result used directly in if condition
    let out = compile_and_run_stdout(r#"
trait Checker {
    fn is_valid(self) bool
}

class PositiveChecker impl Checker {
    val: int

    fn is_valid(self) bool {
        return self.val > 0
    }
}

fn check(c: Checker) {
    if c.is_valid() {
        print(1)
    } else {
        print(0)
    }
}

fn main() {
    check(PositiveChecker { val: 5 })
    check(PositiveChecker { val: -3 })
}
"#);
    assert_eq!(out, "1\n0\n");
}

#[test]
fn trait_dispatch_in_while_condition() {
    // Trait method call used in while loop condition (read-only dispatch)
    let out = compile_and_run_stdout(r#"
trait Limiter {
    fn limit(self) int
}

class MaxLimiter impl Limiter {
    max: int
    fn limit(self) int { return self.max }
}

fn count_up(l: Limiter) {
    let i = 0
    while i < l.limit() {
        i = i + 1
    }
    print(i)
}

fn main() {
    count_up(MaxLimiter { max: 5 })
}
"#);
    assert_eq!(out, "5\n");
}

#[test]
fn trait_multiple_trait_params_in_function() {
    // Function takes two different trait-typed parameters
    let out = compile_and_run_stdout(r#"
trait Namer {
    fn name(self) string
}

trait Scorer {
    fn score(self) int
}

class Player impl Namer {
    n: string
    fn name(self) string { return self.n }
}

class Game impl Scorer {
    s: int
    fn score(self) int { return self.s }
}

fn report(n: Namer, s: Scorer) {
    print(n.name())
    print(s.score())
}

fn main() {
    let p = Player { n: "alice" }
    let g = Game { s: 100 }
    report(p, g)
}
"#);
    assert_eq!(out, "alice\n100\n");
}

#[test]
fn trait_nested_dispatch_chain() {
    // Trait method returns a value, which is passed to another trait method
    let out = compile_and_run_stdout(r#"
trait Producer {
    fn produce(self) int
}

trait Consumer {
    fn consume(self, val: int) int
}

class Maker impl Producer {
    base: int
    fn produce(self) int { return self.base }
}

class Doubler impl Consumer {
    factor: int
    fn consume(self, val: int) int { return val * self.factor }
}

fn pipeline(p: Producer, c: Consumer) {
    let val = p.produce()
    print(c.consume(val))
}

fn main() {
    let m = Maker { base: 7 }
    let d = Doubler { factor: 3 }
    pipeline(m, d)
}
"#);
    assert_eq!(out, "21\n");
}

#[test]
fn trait_handle_in_nested_function_calls() {
    // f(g(trait_handle.method())) — nested call with dispatch result
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn double(x: int) int {
    return x * 2
}

fn add_one(x: int) int {
    return x + 1
}

fn main() {
    let v: Valued = X { n: 5 }
    print(add_one(double(v.val())))
}
"#);
    assert_eq!(out, "11\n");
}

#[test]
fn trait_recursive_function_with_handle() {
    // Recursive function taking a trait handle — dispatches at each level
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn count_down(v: Valued, remaining: int) int {
    if remaining <= 0 {
        return v.val()
    }
    return count_down(v, remaining - 1) + 1
}

fn main() {
    let x = X { n: 100 }
    print(count_down(x, 5))
}
"#);
    assert_eq!(out, "105\n");
}

#[test]
fn trait_dispatch_result_as_array_index() {
    // Trait method returns an index used to access an array
    let out = compile_and_run_stdout(r#"
trait Indexer {
    fn index(self) int
}

class Selector impl Indexer {
    idx: int
    fn index(self) int { return self.idx }
}

fn main() {
    let arr: [int] = [10, 20, 30, 40, 50]
    let s: Indexer = Selector { idx: 2 }
    print(arr[s.index()])
}
"#);
    assert_eq!(out, "30\n");
}

#[test]
fn trait_dispatch_result_in_arithmetic() {
    // Use trait dispatch results directly in arithmetic expressions
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class A impl Valued {
    n: int
    fn val(self) int { return self.n }
}

class B impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn main() {
    let a: Valued = A { n: 10 }
    let b: Valued = B { n: 3 }
    print(a.val() + b.val())
    print(a.val() * b.val())
    print(a.val() - b.val())
}
"#);
    assert_eq!(out, "13\n30\n7\n");
}

#[test]
fn trait_dispatch_result_in_string_concat() {
    // Trait method returns string, concatenated with other strings
    let out = compile_and_run_stdout(r#"
trait Named {
    fn name(self) string
}

class Person impl Named {
    n: string
    fn name(self) string { return self.n }
}

fn greet(n: Named) {
    print("hello " + n.name())
}

fn main() {
    greet(Person { n: "world" })
}
"#);
    assert_eq!(out, "hello world\n");
}

#[test]
fn trait_handle_passed_through_five_functions() {
    // Deep call chain: 5 functions passing trait handle
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn f1(v: Valued) int { return f2(v) }
fn f2(v: Valued) int { return f3(v) }
fn f3(v: Valued) int { return f4(v) }
fn f4(v: Valued) int { return f5(v) }
fn f5(v: Valued) int { return v.val() }

fn main() {
    let x = X { n: 42 }
    print(f1(x))
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn trait_two_handles_same_trait_in_scope() {
    // Two different trait handles of same trait type in the same function
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class A impl Valued {
    n: int
    fn val(self) int { return self.n }
}

class B impl Valued {
    n: int
    fn val(self) int { return self.n * 10 }
}

fn compare(x: Valued, y: Valued) {
    if x.val() > y.val() {
        print(1)
    } else {
        print(0)
    }
}

fn main() {
    compare(A { n: 50 }, B { n: 3 })
    compare(A { n: 2 }, B { n: 1 })
}
"#);
    assert_eq!(out, "1\n0\n");
}

#[test]
fn trait_dispatch_method_with_string_interpolation() {
    // Trait method uses string interpolation in its body
    let out = compile_and_run_stdout(r#"
trait Describer {
    fn describe(self) string
}

class Item impl Describer {
    name: string
    count: int

    fn describe(self) string {
        return "{self.name}: {self.count}"
    }
}

fn show(d: Describer) {
    print(d.describe())
}

fn main() {
    show(Item { name: "apples", count: 5 })
}
"#);
    assert_eq!(out, "apples: 5\n");
}

#[test]
fn trait_method_body_with_while_loop() {
    // Trait method implementation contains a while loop
    let out = compile_and_run_stdout(r#"
trait Summer {
    fn sum_to(self, n: int) int
}

class Adder impl Summer {
    base: int

    fn sum_to(self, n: int) int {
        let total = self.base
        let i = 1
        while i <= n {
            total = total + i
            i = i + 1
        }
        return total
    }
}

fn run(s: Summer) {
    print(s.sum_to(5))
}

fn main() {
    run(Adder { base: 100 })
}
"#);
    assert_eq!(out, "115\n");
}

#[test]
fn trait_method_body_with_for_loop() {
    // Trait method implementation uses for loop
    let out = compile_and_run_stdout(r#"
trait Summer {
    fn sum_range(self, start: int, end: int) int
}

class RangeSummer impl Summer {
    multiplier: int

    fn sum_range(self, start: int, end: int) int {
        let total = 0
        for i in start..end {
            total = total + i * self.multiplier
        }
        return total
    }
}

fn run(s: Summer) {
    print(s.sum_range(1, 4))
}

fn main() {
    run(RangeSummer { multiplier: 2 })
}
"#);
    assert_eq!(out, "12\n");
}

#[test]
fn trait_method_body_creates_and_returns_object() {
    // Trait method creates a new class instance and returns a field from it
    let out = compile_and_run_stdout(r#"
class Point {
    x: int
    y: int
}

trait PointMaker {
    fn make(self) int
}

class Factory impl PointMaker {
    scale: int

    fn make(self) int {
        let p = Point { x: self.scale * 2, y: self.scale * 3 }
        return p.x + p.y
    }
}

fn run(pm: PointMaker) {
    print(pm.make())
}

fn main() {
    run(Factory { scale: 10 })
}
"#);
    assert_eq!(out, "50\n");
}

#[test]
fn trait_method_body_with_array_operations() {
    // Trait method creates and manipulates arrays
    let out = compile_and_run_stdout(r#"
trait Collector {
    fn collect(self, n: int) int
}

class SumCollector impl Collector {
    base: int

    fn collect(self, n: int) int {
        let arr: [int] = []
        let i = 0
        while i < n {
            arr.push(i + self.base)
            i = i + 1
        }
        let total = 0
        let j = 0
        while j < arr.len() {
            total = total + arr[j]
            j = j + 1
        }
        return total
    }
}

fn run(c: Collector) {
    print(c.collect(4))
}

fn main() {
    run(SumCollector { base: 10 })
}
"#);
    assert_eq!(out, "46\n");
}

#[test]
fn trait_all_default_methods() {
    // Trait with only default methods, no required methods
    let out = compile_and_run_stdout(r#"
trait Defaults {
    fn a(self) int {
        return 1
    }

    fn b(self) int {
        return 2
    }

    fn c(self) int {
        return 3
    }
}

class X impl Defaults {
    val: int
}

fn run(d: Defaults) {
    print(d.a())
    print(d.b())
    print(d.c())
}

fn main() {
    run(X { val: 0 })
}
"#);
    assert_eq!(out, "1\n2\n3\n");
}

#[test]
fn trait_default_calls_another_default() {
    // Default method calls another default method on self
    let out = compile_and_run_stdout(r#"
trait Compute {
    fn base(self) int {
        return 10
    }

    fn doubled(self) int {
        return self.base() * 2
    }
}

class X impl Compute {
    val: int
}

fn run(c: Compute) {
    print(c.doubled())
}

fn main() {
    run(X { val: 0 })
}
"#);
    assert_eq!(out, "20\n");
}

#[test]
fn trait_large_trait_ten_methods() {
    // Trait with 10 methods — tests vtable size handling
    let out = compile_and_run_stdout(r#"
trait BigTrait {
    fn m1(self) int
    fn m2(self) int
    fn m3(self) int
    fn m4(self) int
    fn m5(self) int
    fn m6(self) int
    fn m7(self) int
    fn m8(self) int
    fn m9(self) int
    fn m10(self) int
}

class Impl impl BigTrait {
    val: int

    fn m1(self) int { return 1 }
    fn m2(self) int { return 2 }
    fn m3(self) int { return 3 }
    fn m4(self) int { return 4 }
    fn m5(self) int { return 5 }
    fn m6(self) int { return 6 }
    fn m7(self) int { return 7 }
    fn m8(self) int { return 8 }
    fn m9(self) int { return 9 }
    fn m10(self) int { return 10 }
}

fn run(b: BigTrait) {
    print(b.m1() + b.m2() + b.m3() + b.m4() + b.m5())
    print(b.m6() + b.m7() + b.m8() + b.m9() + b.m10())
}

fn main() {
    run(Impl { val: 0 })
}
"#);
    assert_eq!(out, "15\n40\n");
}

// ===== Batch 6: Negative tests, naming edge cases, DI interaction =====

#[test]
fn trait_same_name_as_class_allowed() {
    // COMPILER GAP: Trait and class can have the same name — compiler doesn't reject it
    // This documents current behavior; may want to reject in the future
    let out = compile_and_run_stdout(r#"
trait Foo {
    fn work(self) int
}

class Foo impl Foo {
    val: int
    fn work(self) int { return self.val }
}

fn run(f: Foo) {
    print(f.work())
}

fn main() {
    let f = Foo { val: 42 }
    run(f)
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn trait_two_traits_same_name_allowed() {
    // COMPILER GAP: Two traits with the same name — compiler doesn't reject it
    // The second definition silently overwrites the first
    let out = compile_and_run_stdout(r#"
trait Foo {
    fn a(self) int
}

trait Foo {
    fn b(self) int
}

class X impl Foo {
    val: int
    fn b(self) int { return self.val }
}

fn run(f: Foo) {
    print(f.b())
}

fn main() {
    run(X { val: 99 })
}
"#);
    assert_eq!(out, "99\n");
}

#[test]
fn fail_impl_trait_missing_one_of_two_methods() {
    // Class implements trait but is missing one of two required methods
    compile_should_fail_with(r#"
trait Duo {
    fn first(self) int
    fn second(self) int
}

class X impl Duo {
    val: int

    fn first(self) int {
        return self.val
    }
}

fn main() {
}
"#, "second");
}

#[test]
fn fail_impl_trait_wrong_return_type_on_one_method() {
    // Class has right method names but one has wrong return type
    compile_should_fail(r#"
trait Pair {
    fn name(self) string
    fn age(self) int
}

class Person impl Pair {
    n: string
    a: int

    fn name(self) string {
        return self.n
    }

    fn age(self) string {
        return self.n
    }
}

fn main() {
}
"#);
}

#[test]
fn trait_method_with_map_param() {
    // Trait method takes a Map parameter
    let out = compile_and_run_stdout(r#"
trait Lookup {
    fn get(self, m: Map<string, int>, key: string) int
}

class MapLookup impl Lookup {
    default_val: int

    fn get(self, m: Map<string, int>, key: string) int {
        if m.contains(key) {
            return m[key]
        }
        return self.default_val
    }
}

fn run(l: Lookup) {
    let m = Map<string, int> { "a": 1, "b": 2 }
    print(l.get(m, "a"))
    print(l.get(m, "c"))
}

fn main() {
    run(MapLookup { default_val: -1 })
}
"#);
    assert_eq!(out, "1\n-1\n");
}

#[test]
fn trait_method_returning_string_in_interp() {
    // Trait dispatch result used directly in string interpolation
    let out = compile_and_run_stdout(r#"
trait Named {
    fn name(self) string
}

class User impl Named {
    n: string
    fn name(self) string { return self.n }
}

fn greet(n: Named) {
    print("hi {n.name()}")
}

fn main() {
    greet(User { n: "bob" })
}
"#);
    assert_eq!(out, "hi bob\n");
}

#[test]
fn trait_method_returning_int_in_interp() {
    // Trait dispatch int result used in string interpolation
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn show(v: Valued) {
    print("value={v.val()}")
}

fn main() {
    show(X { n: 42 })
}
"#);
    assert_eq!(out, "value=42\n");
}

#[test]
fn trait_dispatch_in_for_range() {
    // Trait method result used as range bound in for loop
    let out = compile_and_run_stdout(r#"
trait Bounded {
    fn upper(self) int
}

class Limit impl Bounded {
    max: int
    fn upper(self) int { return self.max }
}

fn sum_to(b: Bounded) {
    let total = 0
    for i in 0..b.upper() {
        total = total + i
    }
    print(total)
}

fn main() {
    sum_to(Limit { max: 5 })
}
"#);
    assert_eq!(out, "10\n");
}

#[test]
fn trait_class_with_many_fields_dispatch() {
    // Class with 6 fields implements trait, ensure vtable works with complex layout
    let out = compile_and_run_stdout(r#"
trait Summary {
    fn total(self) int
}

class Record impl Summary {
    a: int
    b: int
    c: int
    d: int
    e: int
    f: int

    fn total(self) int {
        return self.a + self.b + self.c + self.d + self.e + self.f
    }
}

fn show(s: Summary) {
    print(s.total())
}

fn main() {
    show(Record { a: 1, b: 2, c: 3, d: 4, e: 5, f: 6 })
}
"#);
    assert_eq!(out, "21\n");
}

#[test]
fn trait_multiple_classes_different_field_counts() {
    // Two classes with very different field counts, same trait
    let out = compile_and_run_stdout(r#"
trait Sized {
    fn size(self) int
}

class Small impl Sized {
    n: int
    fn size(self) int { return 1 }
}

class Large impl Sized {
    a: int
    b: int
    c: int
    d: int
    e: int

    fn size(self) int { return 5 }
}

fn show(s: Sized) {
    print(s.size())
}

fn main() {
    show(Small { n: 0 })
    show(Large { a: 1, b: 2, c: 3, d: 4, e: 5 })
}
"#);
    assert_eq!(out, "1\n5\n");
}

#[test]
fn trait_method_with_bool_param() {
    // Trait method takes bool parameter
    let out = compile_and_run_stdout(r#"
trait Toggler {
    fn toggle(self, on: bool) int
}

class Switch impl Toggler {
    val: int

    fn toggle(self, on: bool) int {
        if on {
            return self.val
        }
        return 0
    }
}

fn run(t: Toggler) {
    print(t.toggle(true))
    print(t.toggle(false))
}

fn main() {
    run(Switch { val: 42 })
}
"#);
    assert_eq!(out, "42\n0\n");
}

#[test]
fn trait_method_with_float_param() {
    // Trait method takes float parameter
    let out = compile_and_run_stdout(r#"
trait Scaler {
    fn scale(self, factor: float) float
}

class Value impl Scaler {
    base: float

    fn scale(self, factor: float) float {
        return self.base * factor
    }
}

fn run(s: Scaler) {
    print(s.scale(2.0))
}

fn main() {
    run(Value { base: 3.5 })
}
"#);
    assert_eq!(out, "7.000000\n");
}

#[test]
fn trait_dispatch_with_type_cast() {
    // Trait method returns int, cast to float
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn main() {
    let v: Valued = X { n: 5 }
    let f = v.val() as float
    print(f)
}
"#);
    assert_eq!(out, "5.000000\n");
}

#[test]
fn trait_default_method_with_if_else() {
    // Default method with if/else control flow
    let out = compile_and_run_stdout(r#"
trait Classifier {
    fn value(self) int

    fn classify(self) string {
        if self.value() > 0 {
            return "positive"
        } else {
            return "non-positive"
        }
    }
}

class Num impl Classifier {
    n: int
    fn value(self) int { return self.n }
}

fn run(c: Classifier) {
    print(c.classify())
}

fn main() {
    run(Num { n: 5 })
    run(Num { n: -3 })
}
"#);
    assert_eq!(out, "positive\nnon-positive\n");
}

#[test]
fn trait_method_returns_result_of_free_function() {
    // Trait method body calls a free function and returns its result
    let out = compile_and_run_stdout(r#"
fn compute(x: int) int {
    return x * x + 1
}

trait Computer {
    fn run(self) int
}

class Wrapper impl Computer {
    input: int

    fn run(self) int {
        return compute(self.input)
    }
}

fn dispatch(c: Computer) {
    print(c.run())
}

fn main() {
    dispatch(Wrapper { input: 4 })
}
"#);
    assert_eq!(out, "17\n");
}

#[test]
fn fail_trait_method_returns_enum() {
    // COMPILER GAP: Enum type used in trait method return type is not found
    // (forward reference issue — enum not yet registered when trait is parsed)
    compile_should_fail_with(r#"
enum Status {
    Active
    Inactive
}

trait Stateful {
    fn status(self) Status
}

class Server impl Stateful {
    running: bool

    fn status(self) Status {
        if self.running {
            return Status.Active
        }
        return Status.Inactive
    }
}

fn main() {
}
"#, "unknown type");
}

#[test]
fn trait_class_with_string_field_dispatch() {
    // Class with string fields, ensure heap types work through dispatch
    let out = compile_and_run_stdout(r#"
trait Greeter {
    fn greet(self) string
}

class Formal impl Greeter {
    title: string
    name: string

    fn greet(self) string {
        return self.title + " " + self.name
    }
}

class Casual impl Greeter {
    nickname: string

    fn greet(self) string {
        return "hey " + self.nickname
    }
}

fn show(g: Greeter) {
    print(g.greet())
}

fn main() {
    show(Formal { title: "Dr", name: "Smith" })
    show(Casual { nickname: "Bob" })
}
"#);
    assert_eq!(out, "Dr Smith\nhey Bob\n");
}

#[test]
fn trait_method_modifies_array_param() {
    // Trait method receives array and modifies it (arrays are heap, passed by reference)
    let out = compile_and_run_stdout(r#"
trait Filler {
    fn fill(self, arr: [int])
}

class ConstFiller impl Filler {
    value: int

    fn fill(self, arr: [int]) {
        let i = 0
        while i < 3 {
            arr.push(self.value)
            i = i + 1
        }
    }
}

fn run(f: Filler) {
    let arr: [int] = []
    f.fill(arr)
    print(arr.len())
    print(arr[0])
    print(arr[2])
}

fn main() {
    run(ConstFiller { value: 7 })
}
"#);
    assert_eq!(out, "3\n7\n7\n");
}

#[test]
fn trait_six_classes_same_trait_dispatch() {
    // 6 classes implementing same trait — stress test vtable generation
    let out = compile_and_run_stdout(r#"
trait Id {
    fn id(self) int
}

class C1 impl Id { val: int  fn id(self) int { return 1 } }
class C2 impl Id { val: int  fn id(self) int { return 2 } }
class C3 impl Id { val: int  fn id(self) int { return 3 } }
class C4 impl Id { val: int  fn id(self) int { return 4 } }
class C5 impl Id { val: int  fn id(self) int { return 5 } }
class C6 impl Id { val: int  fn id(self) int { return 6 } }

fn show(i: Id) {
    print(i.id())
}

fn main() {
    show(C1 { val: 0 })
    show(C2 { val: 0 })
    show(C3 { val: 0 })
    show(C4 { val: 0 })
    show(C5 { val: 0 })
    show(C6 { val: 0 })
}
"#);
    assert_eq!(out, "1\n2\n3\n4\n5\n6\n");
}

// ===== Batch 7: DI interaction, generics + traits, more stress =====

#[test]
fn trait_with_di_bracket_deps() {
    // Class with DI bracket deps implements a trait — dispatched through trait handle
    let out = compile_and_run_stdout(r#"
trait Logger {
    fn log(self) int
}

class ConsoleLogger impl Logger {
    prefix: int

    fn log(self) int {
        return self.prefix + 42
    }
}

class Service[logger: ConsoleLogger] {
    tag: int

    fn run(self) int {
        return self.logger.log()
    }
}

fn use_logger(l: Logger) {
    print(l.log())
}

app MyApp[service: Service] {
    fn main(self) {
        print(self.service.run())
    }
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn trait_generic_class_method_uses_field() {
    // Generic class implements trait, method accesses generic-typed field
    let out = compile_and_run_stdout(r#"
trait HasLen {
    fn len_val(self) int
}

class Wrapper<T> impl HasLen {
    items: [int]

    fn len_val(self) int {
        return self.items.len()
    }
}

fn show(h: HasLen) {
    print(h.len_val())
}

fn main() {
    let w = Wrapper<string> { items: [1, 2, 3] }
    show(w)
}
"#);
    assert_eq!(out, "3\n");
}

#[test]
fn trait_dispatch_in_match_arm_body() {
    // Trait dispatch happens inside a match arm body
    let out = compile_and_run_stdout(r#"
enum Mode {
    Fast
    Slow
}

trait Runner {
    fn speed(self) int
}

class Racer impl Runner {
    base: int
    fn speed(self) int { return self.base * 2 }
}

fn run(mode: Mode, r: Runner) {
    match mode {
        Mode.Fast {
            print(r.speed())
        }
        Mode.Slow {
            print(r.speed() / 2)
        }
    }
}

fn main() {
    let r = Racer { base: 10 }
    run(Mode.Fast, r)
    run(Mode.Slow, r)
}
"#);
    assert_eq!(out, "20\n10\n");
}

#[test]
fn trait_dispatch_with_bitwise_ops() {
    // Trait method result used with bitwise operations
    let out = compile_and_run_stdout(r#"
trait Masked {
    fn mask(self) int
}

class BitMask impl Masked {
    bits: int
    fn mask(self) int { return self.bits }
}

fn apply(m: Masked, val: int) {
    print(val & m.mask())
}

fn main() {
    let m = BitMask { bits: 15 }
    apply(m, 255)
    apply(m, 8)
}
"#);
    assert_eq!(out, "15\n8\n");
}

#[test]
fn trait_default_method_string_return() {
    // Default method returns a string constant
    let out = compile_and_run_stdout(r#"
trait Described {
    fn description(self) string {
        return "no description"
    }
}

class Thing impl Described {
    val: int
}

class Named impl Described {
    name: string

    fn description(self) string {
        return self.name
    }
}

fn show(d: Described) {
    print(d.description())
}

fn main() {
    show(Thing { val: 1 })
    show(Named { name: "widget" })
}
"#);
    assert_eq!(out, "no description\nwidget\n");
}

#[test]
fn trait_method_with_negative_literals() {
    // Trait method works with negative number comparisons
    let out = compile_and_run_stdout(r#"
trait Signum {
    fn sign(self) int
}

class Num impl Signum {
    val: int

    fn sign(self) int {
        if self.val > 0 {
            return 1
        }
        if self.val < 0 {
            return -1
        }
        return 0
    }
}

fn show(s: Signum) {
    print(s.sign())
}

fn main() {
    show(Num { val: 10 })
    show(Num { val: -5 })
    show(Num { val: 0 })
}
"#);
    assert_eq!(out, "1\n-1\n0\n");
}

#[test]
fn trait_method_early_return() {
    // Trait method with early return (guard pattern)
    let out = compile_and_run_stdout(r#"
trait Validator {
    fn validate(self, x: int) int
}

class RangeValidator impl Validator {
    min: int
    max: int

    fn validate(self, x: int) int {
        if x < self.min {
            return self.min
        }
        if x > self.max {
            return self.max
        }
        return x
    }
}

fn run(v: Validator) {
    print(v.validate(5))
    print(v.validate(-10))
    print(v.validate(100))
}

fn main() {
    run(RangeValidator { min: 0, max: 50 })
}
"#);
    assert_eq!(out, "5\n0\n50\n");
}

#[test]
fn trait_two_traits_two_classes_cross_dispatch() {
    // Two traits, two classes, each implements one — cross-dispatch
    let out = compile_and_run_stdout(r#"
trait Reader {
    fn read(self) int
}

trait Writer {
    fn write(self, val: int) int
}

class Source impl Reader {
    data: int
    fn read(self) int { return self.data }
}

class Sink impl Writer {
    offset: int
    fn write(self, val: int) int { return val + self.offset }
}

fn transfer(r: Reader, w: Writer) {
    let val = r.read()
    print(w.write(val))
}

fn main() {
    transfer(Source { data: 10 }, Sink { offset: 5 })
}
"#);
    assert_eq!(out, "15\n");
}

#[test]
fn trait_dispatch_then_method_chain() {
    // Dispatch through trait, then call concrete method on result (string.len())
    let out = compile_and_run_stdout(r#"
trait Named {
    fn name(self) string
}

class User impl Named {
    n: string
    fn name(self) string { return self.n }
}

fn name_length(n: Named) {
    let s = n.name()
    print(s.len())
}

fn main() {
    name_length(User { n: "hello" })
}
"#);
    assert_eq!(out, "5\n");
}

#[test]
fn trait_dispatch_both_paths_if_else() {
    // If/else where both branches dispatch on trait handle
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
    fn double(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
    fn double(self) int { return self.n * 2 }
}

fn pick(v: Valued, use_double: bool) {
    if use_double {
        print(v.double())
    } else {
        print(v.val())
    }
}

fn main() {
    let x = X { n: 7 }
    pick(x, true)
    pick(x, false)
}
"#);
    assert_eq!(out, "14\n7\n");
}

#[test]
fn trait_dispatch_in_let_binding() {
    // Trait dispatch result bound to let, then used multiple times
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn main() {
    let v: Valued = X { n: 10 }
    let result = v.val()
    print(result)
    print(result + 1)
    print(result * 2)
}
"#);
    assert_eq!(out, "10\n11\n20\n");
}

#[test]
fn fail_trait_method_body_match_on_enum_param() {
    // COMPILER GAP: Enum type used in trait method parameter is not found
    // (forward reference issue)
    compile_should_fail_with(r#"
enum Op {
    Add
    Sub
}

trait Calculator {
    fn calc(self, op: Op, a: int, b: int) int
}

class SimpleCalc impl Calculator {
    val: int

    fn calc(self, op: Op, a: int, b: int) int {
        match op {
            Op.Add {
                return a + b
            }
            Op.Sub {
                return a - b
            }
        }
    }
}

fn main() {
}
"#, "unknown type");
}

#[test]
fn trait_concrete_and_trait_call_same_method() {
    // Same method called both through concrete type and through trait handle
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn via_trait(v: Valued) {
    print(v.val())
}

fn main() {
    let x = X { n: 42 }
    print(x.val())
    via_trait(x)
}
"#);
    assert_eq!(out, "42\n42\n");
}

#[test]
fn trait_class_impl_two_traits_dispatch_each() {
    // Class implements two traits, both are dispatched through separately
    let out = compile_and_run_stdout(r#"
trait Readable {
    fn read(self) int
}

trait Writable {
    fn write(self, val: int) int
}

class Buffer impl Readable, Writable {
    data: int

    fn read(self) int {
        return self.data
    }

    fn write(self, val: int) int {
        return val + self.data
    }
}

fn read_it(r: Readable) {
    print(r.read())
}

fn write_it(w: Writable) {
    print(w.write(10))
}

fn main() {
    let b = Buffer { data: 5 }
    read_it(b)
    write_it(b)
}
"#);
    assert_eq!(out, "5\n15\n");
}

#[test]
fn trait_dispatch_return_used_as_param() {
    // Return value of one trait dispatch used as parameter to another
    let out = compile_and_run_stdout(r#"
trait Generator {
    fn generate(self) int
}

trait Printer {
    fn show(self, val: int)
}

class NumGen impl Generator {
    base: int
    fn generate(self) int { return self.base * 3 }
}

class NumPrinter impl Printer {
    prefix: int

    fn show(self, val: int) {
        print(self.prefix + val)
    }
}

fn chain(g: Generator, p: Printer) {
    p.show(g.generate())
}

fn main() {
    chain(NumGen { base: 4 }, NumPrinter { prefix: 100 })
}
"#);
    assert_eq!(out, "112\n");
}

#[test]
fn trait_method_with_string_comparison() {
    // Trait method performs string comparison
    let out = compile_and_run_stdout(r#"
trait Matcher {
    fn matches(self, input: string) bool
}

class ExactMatcher impl Matcher {
    expected: string

    fn matches(self, input: string) bool {
        return input == self.expected
    }
}

fn check(m: Matcher, input: string) {
    if m.matches(input) {
        print(1)
    } else {
        print(0)
    }
}

fn main() {
    let m = ExactMatcher { expected: "hello" }
    check(m, "hello")
    check(m, "world")
}
"#);
    assert_eq!(out, "1\n0\n");
}

#[test]
fn trait_dispatch_in_nested_if() {
    // Trait dispatch inside nested if/else
    let out = compile_and_run_stdout(r#"
trait Scorer {
    fn score(self) int
}

class Player impl Scorer {
    points: int
    fn score(self) int { return self.points }
}

fn grade(s: Scorer) {
    if s.score() > 90 {
        print("A")
    } else {
        if s.score() > 70 {
            print("B")
        } else {
            print("C")
        }
    }
}

fn main() {
    grade(Player { points: 95 })
    grade(Player { points: 80 })
    grade(Player { points: 50 })
}
"#);
    assert_eq!(out, "A\nB\nC\n");
}

#[test]
fn trait_method_accumulates_across_calls() {
    // Multiple dispatch calls on same handle, accumulating results
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn accumulate(v: Valued) {
    let sum = 0
    sum = sum + v.val()
    sum = sum + v.val()
    sum = sum + v.val()
    print(sum)
}

fn main() {
    accumulate(X { n: 7 })
}
"#);
    assert_eq!(out, "21\n");
}

#[test]
fn trait_handle_as_return_value() {
    // Function creates a concrete class and returns it as a trait handle
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn make_valued() Valued {
    return X { n: 42 }
}

fn main() {
    let v = make_valued()
    print(v.val())
}
"#);
    assert_eq!(out, "42\n");
}

// ===== Batch 8: Closures + traits, factory patterns, more edge cases =====

#[test]
fn trait_closure_captures_and_dispatches() {
    // Closure captures a trait handle and dispatches inside it
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn make_fn(v: Valued) fn() int {
    return () => v.val()
}

fn main() {
    let x = X { n: 99 }
    let f = make_fn(x)
    print(f())
}
"#);
    assert_eq!(out, "99\n");
}

#[test]
fn trait_closure_param_dispatches() {
    // Function takes a closure that takes a trait handle
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn apply(f: fn(Valued) int, v: Valued) {
    print(f(v))
}

fn main() {
    let x = X { n: 7 }
    apply((v: Valued) => v.val() * 3, x)
}
"#);
    assert_eq!(out, "21\n");
}

#[test]
fn trait_factory_function_pattern() {
    // Factory function returns different concrete types as trait handles
    let out = compile_and_run_stdout(r#"
trait Worker {
    fn work(self) int
}

class Fast impl Worker {
    val: int
    fn work(self) int { return self.val * 2 }
}

class Slow impl Worker {
    val: int
    fn work(self) int { return self.val }
}

fn create_worker(fast: bool) Worker {
    if fast {
        return Fast { val: 10 }
    }
    return Slow { val: 10 }
}

fn main() {
    let w1 = create_worker(true)
    let w2 = create_worker(false)
    print(w1.work())
    print(w2.work())
}
"#);
    assert_eq!(out, "20\n10\n");
}

#[test]
fn trait_method_called_on_function_return() {
    // Chain: function returns trait handle, immediately dispatch method
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn make() Valued {
    return X { n: 55 }
}

fn main() {
    print(make().val())
}
"#);
    assert_eq!(out, "55\n");
}

#[test]
fn trait_multiple_methods_interleaved_calls() {
    // Call different trait methods in interleaved order
    let out = compile_and_run_stdout(r#"
trait Math {
    fn add(self, x: int) int
    fn mul(self, x: int) int
}

class Calc impl Math {
    base: int

    fn add(self, x: int) int {
        return self.base + x
    }

    fn mul(self, x: int) int {
        return self.base * x
    }
}

fn run(m: Math) {
    print(m.add(5))
    print(m.mul(3))
    print(m.add(10))
    print(m.mul(2))
}

fn main() {
    run(Calc { base: 4 })
}
"#);
    assert_eq!(out, "9\n12\n14\n8\n");
}

#[test]
fn trait_dispatch_preserves_class_state() {
    // Verify that dispatching through trait doesn't corrupt class field values
    let out = compile_and_run_stdout(r#"
trait Pair {
    fn first(self) int
    fn second(self) int
}

class Point impl Pair {
    x: int
    y: int

    fn first(self) int { return self.x }
    fn second(self) int { return self.y }
}

fn show(p: Pair) {
    print(p.first())
    print(p.second())
    print(p.first())
    print(p.second())
}

fn main() {
    show(Point { x: 42, y: 99 })
}
"#);
    assert_eq!(out, "42\n99\n42\n99\n");
}

#[test]
fn trait_class_with_string_and_int_fields() {
    // Class with mixed string and int fields, dispatched through trait
    let out = compile_and_run_stdout(r#"
trait Info {
    fn describe(self) string
    fn count(self) int
}

class Item impl Info {
    name: string
    qty: int

    fn describe(self) string {
        return self.name
    }

    fn count(self) int {
        return self.qty
    }
}

fn show(i: Info) {
    print(i.describe())
    print(i.count())
}

fn main() {
    show(Item { name: "widget", qty: 5 })
}
"#);
    assert_eq!(out, "widget\n5\n");
}

#[test]
fn trait_dispatch_across_multiple_scopes() {
    // Trait handle used across if/else scopes, verifying it persists
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn main() {
    let v: Valued = X { n: 10 }
    let result = 0
    if true {
        result = v.val()
    } else {
        result = 0
    }
    print(result)
    if false {
        result = 0
    } else {
        result = v.val() + 1
    }
    print(result)
}
"#);
    assert_eq!(out, "10\n11\n");
}

#[test]
fn trait_method_with_multiple_string_params() {
    // Trait method taking multiple string parameters
    let out = compile_and_run_stdout(r#"
trait Formatter {
    fn format(self, prefix: string, suffix: string) string
}

class Wrapper impl Formatter {
    middle: string

    fn format(self, prefix: string, suffix: string) string {
        return prefix + self.middle + suffix
    }
}

fn show(f: Formatter) {
    print(f.format("[", "]"))
}

fn main() {
    show(Wrapper { middle: "hello" })
}
"#);
    assert_eq!(out, "[hello]\n");
}

#[test]
fn trait_method_returning_large_computation() {
    // Method does substantial work before returning through dispatch
    let out = compile_and_run_stdout(r#"
trait Fibonacci {
    fn fib(self, n: int) int
}

class FibCalc impl Fibonacci {
    memo: int

    fn fib(self, n: int) int {
        if n <= 1 {
            return n
        }
        let a = 0
        let b = 1
        let i = 2
        while i <= n {
            let temp = a + b
            a = b
            b = temp
            i = i + 1
        }
        return b
    }
}

fn run(f: Fibonacci) {
    print(f.fib(10))
    print(f.fib(0))
    print(f.fib(1))
}

fn main() {
    run(FibCalc { memo: 0 })
}
"#);
    assert_eq!(out, "55\n0\n1\n");
}

#[test]
fn trait_default_method_with_match() {
    // Default method uses match on a bool (equivalent)
    let out = compile_and_run_stdout(r#"
trait Toggle {
    fn state(self) bool

    fn label(self) string {
        if self.state() {
            return "on"
        }
        return "off"
    }
}

class Switch impl Toggle {
    active: bool

    fn state(self) bool {
        return self.active
    }
}

fn show(t: Toggle) {
    print(t.label())
}

fn main() {
    show(Switch { active: true })
    show(Switch { active: false })
}
"#);
    assert_eq!(out, "on\noff\n");
}

#[test]
fn trait_method_with_zero_and_negative_returns() {
    // Trait method returning zero and negative values through dispatch
    let out = compile_and_run_stdout(r#"
trait Scorer {
    fn score(self) int
}

class Positive impl Scorer {
    val: int
    fn score(self) int { return self.val }
}

class Zero impl Scorer {
    val: int
    fn score(self) int { return 0 }
}

class Negative impl Scorer {
    val: int
    fn score(self) int { return -self.val }
}

fn show(s: Scorer) {
    print(s.score())
}

fn main() {
    show(Positive { val: 5 })
    show(Zero { val: 99 })
    show(Negative { val: 3 })
}
"#);
    assert_eq!(out, "5\n0\n-3\n");
}

#[test]
fn trait_dispatch_many_times_in_loop() {
    // Dispatch same trait handle many times in a loop (100 iterations)
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn sum_dispatches(v: Valued, count: int) {
    let total = 0
    let i = 0
    while i < count {
        total = total + v.val()
        i = i + 1
    }
    print(total)
}

fn main() {
    sum_dispatches(X { n: 3 }, 100)
}
"#);
    assert_eq!(out, "300\n");
}

#[test]
#[should_panic]
fn crash_trait_method_without_self() {
    // COMPILER BUG: Trait method without self parameter causes compiler panic
    // (range start index 1 out of range for slice of length 0)
    // Should produce a graceful error instead of crashing
    compile_and_run_stdout(r#"
trait Foo {
    fn work() int {
        return 42
    }
}

class X impl Foo {
    val: int
}

fn main() {
    print(42)
}
"#);
}

#[test]
fn trait_generic_class_three_instantiations_same_trait() {
    // Three different instantiations of a generic class, all dispatched through same trait
    let out = compile_and_run_stdout(r#"
trait HasCount {
    fn count(self) int
}

class Bag<T> impl HasCount {
    n: int

    fn count(self) int {
        return self.n
    }
}

fn show(h: HasCount) {
    print(h.count())
}

fn main() {
    show(Bag<int> { n: 1 })
    show(Bag<string> { n: 2 })
    show(Bag<bool> { n: 3 })
}
"#);
    assert_eq!(out, "1\n2\n3\n");
}

#[test]
fn trait_method_conditional_raise() {
    // Trait method conditionally raises error, handled by caller
    let out = compile_and_run_stdout(r#"
error Overflow {
    val: int
}

trait Bounded {
    fn add(self, x: int) int
}

class SafeInt impl Bounded {
    max: int

    fn add(self, x: int) int {
        if x > self.max {
            raise Overflow { val: x }
        }
        return x
    }
}

fn try_add(b: Bounded, x: int) {
    let result = b.add(x) catch -1
    print(result)
}

fn main() {
    let s = SafeInt { max: 10 }
    try_add(s, 5)
    try_add(s, 20)
}
"#);
    assert_eq!(out, "5\n-1\n");
}

#[test]
fn trait_eight_classes_vtable_stress() {
    // 8 classes implementing same trait — thorough vtable stress
    let out = compile_and_run_stdout(r#"
trait Id {
    fn id(self) int
}

class A impl Id { val: int  fn id(self) int { return 10 } }
class B impl Id { val: int  fn id(self) int { return 20 } }
class C impl Id { val: int  fn id(self) int { return 30 } }
class D impl Id { val: int  fn id(self) int { return 40 } }
class E impl Id { val: int  fn id(self) int { return 50 } }
class F impl Id { val: int  fn id(self) int { return 60 } }
class G impl Id { val: int  fn id(self) int { return 70 } }
class H impl Id { val: int  fn id(self) int { return 80 } }

fn show(i: Id) { print(i.id()) }

fn main() {
    let total = 0
    let a: Id = A { val: 0 }
    let b: Id = B { val: 0 }
    let c: Id = C { val: 0 }
    let d: Id = D { val: 0 }
    total = a.id() + b.id() + c.id() + d.id()
    print(total)
    let e: Id = E { val: 0 }
    let f: Id = F { val: 0 }
    let g: Id = G { val: 0 }
    let h: Id = H { val: 0 }
    total = e.id() + f.id() + g.id() + h.id()
    print(total)
}
"#);
    assert_eq!(out, "100\n260\n");
}

#[test]
fn trait_dispatch_string_then_use_len() {
    // Dispatch returns string, then call .len() on it
    let out = compile_and_run_stdout(r#"
trait Named {
    fn name(self) string
}

class Foo impl Named {
    n: string
    fn name(self) string { return self.n }
}

fn show_len(n: Named) {
    let s = n.name()
    print(s.len())
}

fn main() {
    show_len(Foo { n: "hello world" })
}
"#);
    assert_eq!(out, "11\n");
}

#[test]
fn trait_method_returns_bool_in_condition() {
    // Trait method returning bool used directly in && and ||
    let out = compile_and_run_stdout(r#"
trait Checker {
    fn check_a(self) bool
    fn check_b(self) bool
}

class Both impl Checker {
    a: bool
    b: bool

    fn check_a(self) bool { return self.a }
    fn check_b(self) bool { return self.b }
}

fn run(c: Checker) {
    if c.check_a() && c.check_b() {
        print(1)
    } else {
        print(0)
    }
}

fn main() {
    run(Both { a: true, b: true })
    run(Both { a: true, b: false })
    run(Both { a: false, b: true })
}
"#);
    assert_eq!(out, "1\n0\n0\n");
}

// ===== Batch 9: Corner cases, unusual patterns, more negative tests =====

#[test]
fn trait_same_trait_variable_reassigned_three_times() {
    // Reassign trait variable to three different concrete types
    let out = compile_and_run_stdout(r#"
trait Id {
    fn id(self) int
}

class A impl Id { val: int  fn id(self) int { return 1 } }
class B impl Id { val: int  fn id(self) int { return 2 } }
class C impl Id { val: int  fn id(self) int { return 3 } }

fn main() {
    let x: Id = A { val: 0 }
    print(x.id())
    x = B { val: 0 }
    print(x.id())
    x = C { val: 0 }
    print(x.id())
}
"#);
    assert_eq!(out, "1\n2\n3\n");
}

#[test]
fn trait_method_calls_math_builtin() {
    // Trait method body uses math builtins
    let out = compile_and_run_stdout(r#"
trait Compute {
    fn compute(self, x: int) int
}

class AbsComputer impl Compute {
    val: int

    fn compute(self, x: int) int {
        return abs(x) + self.val
    }
}

fn run(c: Compute) {
    print(c.compute(-5))
    print(c.compute(3))
}

fn main() {
    run(AbsComputer { val: 10 })
}
"#);
    assert_eq!(out, "15\n13\n");
}

#[test]
fn trait_method_uses_min_max() {
    // Trait method uses min/max builtins
    let out = compile_and_run_stdout(r#"
trait Clamper {
    fn clamp(self, x: int) int
}

class Range impl Clamper {
    lo: int
    hi: int

    fn clamp(self, x: int) int {
        return min(max(x, self.lo), self.hi)
    }
}

fn run(c: Clamper) {
    print(c.clamp(5))
    print(c.clamp(-10))
    print(c.clamp(100))
}

fn main() {
    run(Range { lo: 0, hi: 50 })
}
"#);
    assert_eq!(out, "5\n0\n50\n");
}

#[test]
fn trait_dispatch_in_ternary_style() {
    // Simulated ternary using if expression (Pluto has no ternary op)
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn pick(v: Valued) int {
    if v.val() > 5 {
        return v.val() * 2
    }
    return v.val()
}

fn main() {
    print(pick(X { n: 10 }))
    print(pick(X { n: 3 }))
}
"#);
    assert_eq!(out, "20\n3\n");
}

#[test]
fn trait_two_methods_same_name_different_traits_class_both() {
    // Two traits both require "name()", class implements both — should satisfy both
    let out = compile_and_run_stdout(r#"
trait TraitA {
    fn name(self) string
}

trait TraitB {
    fn name(self) string
}

class X impl TraitA, TraitB {
    n: string

    fn name(self) string {
        return self.n
    }
}

fn show_a(a: TraitA) {
    print(a.name())
}

fn show_b(b: TraitB) {
    print(b.name())
}

fn main() {
    let x = X { n: "hello" }
    show_a(x)
    show_b(x)
}
"#);
    assert_eq!(out, "hello\nhello\n");
}

#[test]
fn trait_default_with_nested_calls() {
    // Default method calls another method which calls another
    let out = compile_and_run_stdout(r#"
trait Chain {
    fn a(self) int {
        return self.b() + 1
    }

    fn b(self) int {
        return self.c() + 1
    }

    fn c(self) int {
        return 0
    }
}

class X impl Chain {
    val: int
}

fn run(ch: Chain) {
    print(ch.a())
}

fn main() {
    run(X { val: 0 })
}
"#);
    assert_eq!(out, "2\n");
}

#[test]
fn trait_override_middle_in_chain() {
    // Default method chain a->b->c, class overrides only b
    let out = compile_and_run_stdout(r#"
trait Chain {
    fn a(self) int {
        return self.b() + 1
    }

    fn b(self) int {
        return self.c() + 1
    }

    fn c(self) int {
        return 0
    }
}

class X impl Chain {
    val: int

    fn b(self) int {
        return 100
    }
}

fn run(ch: Chain) {
    print(ch.a())
}

fn main() {
    run(X { val: 0 })
}
"#);
    assert_eq!(out, "101\n");
}

#[test]
fn trait_dispatch_result_negated() {
    // Negate the result of a trait dispatch
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn main() {
    let v: Valued = X { n: 5 }
    print(-v.val())
}
"#);
    assert_eq!(out, "-5\n");
}

#[test]
fn trait_dispatch_result_compared() {
    // Compare two trait dispatch results
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class A impl Valued {
    n: int
    fn val(self) int { return self.n }
}

class B impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn main() {
    let a: Valued = A { n: 10 }
    let b: Valued = B { n: 5 }
    if a.val() > b.val() {
        print("a wins")
    } else {
        print("b wins")
    }
}
"#);
    assert_eq!(out, "a wins\n");
}

#[test]
fn trait_class_method_calls_non_trait_method() {
    // Class has both trait and non-trait methods; trait method calls non-trait one
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int

    fn helper(self) int {
        return self.n * 3
    }

    fn val(self) int {
        return self.helper()
    }
}

fn show(v: Valued) {
    print(v.val())
}

fn main() {
    show(X { n: 4 })
}
"#);
    assert_eq!(out, "12\n");
}

#[test]
fn trait_default_method_returns_empty_string() {
    // Default method returning empty string
    let out = compile_and_run_stdout(r#"
trait Named {
    fn name(self) string {
        return ""
    }
}

class Anon impl Named {
    val: int
}

class Named2 impl Named {
    n: string

    fn name(self) string {
        return self.n
    }
}

fn show(n: Named) {
    let s = n.name()
    if s.len() == 0 {
        print("anonymous")
    } else {
        print(s)
    }
}

fn main() {
    show(Anon { val: 0 })
    show(Named2 { n: "alice" })
}
"#);
    assert_eq!(out, "anonymous\nalice\n");
}

#[test]
fn trait_method_returns_zero() {
    // Ensure zero return value is correctly passed through dispatch
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class Zero impl Valued {
    padding: int
    fn val(self) int { return 0 }
}

fn show(v: Valued) {
    print(v.val())
}

fn main() {
    show(Zero { padding: 999 })
}
"#);
    assert_eq!(out, "0\n");
}

#[test]
fn trait_method_returns_max_int() {
    // Return a large integer through dispatch
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class Big impl Valued {
    n: int
    fn val(self) int { return 9999999 }
}

fn show(v: Valued) {
    print(v.val())
}

fn main() {
    show(Big { n: 0 })
}
"#);
    assert_eq!(out, "9999999\n");
}

#[test]
fn trait_two_classes_same_fields_different_behavior() {
    // Two classes with identical field layout but different method behavior
    let out = compile_and_run_stdout(r#"
trait Transform {
    fn transform(self) int
}

class Adder impl Transform {
    a: int
    b: int

    fn transform(self) int {
        return self.a + self.b
    }
}

class Multiplier impl Transform {
    a: int
    b: int

    fn transform(self) int {
        return self.a * self.b
    }
}

fn run(t: Transform) {
    print(t.transform())
}

fn main() {
    run(Adder { a: 3, b: 4 })
    run(Multiplier { a: 3, b: 4 })
}
"#);
    assert_eq!(out, "7\n12\n");
}

#[test]
fn trait_dispatch_with_unary_not() {
    // Trait method returns bool, negated with !
    let out = compile_and_run_stdout(r#"
trait Checker {
    fn is_valid(self) bool
}

class AlwaysTrue impl Checker {
    val: int
    fn is_valid(self) bool { return true }
}

fn main() {
    let c: Checker = AlwaysTrue { val: 0 }
    if !c.is_valid() {
        print(0)
    } else {
        print(1)
    }
}
"#);
    assert_eq!(out, "1\n");
}

#[test]
fn trait_three_traits_one_class_dispatch_all() {
    // Class implements 3 traits, each dispatched independently
    let out = compile_and_run_stdout(r#"
trait Alpha {
    fn a(self) int
}

trait Beta {
    fn b(self) int
}

trait Gamma {
    fn g(self) int
}

class Triple impl Alpha, Beta, Gamma {
    val: int

    fn a(self) int { return self.val + 1 }
    fn b(self) int { return self.val + 2 }
    fn g(self) int { return self.val + 3 }
}

fn use_alpha(a: Alpha) { print(a.a()) }
fn use_beta(b: Beta) { print(b.b()) }
fn use_gamma(g: Gamma) { print(g.g()) }

fn main() {
    let t = Triple { val: 10 }
    use_alpha(t)
    use_beta(t)
    use_gamma(t)
}
"#);
    assert_eq!(out, "11\n12\n13\n");
}

#[test]
fn trait_method_body_with_break() {
    // Trait method body uses break in a while loop
    let out = compile_and_run_stdout(r#"
trait Finder {
    fn find_first_gt(self, arr: [int], threshold: int) int
}

class LinearFinder impl Finder {
    default_val: int

    fn find_first_gt(self, arr: [int], threshold: int) int {
        let i = 0
        while i < arr.len() {
            if arr[i] > threshold {
                return arr[i]
            }
            i = i + 1
        }
        return self.default_val
    }
}

fn run(f: Finder) {
    let arr: [int] = [1, 3, 7, 2, 9]
    print(f.find_first_gt(arr, 5))
    print(f.find_first_gt(arr, 100))
}

fn main() {
    run(LinearFinder { default_val: -1 })
}
"#);
    assert_eq!(out, "7\n-1\n");
}

#[test]
fn fail_call_trait_method_on_wrong_trait() {
    // Method exists on trait A but called through trait B handle
    compile_should_fail(r#"
trait Alpha {
    fn a_method(self) int
}

trait Beta {
    fn b_method(self) int
}

class X impl Alpha, Beta {
    val: int
    fn a_method(self) int { return 1 }
    fn b_method(self) int { return 2 }
}

fn use_beta(b: Beta) {
    print(b.a_method())
}

fn main() {
}
"#);
}

// ===== Batch 10: Recursion through traits, trait + array iteration, deeper dispatch patterns =====

#[test]
fn trait_recursive_dispatch() {
    // Trait method calls itself recursively through dispatch
    let out = compile_and_run_stdout(r#"
trait Counter {
    fn count_down(self, n: int)
}

class Printer impl Counter {
    val: int

    fn count_down(self, n: int) {
        if n <= 0 {
            return
        }
        print(n)
        self.count_down(n - 1)
    }
}

fn run(c: Counter) {
    c.count_down(3)
}

fn main() {
    run(Printer { val: 0 })
}
"#);
    assert_eq!(out, "3\n2\n1\n");
}

#[test]
fn trait_dispatch_in_for_loop() {
    // Trait handle used inside a for loop over range
    let out = compile_and_run_stdout(r#"
trait Multiplier {
    fn mul(self, x: int) int
}

class Doubler impl Multiplier {
    val: int
    fn mul(self, x: int) int { return x * 2 }
}

fn run(m: Multiplier) {
    for i in 0..5 {
        print(m.mul(i))
    }
}

fn main() {
    run(Doubler { val: 0 })
}
"#);
    assert_eq!(out, "0\n2\n4\n6\n8\n");
}

#[test]
fn trait_method_creates_and_returns_string() {
    // Trait method builds a string from fields and returns it
    let out = compile_and_run_stdout(r#"
trait Describable {
    fn describe(self) string
}

class Person impl Describable {
    name: string
    age: int

    fn describe(self) string {
        return "{self.name} is {self.age}"
    }
}

fn show(d: Describable) {
    print(d.describe())
}

fn main() {
    show(Person { name: "Alice", age: 30 })
}
"#);
    assert_eq!(out, "Alice is 30\n");
}

#[test]
fn trait_dispatch_result_in_arithmetic_chain() {
    // Use dispatch result in a chain of arithmetic operations
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn main() {
    let v: Valued = X { n: 5 }
    let result = v.val() * 3 + v.val() - 2
    print(result)
}
"#);
    assert_eq!(out, "18\n");
}

#[test]
fn trait_method_that_calls_free_function() {
    // Trait method implementation calls a free function
    let out = compile_and_run_stdout(r#"
fn square(x: int) int {
    return x * x
}

trait HasArea {
    fn area(self) int
}

class Square impl HasArea {
    side: int

    fn area(self) int {
        return square(self.side)
    }
}

fn show(h: HasArea) {
    print(h.area())
}

fn main() {
    show(Square { side: 5 })
}
"#);
    assert_eq!(out, "25\n");
}

#[test]
fn trait_dispatch_in_nested_function_calls() {
    // Trait dispatch result passed as arg to another function call
    let out = compile_and_run_stdout(r#"
fn double(x: int) int {
    return x * 2
}

fn add_one(x: int) int {
    return x + 1
}

trait Source {
    fn get(self) int
}

class Num impl Source {
    n: int
    fn get(self) int { return self.n }
}

fn main() {
    let s: Source = Num { n: 5 }
    print(double(add_one(s.get())))
}
"#);
    assert_eq!(out, "12\n");
}

#[test]
fn trait_multiple_methods_vtable_order_matters() {
    // Trait with 4 methods — verify each resolves to the correct implementation
    let out = compile_and_run_stdout(r#"
trait FourMethods {
    fn m1(self) int
    fn m2(self) int
    fn m3(self) int
    fn m4(self) int
}

class Impl impl FourMethods {
    val: int

    fn m1(self) int { return 10 }
    fn m2(self) int { return 20 }
    fn m3(self) int { return 30 }
    fn m4(self) int { return 40 }
}

fn run(f: FourMethods) {
    print(f.m1())
    print(f.m2())
    print(f.m3())
    print(f.m4())
}

fn main() {
    run(Impl { val: 0 })
}
"#);
    assert_eq!(out, "10\n20\n30\n40\n");
}

#[test]
fn trait_method_with_three_params() {
    // Trait method with 3 non-self parameters
    let out = compile_and_run_stdout(r#"
trait Calculator {
    fn calc(self, a: int, b: int, c: int) int
}

class Weighted impl Calculator {
    weight: int

    fn calc(self, a: int, b: int, c: int) int {
        return a * self.weight + b * 2 + c
    }
}

fn run(calc: Calculator) {
    print(calc.calc(1, 2, 3))
}

fn main() {
    run(Weighted { weight: 10 })
}
"#);
    assert_eq!(out, "17\n");
}

#[test]
fn trait_five_classes_same_trait_sequential_dispatch() {
    // Five different classes dispatched sequentially through same trait type
    let out = compile_and_run_stdout(r#"
trait Id {
    fn id(self) int
}

class A impl Id { val: int  fn id(self) int { return 1 } }
class B impl Id { val: int  fn id(self) int { return 2 } }
class C impl Id { val: int  fn id(self) int { return 3 } }
class D impl Id { val: int  fn id(self) int { return 4 } }
class E impl Id { val: int  fn id(self) int { return 5 } }

fn show(x: Id) {
    print(x.id())
}

fn main() {
    show(A { val: 0 })
    show(B { val: 0 })
    show(C { val: 0 })
    show(D { val: 0 })
    show(E { val: 0 })
}
"#);
    assert_eq!(out, "1\n2\n3\n4\n5\n");
}

#[test]
fn trait_method_string_concatenation_through_dispatch() {
    // Build up string through repeated dispatch calls
    let out = compile_and_run_stdout(r#"
trait Part {
    fn part(self) string
}

class Prefix impl Part {
    val: string
    fn part(self) string { return self.val }
}

class Suffix impl Part {
    val: string
    fn part(self) string { return self.val }
}

fn combine(a: Part, b: Part) {
    print(a.part() + b.part())
}

fn main() {
    combine(Prefix { val: "hello " }, Suffix { val: "world" })
}
"#);
    assert_eq!(out, "hello world\n");
}

#[test]
fn trait_default_method_calls_required_method_twice() {
    // Default method calls the required method twice
    let out = compile_and_run_stdout(r#"
trait Doubler {
    fn val(self) int

    fn doubled(self) int {
        return self.val() + self.val()
    }
}

class X impl Doubler {
    n: int
    fn val(self) int { return self.n }
}

fn show(d: Doubler) {
    print(d.doubled())
}

fn main() {
    show(X { n: 7 })
}
"#);
    assert_eq!(out, "14\n");
}

#[test]
fn trait_dispatch_result_as_while_bound() {
    // Trait dispatch result used as upper bound in while loop
    let out = compile_and_run_stdout(r#"
trait Limiter {
    fn limit(self) int
}

class Fixed impl Limiter {
    max: int
    fn limit(self) int { return self.max }
}

fn count_to(l: Limiter) {
    let i = 0
    while i < l.limit() {
        i = i + 1
    }
    print(i)
}

fn main() {
    count_to(Fixed { max: 5 })
}
"#);
    assert_eq!(out, "5\n");
}

#[test]
fn trait_dispatch_inside_string_interpolation() {
    // Trait dispatch result used in string interpolation
    let out = compile_and_run_stdout(r#"
trait Named {
    fn name(self) string
}

class User impl Named {
    n: string
    fn name(self) string { return self.n }
}

fn greet(n: Named) {
    print("hello {n.name()}")
}

fn main() {
    greet(User { n: "alice" })
}
"#);
    assert_eq!(out, "hello alice\n");
}

#[test]
fn trait_dispatch_int_in_string_interpolation() {
    // Trait dispatch returning int, used in string interpolation via .to_string()
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn show(v: Valued) {
    let n = v.val()
    print("value is {n}")
}

fn main() {
    show(X { n: 42 })
}
"#);
    assert_eq!(out, "value is 42\n");
}

#[test]
fn trait_method_with_bool_param_two_fields() {
    // Trait method takes bool parameter, class has separate on/off fields
    let out = compile_and_run_stdout(r#"
trait Toggler {
    fn toggle(self, on: bool) int
}

class Switch impl Toggler {
    on_val: int
    off_val: int

    fn toggle(self, on: bool) int {
        if on {
            return self.on_val
        }
        return self.off_val
    }
}

fn run(t: Toggler) {
    print(t.toggle(true))
    print(t.toggle(false))
}

fn main() {
    run(Switch { on_val: 100, off_val: 0 })
}
"#);
    assert_eq!(out, "100\n0\n");
}

#[test]
fn trait_method_returns_negative_number() {
    // Verify negative return values pass correctly through dispatch
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class Neg impl Valued {
    n: int
    fn val(self) int { return -self.n }
}

fn show(v: Valued) {
    print(v.val())
}

fn main() {
    show(Neg { n: 42 })
}
"#);
    assert_eq!(out, "-42\n");
}

#[test]
fn fail_trait_method_wrong_return_type_string_vs_int() {
    // Class returns string but trait requires int
    compile_should_fail(r#"
trait Valued {
    fn val(self) int
}

class Bad impl Valued {
    n: int

    fn val(self) string {
        return "wrong"
    }
}

fn main() {
}
"#);
}

#[test]
fn fail_trait_missing_one_of_two_methods() {
    // Trait requires two methods, class only implements one
    compile_should_fail(r#"
trait TwoMethods {
    fn first(self) int
    fn second(self) int
}

class Half impl TwoMethods {
    val: int

    fn first(self) int {
        return self.val
    }
}

fn main() {
}
"#);
}

#[test]
fn fail_undeclared_trait_in_impl() {
    // Class implements a trait that doesn't exist
    compile_should_fail_with(r#"
class X impl NonExistent {
    val: int
    fn foo(self) int { return 1 }
}

fn main() {
}
"#, "unknown trait");
}

#[test]
fn trait_dispatch_many_small_classes() {
    // 6 small classes, each 1 field 1 method, all dispatched
    let out = compile_and_run_stdout(r#"
trait Num {
    fn num(self) int
}

class N1 impl Num { x: int  fn num(self) int { return 1 } }
class N2 impl Num { x: int  fn num(self) int { return 2 } }
class N3 impl Num { x: int  fn num(self) int { return 3 } }
class N4 impl Num { x: int  fn num(self) int { return 4 } }
class N5 impl Num { x: int  fn num(self) int { return 5 } }
class N6 impl Num { x: int  fn num(self) int { return 6 } }

fn sum_all(a: Num, b: Num, c: Num, d: Num, e: Num, f: Num) {
    print(a.num() + b.num() + c.num() + d.num() + e.num() + f.num())
}

fn main() {
    sum_all(N1{x:0}, N2{x:0}, N3{x:0}, N4{x:0}, N5{x:0}, N6{x:0})
}
"#);
    assert_eq!(out, "21\n");
}

// ===== Batch 11: Trait + class field interactions, default method edge cases, more negatives =====

#[test]
fn trait_method_accesses_multiple_fields() {
    // Trait method implementation reads 4 fields
    let out = compile_and_run_stdout(r#"
trait Summary {
    fn sum(self) int
}

class FourFields impl Summary {
    a: int
    b: int
    c: int
    d: int

    fn sum(self) int {
        return self.a + self.b + self.c + self.d
    }
}

fn show(s: Summary) {
    print(s.sum())
}

fn main() {
    show(FourFields { a: 1, b: 2, c: 3, d: 4 })
}
"#);
    assert_eq!(out, "10\n");
}

#[test]
fn trait_dispatch_with_class_having_string_fields() {
    // Class with 2 string fields dispatched through trait
    let out = compile_and_run_stdout(r#"
trait Concat {
    fn combined(self) string
}

class TwoStrings impl Concat {
    first: string
    second: string

    fn combined(self) string {
        return self.first + " " + self.second
    }
}

fn show(c: Concat) {
    print(c.combined())
}

fn main() {
    show(TwoStrings { first: "hello", second: "world" })
}
"#);
    assert_eq!(out, "hello world\n");
}

#[test]
fn trait_default_that_returns_constant() {
    // Default method returns a constant, class doesn't override
    let out = compile_and_run_stdout(r#"
trait Versioned {
    fn version(self) int {
        return 1
    }
}

class App impl Versioned {
    name: string
}

fn show(v: Versioned) {
    print(v.version())
}

fn main() {
    show(App { name: "test" })
}
"#);
    assert_eq!(out, "1\n");
}

#[test]
fn trait_default_that_returns_string_constant() {
    // Default method returns a string constant
    let out = compile_and_run_stdout(r#"
trait Describable {
    fn describe(self) string {
        return "unknown"
    }
}

class Thing impl Describable {
    val: int
}

fn show(d: Describable) {
    print(d.describe())
}

fn main() {
    show(Thing { val: 0 })
}
"#);
    assert_eq!(out, "unknown\n");
}

#[test]
fn trait_class_has_no_extra_methods() {
    // Class has ONLY the trait methods, nothing else
    let out = compile_and_run_stdout(r#"
trait Simple {
    fn val(self) int
}

class Bare impl Simple {
    n: int
    fn val(self) int { return self.n }
}

fn show(s: Simple) {
    print(s.val())
}

fn main() {
    show(Bare { n: 77 })
}
"#);
    assert_eq!(out, "77\n");
}

#[test]
fn trait_class_has_many_extra_methods() {
    // Class has trait method plus 3 non-trait methods, only trait method dispatched
    let out = compile_and_run_stdout(r#"
trait HasVal {
    fn val(self) int
}

class Rich impl HasVal {
    n: int

    fn val(self) int { return self.n }
    fn doubled(self) int { return self.n * 2 }
    fn tripled(self) int { return self.n * 3 }
    fn as_string(self) string { return "{self.n}" }
}

fn show(h: HasVal) {
    print(h.val())
}

fn main() {
    let r = Rich { n: 5 }
    show(r)
    print(r.doubled())
    print(r.tripled())
}
"#);
    assert_eq!(out, "5\n10\n15\n");
}

#[test]
fn trait_method_returns_bool() {
    // Trait method returning bool, dispatched and used in condition
    let out = compile_and_run_stdout(r#"
trait Predicate {
    fn check_val(self, x: int) bool
}

class GreaterThanZero impl Predicate {
    val: int

    fn check_val(self, x: int) bool {
        return x > 0
    }
}

fn check(p: Predicate, x: int) {
    if p.check_val(x) {
        print("yes")
    } else {
        print("no")
    }
}

fn main() {
    let p = GreaterThanZero { val: 0 }
    check(p, 5)
    check(p, -3)
    check(p, 0)
}
"#);
    assert_eq!(out, "yes\nno\nno\n");
}

#[test]
fn trait_two_defaults_one_required() {
    // Trait with 2 default methods and 1 required
    let out = compile_and_run_stdout(r#"
trait WithDefaults {
    fn required(self) int

    fn default_one(self) int {
        return self.required() + 10
    }

    fn default_two(self) int {
        return self.required() * 2
    }
}

class Impl impl WithDefaults {
    n: int

    fn required(self) int {
        return self.n
    }
}

fn run(w: WithDefaults) {
    print(w.required())
    print(w.default_one())
    print(w.default_two())
}

fn main() {
    run(Impl { n: 5 })
}
"#);
    assert_eq!(out, "5\n15\n10\n");
}

#[test]
fn trait_override_one_default_keep_other() {
    // Override one default but keep the other
    let out = compile_and_run_stdout(r#"
trait WithDefaults {
    fn required(self) int

    fn default_one(self) int {
        return self.required() + 10
    }

    fn default_two(self) int {
        return self.required() * 2
    }
}

class Impl impl WithDefaults {
    n: int

    fn required(self) int {
        return self.n
    }

    fn default_one(self) int {
        return 999
    }
}

fn run(w: WithDefaults) {
    print(w.default_one())
    print(w.default_two())
}

fn main() {
    run(Impl { n: 5 })
}
"#);
    assert_eq!(out, "999\n10\n");
}

#[test]
fn trait_dispatch_deeply_nested_call() {
    // 4-level deep function call chain with trait dispatch at each level
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn level3(v: Valued) int {
    return v.val()
}

fn level2(v: Valued) int {
    return level3(v) + 1
}

fn level1(v: Valued) int {
    return level2(v) + 1
}

fn main() {
    let x = X { n: 10 }
    print(level1(x))
}
"#);
    assert_eq!(out, "12\n");
}

#[test]
fn trait_handle_passed_to_two_functions_sequentially() {
    // Same trait handle passed to two different functions
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn add_ten(v: Valued) int {
    return v.val() + 10
}

fn multiply_two(v: Valued) int {
    return v.val() * 2
}

fn main() {
    let x = X { n: 5 }
    print(add_ten(x))
    print(multiply_two(x))
}
"#);
    assert_eq!(out, "15\n10\n");
}

#[test]
fn fail_class_impl_trait_with_extra_param() {
    // Class method has extra parameter vs trait signature
    compile_should_fail(r#"
trait Simple {
    fn val(self) int
}

class Bad impl Simple {
    n: int

    fn val(self, extra: int) int {
        return self.n + extra
    }
}

fn main() {
}
"#);
}

#[test]
fn fail_class_impl_trait_missing_param() {
    // Class method has fewer parameters than trait signature
    compile_should_fail(r#"
trait TakesParam {
    fn compute(self, x: int) int
}

class Bad impl TakesParam {
    n: int

    fn compute(self) int {
        return self.n
    }
}

fn main() {
}
"#);
}

#[test]
fn fail_assign_concrete_to_wrong_trait_var() {
    // Class implements TraitA, but assigned to TraitB variable
    compile_should_fail(r#"
trait TraitA {
    fn a(self) int
}

trait TraitB {
    fn b(self) int
}

class X impl TraitA {
    val: int
    fn a(self) int { return 1 }
}

fn main() {
    let b: TraitB = X { val: 0 }
}
"#);
}

#[test]
fn trait_method_with_array_param_and_base() {
    // Trait method takes an array parameter and adds to base
    let out = compile_and_run_stdout(r#"
trait Summable {
    fn sum(self, arr: [int]) int
}

class Adder impl Summable {
    base: int

    fn sum(self, arr: [int]) int {
        let total = self.base
        let i = 0
        while i < arr.len() {
            total = total + arr[i]
            i = i + 1
        }
        return total
    }
}

fn run(s: Summable) {
    let nums: [int] = [1, 2, 3, 4, 5]
    print(s.sum(nums))
}

fn main() {
    run(Adder { base: 100 })
}
"#);
    assert_eq!(out, "115\n");
}

#[test]
fn trait_method_returns_array() {
    // Trait method returns an array
    let out = compile_and_run_stdout(r#"
trait Generator {
    fn generate(self, count: int) [int]
}

class Counter impl Generator {
    start: int

    fn generate(self, count: int) [int] {
        let result: [int] = []
        let i = 0
        while i < count {
            result.push(self.start + i)
            i = i + 1
        }
        return result
    }
}

fn show(g: Generator) {
    let arr = g.generate(3)
    let i = 0
    while i < arr.len() {
        print(arr[i])
        i = i + 1
    }
}

fn main() {
    show(Counter { start: 10 })
}
"#);
    assert_eq!(out, "10\n11\n12\n");
}

#[test]
fn trait_method_appends_to_array() {
    // Trait method receives array and adds to it (heap shared)
    let out = compile_and_run_stdout(r#"
trait Appender {
    fn append(self, arr: [int])
}

class DoubleAppender impl Appender {
    val: int

    fn append(self, arr: [int]) {
        arr.push(self.val)
        arr.push(self.val * 2)
    }
}

fn run(a: Appender) {
    let arr: [int] = [1]
    a.append(arr)
    let i = 0
    while i < arr.len() {
        print(arr[i])
        i = i + 1
    }
}

fn main() {
    run(DoubleAppender { val: 5 })
}
"#);
    assert_eq!(out, "1\n5\n10\n");
}

#[test]
fn trait_dispatch_result_stored_in_array() {
    // Store multiple trait dispatch results in an array
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class A impl Valued { n: int  fn val(self) int { return self.n } }
class B impl Valued { n: int  fn val(self) int { return self.n * 2 } }

fn main() {
    let a: Valued = A { n: 3 }
    let b: Valued = B { n: 4 }
    let results: [int] = []
    results.push(a.val())
    results.push(b.val())
    results.push(a.val() + b.val())
    let i = 0
    while i < results.len() {
        print(results[i])
        i = i + 1
    }
}
"#);
    assert_eq!(out, "3\n8\n11\n");
}

#[test]
fn trait_method_with_string_param_and_return() {
    // Trait method takes string, returns string
    let out = compile_and_run_stdout(r#"
trait Transformer {
    fn transform(self, input: string) string
}

class Wrapper impl Transformer {
    prefix: string
    suffix: string

    fn transform(self, input: string) string {
        return self.prefix + input + self.suffix
    }
}

fn run(t: Transformer) {
    print(t.transform("hello"))
}

fn main() {
    run(Wrapper { prefix: "[", suffix: "]" })
}
"#);
    assert_eq!(out, "[hello]\n");
}

#[test]
fn trait_default_method_with_string_return() {
    // Default method returns a formatted string
    let out = compile_and_run_stdout(r#"
trait Greeter {
    fn name(self) string

    fn greet(self) string {
        return "hello " + self.name()
    }
}

class Person impl Greeter {
    n: string
    fn name(self) string { return self.n }
}

fn show(g: Greeter) {
    print(g.greet())
}

fn main() {
    show(Person { n: "alice" })
}
"#);
    assert_eq!(out, "hello alice\n");
}
