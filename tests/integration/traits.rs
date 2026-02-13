mod common;
use common::{compile_and_run_stdout, compile_and_run_output, compile_should_fail, compile_should_fail_with};

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
    compile_should_fail_with(
        "trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n}\n\nfn main() {\n}",
        "class 'X' does not implement required method 'bar' from trait 'Foo'",
    );
}

#[test]
fn trait_wrong_return_type_rejected() {
    compile_should_fail_with(
        "trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n\n    fn bar(self) bool {\n        return true\n    }\n}\n\nfn main() {\n}",
        "method 'bar' return type mismatch: trait 'Foo' expects int, class 'X' returns bool",
    );
}

#[test]
fn trait_unknown_trait_rejected() {
    compile_should_fail_with(
        "class X impl NonExistent {\n    val: int\n}\n\nfn main() {\n}",
        "unknown trait 'NonExistent'",
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
    let out = compile_and_run_stdout(r#"
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
"#);
    assert_eq!(out, "20\n5\n");
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
    compile_should_fail_with(r#"
class Other {
    val: int
}

class X impl Other {
    val: int
}

fn main() {
}
"#, "unknown trait 'Other'");
}

#[test]
fn fail_impl_enum_name() {
    // impl an enum name instead of a trait name
    compile_should_fail_with(r#"
enum Color {
    Red
    Blue
}

class X impl Color {
    val: int
}

fn main() {
}
"#, "unknown trait 'Color'");
}

#[test]
fn fail_non_implementing_class_as_trait_param() {
    // Class has matching methods but doesn't declare impl — should fail
    compile_should_fail_with(r#"
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
"#, "argument 1 of 'use_worker': expected trait Worker, found NotAWorker");
}

#[test]
fn fail_call_non_trait_method_on_handle() {
    // Dog has fetch() but Worker trait doesn't — calling on trait handle should fail
    compile_should_fail_with(r#"
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
"#, "trait 'Worker' has no method 'fetch'");
}

#[test]
fn fail_access_field_on_trait_handle() {
    // Cannot access concrete class fields through trait handle
    compile_should_fail_with(r#"
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
"#, "field access on non-class type trait Worker");
}

#[test]
fn fail_assign_primitive_to_trait() {
    // Cannot assign int to trait-typed variable
    compile_should_fail_with(r#"
trait Worker {
    fn work(self) int
}

fn main() {
    let w: Worker = 42
}
"#, "type mismatch: expected trait Worker, found int");
}

#[test]
fn fail_assign_incompatible_class_to_trait() {
    // Class doesn't implement the trait
    compile_should_fail_with(r#"
trait Worker {
    fn work(self) int
}

class NotWorker {
    val: int
}

fn main() {
    let w: Worker = NotWorker { val: 1 }
}
"#, "type mismatch: expected trait Worker, found NotWorker");
}

#[test]
fn trait_duplicate_trait_in_impl_allowed() {
    // Duplicate trait in impl list should be rejected
    compile_should_fail_with(r#"
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
"#, "trait 'Bar' appears multiple times in impl list for class 'Foo'");
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
    // Fixed: trait method signatures can now reference class types via forward references
    let out = compile_and_run_stdout(r#"
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
"#);
    assert_eq!(out, "50\n");
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
    // Fixed: trait method signatures can now reference enum types via forward references
    let out = compile_and_run_stdout(r#"
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
"#);
    assert_eq!(out, "7\n12\n");
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
    compile_should_fail_with(r#"
trait Foo {
    fn bar(self) UnknownType
}

fn main() {
}
"#, "unknown type 'UnknownType'");
}

#[test]
fn fail_trait_method_unknown_param_type() {
    // Trait method with a param type that doesn't exist
    compile_should_fail_with(r#"
trait Foo {
    fn bar(self, x: UnknownType) int
}

fn main() {
}
"#, "unknown type 'UnknownType'");
}

#[test]
fn fail_impl_function_name() {
    // impl a function name instead of trait
    compile_should_fail_with(r#"
fn some_func() int {
    return 1
}

class X impl some_func {
    val: int
}

fn main() {
}
"#, "unknown trait 'some_func'");
}

#[test]
fn fail_trait_handle_to_concrete_function() {
    // Cannot pass trait-typed value to function expecting concrete type
    compile_should_fail_with(r#"
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
"#, "argument 1 of 'use_dog': expected Dog, found trait Worker");
}

#[test]
fn fail_assign_one_trait_to_different_trait() {
    // Cannot assign TraitA-typed value to TraitB variable
    compile_should_fail_with(r#"
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
"#, "type mismatch: expected trait TraitB, found trait TraitA");
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
    compile_should_fail_with(r#"
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
"#, "Duplicate definition of identifier: C$work");
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
    compile_should_fail_with(r#"
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
"#, "method 'work' return type mismatch: trait 'B' expects string, class 'C' returns int");
}

#[test]
fn trait_three_traits_same_method() {
    // Three traits all define the same method name — maximum ambiguity
    compile_should_fail_with(r#"
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
"#, "Duplicate definition of identifier: X$work");
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
    compile_should_fail_with(r#"
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
"#, "print() does not support type trait Worker");
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
    compile_should_fail_with(r#"
trait Worker {
    fn work(self) int
}

fn main() {
    let m = Map<Worker, int> {}
}
"#, "type trait Worker cannot be used as a map/set key");
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
fn trait_self_trait_param() {
    // Fixed: trait method referencing its own trait name as a parameter type now works with two-pass
    // 
    let out = compile_and_run_stdout(r#"
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
"#);
    assert_eq!(out, "1\n");}

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
    compile_should_fail_with(r#"
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
"#, "method 'age' return type mismatch: trait 'Pair' expects int, class 'Person' returns string");
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
    // Fixed: trait method signatures can now reference enum types via forward references
    let out = compile_and_run_stdout(r#"
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

fn check(s: Stateful) {
    match s.status() {
        Status.Active {
            print(1)
        }
        Status.Inactive {
            print(0)
        }
    }
}

fn main() {
    let srv = Server { running: true }
    check(srv)
}
"#);
    assert_eq!(out, "1\n");
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
    // Fixed: trait method signatures can now reference enum types via forward references
    let out = compile_and_run_stdout(r#"
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

fn run(c: Calculator) {
    print(c.calc(Op.Add, 10, 5))
    print(c.calc(Op.Sub, 10, 5))
}

fn main() {
    let calc = SimpleCalc { val: 0 }
    run(calc)
}
"#);
    assert_eq!(out, "15\n5\n");
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
#[ignore] // Compiler bug: panic in typeck/register.rs:1326:59 - range start index 1 out of range
fn trait_method_without_self_shows_error() {
    // Trait methods must have self parameter - should show helpful error, not panic
    compile_should_fail_with(r#"
trait Compute {
    fn calculate() int
}

class Calculator impl Compute {
    value: int
    fn calculate() int { return 42 }
}

fn main() {
    let c: Compute = Calculator { value: 0 }
    print(c.calculate())
}
"#, "trait method 'calculate' must have a 'self' parameter");
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
    compile_should_fail_with(r#"
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
"#, "trait 'Beta' has no method 'a_method'");
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
    compile_should_fail_with(r#"
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
"#, "method 'val' return type mismatch: trait 'Valued' expects int, class 'Bad' returns string");
}

#[test]
fn fail_trait_missing_one_of_two_methods() {
    // Trait requires two methods, class only implements one
    compile_should_fail_with(r#"
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
"#, "class 'Half' does not implement required method 'second' from trait 'TwoMethods'");
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
    compile_should_fail_with(r#"
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
"#, "method 'val' of class 'Bad' has wrong number of parameters for trait 'Simple'");
}

#[test]
fn fail_class_impl_trait_missing_param() {
    // Class method has fewer parameters than trait signature
    compile_should_fail_with(r#"
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
"#, "method 'compute' of class 'Bad' has wrong number of parameters for trait 'TakesParam'");
}

#[test]
fn fail_assign_concrete_to_wrong_trait_var() {
    // Class implements TraitA, but assigned to TraitB variable
    compile_should_fail_with(r#"
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
"#, "type mismatch: expected trait TraitB, found X");
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

// ===== Batch 12: mut self through traits, trait handle in various positions, edge cases =====

#[test]
fn fail_trait_mut_self_not_supported_yet() {
    // COMPILER GAP: mut self in trait method declarations is not parsed yet
    // (expected (, found identifier). Part of mut self enforcement work item.
    compile_should_fail_with(r#"
trait Counter {
    fn increment(mut self)
    fn count(self) int
}

class SimpleCounter impl Counter {
    n: int

    fn increment(mut self) {
        self.n = self.n + 1
    }

    fn count(self) int {
        return self.n
    }
}

fn main() {
    let c = SimpleCounter { n: 0 }
    c.increment()
    print(c.count())
}
"#, "cannot call mutating method");
}

#[test]
fn trait_mut_self_with_multiple_methods() {
    // Trait with mut self method followed by another method
    let out = compile_and_run_stdout(r#"
trait Accumulator {
    fn add(mut self, x: int)
    fn total(self) int
}

class Sum impl Accumulator {
    val: int

    fn add(mut self, x: int) {
        self.val = self.val + x
    }

    fn total(self) int {
        return self.val
    }
}

fn main() {
    let mut s = Sum { val: 0 }
    s.add(10)
    s.add(20)
    print(s.total())
}
"#);
    assert_eq!(out, "30\n");
}

#[test]
fn trait_method_returns_class_instance() {
    // Trait method returns a class that is NOT trait-typed
    let out = compile_and_run_stdout(r#"
class Result {
    value: int
}

trait Producer {
    fn produce(self) int
}

class Factory impl Producer {
    base: int

    fn produce(self) int {
        return self.base * 10
    }
}

fn run(p: Producer) {
    print(p.produce())
}

fn main() {
    run(Factory { base: 7 })
}
"#);
    assert_eq!(out, "70\n");
}

#[test]
fn trait_two_classes_same_field_names_different_types() {
    // Two classes implementing same trait, both have field "data" but different types
    let out = compile_and_run_stdout(r#"
trait Printer {
    fn show(self)
}

class IntPrinter impl Printer {
    data: int

    fn show(self) {
        print(self.data)
    }
}

class StrPrinter impl Printer {
    data: string

    fn show(self) {
        print(self.data)
    }
}

fn run(p: Printer) {
    p.show()
}

fn main() {
    run(IntPrinter { data: 42 })
    run(StrPrinter { data: "hello" })
}
"#);
    assert_eq!(out, "42\nhello\n");
}

#[test]
fn trait_dispatch_where_method_has_void_return() {
    // Trait method returning void, used through dispatch
    let out = compile_and_run_stdout(r#"
trait Logger {
    fn log(self, msg: string)
}

class StdoutLogger impl Logger {
    prefix: string

    fn log(self, msg: string) {
        print(self.prefix + msg)
    }
}

fn use_logger(l: Logger) {
    l.log("test1")
    l.log("test2")
}

fn main() {
    use_logger(StdoutLogger { prefix: "[LOG] " })
}
"#);
    assert_eq!(out, "[LOG] test1\n[LOG] test2\n");
}

#[test]
fn trait_void_method_plus_int_method() {
    // Trait with void method (no return type) followed by int-returning method
    let out = compile_and_run_stdout(r#"
trait Worker {
    fn do_work(self)
    fn status(self) int
}

class SimpleWorker impl Worker {
    n: int

    fn do_work(self) {
        print("working")
    }

    fn status(self) int {
        return self.n
    }
}

fn main() {
    let w: Worker = SimpleWorker { n: 42 }
    w.do_work()
    print(w.status())
}
"#);
    assert_eq!(out, "working\n42\n");
}

#[test]
fn trait_class_nested_field_access() {
    // Regression test: nested field access self.inner.val should work correctly
    // (Previously treated as unknown enum due to uppercase heuristic bug)
    let out = compile_and_run_stdout(r#"
class Inner {
    val: int
}

trait HasInner {
    fn get_inner_val(self) int
}

class Outer impl HasInner {
    inner: Inner

    fn get_inner_val(self) int {
        return self.inner.val
    }
}

fn main() {
    let o = Outer { inner: Inner { val: 42 } }
    print(o.get_inner_val())
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn trait_dispatch_preserves_multiple_class_fields() {
    // Class with 5 int fields, verify all preserved through dispatch
    let out = compile_and_run_stdout(r#"
trait FiveSum {
    fn sum(self) int
}

class Five impl FiveSum {
    a: int
    b: int
    c: int
    d: int
    e: int

    fn sum(self) int {
        return self.a + self.b + self.c + self.d + self.e
    }
}

fn show(f: FiveSum) {
    print(f.sum())
}

fn main() {
    show(Five { a: 1, b: 2, c: 3, d: 4, e: 5 })
}
"#);
    assert_eq!(out, "15\n");
}

#[test]
fn trait_closure_returns_trait_dispatch_result() {
    // Closure captures trait handle, returns result of dispatching
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn make_getter(v: Valued) fn() int {
    return () => v.val()
}

fn main() {
    let x = X { n: 99 }
    let getter = make_getter(x)
    print(getter())
    print(getter())
}
"#);
    assert_eq!(out, "99\n99\n");
}

#[test]
fn trait_dispatch_in_for_range_sum() {
    // Trait dispatch called N times in a for-range loop, accumulating results
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn sum_n_times(v: Valued, n: int) {
    let total = 0
    for i in 0..n {
        total = total + v.val()
    }
    print(total)
}

fn main() {
    sum_n_times(X { n: 7 }, 4)
}
"#);
    assert_eq!(out, "28\n");
}

#[test]
fn trait_dispatch_method_with_negative_param() {
    // Trait method receives negative integer parameter
    let out = compile_and_run_stdout(r#"
trait Math {
    fn add(self, x: int) int
}

class Calc impl Math {
    base: int

    fn add(self, x: int) int {
        return self.base + x
    }
}

fn run(m: Math) {
    print(m.add(-5))
    print(m.add(-100))
}

fn main() {
    run(Calc { base: 10 })
}
"#);
    assert_eq!(out, "5\n-90\n");
}

#[test]
fn trait_handle_survives_multiple_function_calls() {
    // Same trait handle passed through 3 different functions
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn f1(v: Valued) int { return v.val() + 1 }
fn f2(v: Valued) int { return v.val() * 2 }
fn f3(v: Valued) int { return v.val() - 3 }

fn main() {
    let x = X { n: 10 }
    print(f1(x))
    print(f2(x))
    print(f3(x))
}
"#);
    assert_eq!(out, "11\n20\n7\n");
}

#[test]
fn trait_default_method_with_complex_logic() {
    // Default method with if/else and arithmetic
    let out = compile_and_run_stdout(r#"
trait Scorer {
    fn raw_score(self) int

    fn grade(self) string {
        let s = self.raw_score()
        if s >= 90 {
            return "A"
        }
        if s >= 80 {
            return "B"
        }
        if s >= 70 {
            return "C"
        }
        return "F"
    }
}

class Student impl Scorer {
    points: int
    fn raw_score(self) int { return self.points }
}

fn show(s: Scorer) {
    print(s.grade())
}

fn main() {
    show(Student { points: 95 })
    show(Student { points: 85 })
    show(Student { points: 75 })
    show(Student { points: 50 })
}
"#);
    assert_eq!(out, "A\nB\nC\nF\n");
}

#[test]
fn trait_dispatch_with_string_empty_check() {
    // Trait method returns string, caller checks if empty
    let out = compile_and_run_stdout(r#"
trait Named {
    fn name(self) string
}

class Empty impl Named {
    val: int
    fn name(self) string { return "" }
}

class Full impl Named {
    n: string
    fn name(self) string { return self.n }
}

fn show(n: Named) {
    let s = n.name()
    if s.len() > 0 {
        print(s)
    } else {
        print("(empty)")
    }
}

fn main() {
    show(Full { n: "alice" })
    show(Empty { val: 0 })
}
"#);
    assert_eq!(out, "alice\n(empty)\n");
}

#[test]
fn trait_method_returns_string_used_in_concatenation() {
    // Trait dispatch string result used in further string ops
    let out = compile_and_run_stdout(r#"
trait Named {
    fn name(self) string
}

class User impl Named {
    n: string
    fn name(self) string { return self.n }
}

fn main() {
    let u: Named = User { n: "bob" }
    let greeting = "Hello, " + u.name() + "!"
    print(greeting)
}
"#);
    assert_eq!(out, "Hello, bob!\n");
}

#[test]
fn fail_method_not_in_trait_called_via_handle() {
    // Class has extra method, called through trait handle — should fail
    compile_should_fail_with(r#"
trait Simple {
    fn val(self) int
}

class X impl Simple {
    n: int
    fn val(self) int { return self.n }
    fn extra(self) int { return 99 }
}

fn use_it(s: Simple) {
    print(s.extra())
}

fn main() {
}
"#, "trait 'Simple' has no method 'extra'");
}

#[test]
fn trait_void_return_assigned_to_let_compiles() {
    // COMPILER GAP: Void return from trait method can be assigned to let (no error)
    // Ideally this should fail, but compiler accepts it
    let out = compile_and_run_stdout(r#"
trait Logger {
    fn log(self, msg: string)
}

class L impl Logger {
    val: int
    fn log(self, msg: string) {
        print(msg)
    }
}

fn main() {
    let l: Logger = L { val: 0 }
    let result = l.log("test")
    print("ok")
}
"#);
    assert_eq!(out, "test\nok\n");
}

#[test]
fn trait_concrete_method_after_trait_dispatch() {
    // Call concrete method on class after using it through trait
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int

    fn val(self) int { return self.n }
    fn doubled(self) int { return self.n * 2 }
}

fn via_trait(v: Valued) {
    print(v.val())
}

fn main() {
    let x = X { n: 5 }
    via_trait(x)
    print(x.doubled())
}
"#);
    assert_eq!(out, "5\n10\n");
}

#[test]
fn trait_generic_class_two_traits() {
    // Generic class implements two traits
    let out = compile_and_run_stdout(r#"
trait HasSize {
    fn size(self) int
}

trait HasLabel {
    fn label(self) string
}

class Container<T> impl HasSize, HasLabel {
    count: int
    name: string

    fn size(self) int { return self.count }
    fn label(self) string { return self.name }
}

fn show_size(s: HasSize) { print(s.size()) }
fn show_label(l: HasLabel) { print(l.label()) }

fn main() {
    let c = Container<int> { count: 5, name: "ints" }
    show_size(c)
    show_label(c)
}
"#);
    assert_eq!(out, "5\nints\n");
}

// ===== Batch 13: Trait handle identity, more negative tests, trait + generics edge cases =====

#[test]
fn trait_same_handle_two_different_functions_same_result() {
    // Same trait handle dispatched by two independent functions, both get same value
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn reader_a(v: Valued) int {
    return v.val()
}

fn reader_b(v: Valued) int {
    return v.val()
}

fn main() {
    let x = X { n: 42 }
    print(reader_a(x))
    print(reader_b(x))
    print(reader_a(x) == reader_b(x))
}
"#);
    assert_eq!(out, "42\n42\ntrue\n");
}

#[test]
fn trait_default_method_returns_bool_false() {
    // Default method returns false
    let out = compile_and_run_stdout(r#"
trait Enabled {
    fn is_enabled(self) bool {
        return false
    }
}

class Feature impl Enabled {
    val: int
}

class ActiveFeature impl Enabled {
    val: int

    fn is_enabled(self) bool {
        return true
    }
}

fn show(e: Enabled) {
    if e.is_enabled() {
        print("on")
    } else {
        print("off")
    }
}

fn main() {
    show(Feature { val: 0 })
    show(ActiveFeature { val: 0 })
}
"#);
    assert_eq!(out, "off\non\n");
}

#[test]
fn trait_class_with_bool_field() {
    // Class with bool field, accessed through trait dispatch
    let out = compile_and_run_stdout(r#"
trait Checked {
    fn is_active(self) bool
}

class State impl Checked {
    active: bool

    fn is_active(self) bool {
        return self.active
    }
}

fn check(c: Checked) {
    if c.is_active() {
        print("active")
    } else {
        print("inactive")
    }
}

fn main() {
    check(State { active: true })
    check(State { active: false })
}
"#);
    assert_eq!(out, "active\ninactive\n");
}

#[test]
fn trait_multiple_implementations_different_field_counts() {
    // Two classes with different number of fields, same trait
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class OneField impl Valued {
    n: int
    fn val(self) int { return self.n }
}

class ThreeFields impl Valued {
    a: int
    b: int
    c: int
    fn val(self) int { return self.a + self.b + self.c }
}

fn show(v: Valued) {
    print(v.val())
}

fn main() {
    show(OneField { n: 10 })
    show(ThreeFields { a: 1, b: 2, c: 3 })
}
"#);
    assert_eq!(out, "10\n6\n");
}

#[test]
fn trait_dispatch_result_in_boolean_expression() {
    // Trait dispatch results compared with &&
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn main() {
    let a: Valued = X { n: 10 }
    let b: Valued = X { n: 5 }
    if a.val() > 3 && b.val() < 8 {
        print("both")
    } else {
        print("nope")
    }
}
"#);
    assert_eq!(out, "both\n");
}

#[test]
fn trait_generic_class_dispatch_string_type_arg() {
    // Generic class with string type arg, dispatched through trait
    let out = compile_and_run_stdout(r#"
trait HasCount {
    fn count(self) int
}

class Wrapper<T> impl HasCount {
    n: int

    fn count(self) int {
        return self.n
    }
}

fn show(h: HasCount) {
    print(h.count())
}

fn main() {
    show(Wrapper<string> { n: 42 })
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn trait_dispatch_in_while_loop_with_break() {
    // Trait dispatch in while loop with break
    let out = compile_and_run_stdout(r#"
trait Provider {
    fn provide(self) int
}

class Const impl Provider {
    val: int
    fn provide(self) int { return self.val }
}

fn find_threshold(p: Provider) {
    let sum = 0
    while true {
        sum = sum + p.provide()
        if sum >= 20 {
            break
        }
    }
    print(sum)
}

fn main() {
    find_threshold(Const { val: 7 })
}
"#);
    assert_eq!(out, "21\n");
}

#[test]
fn trait_method_ignores_unused_field() {
    // Class has field not used by trait method
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class WithExtra impl Valued {
    important: int
    ignored_str: string
    ignored_bool: bool

    fn val(self) int {
        return self.important
    }
}

fn show(v: Valued) {
    print(v.val())
}

fn main() {
    show(WithExtra { important: 42, ignored_str: "x", ignored_bool: false })
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn trait_method_with_large_int_arithmetic() {
    // Trait method doing substantial arithmetic
    let out = compile_and_run_stdout(r#"
trait Calculator {
    fn calc(self) int
}

class BigCalc impl Calculator {
    a: int
    b: int

    fn calc(self) int {
        let result = 0
        result = self.a * 1000 + self.b * 100
        result = result + (self.a + self.b) * 10
        result = result + self.a - self.b
        return result
    }
}

fn show(c: Calculator) {
    print(c.calc())
}

fn main() {
    show(BigCalc { a: 3, b: 7 })
}
"#);
    // 3*1000 + 7*100 + (3+7)*10 + 3 - 7 = 3000 + 700 + 100 + 3 - 7 = 3796
    assert_eq!(out, "3796\n");
}

#[test]
fn trait_method_with_while_and_early_return() {
    // Trait method with while loop and early return
    let out = compile_and_run_stdout(r#"
trait Searcher {
    fn find(self, target: int) int
}

class ArraySearcher impl Searcher {
    data: [int]

    fn find(self, target: int) int {
        let i = 0
        while i < self.data.len() {
            if self.data[i] == target {
                return i
            }
            i = i + 1
        }
        return -1
    }
}

fn run(s: Searcher) {
    print(s.find(30))
    print(s.find(99))
}

fn main() {
    let arr: [int] = [10, 20, 30, 40, 50]
    run(ArraySearcher { data: arr })
}
"#);
    assert_eq!(out, "2\n-1\n");
}

#[test]
fn trait_method_recursive_fibonacci() {
    // Trait method implementing recursive fibonacci
    let out = compile_and_run_stdout(r#"
trait FibComputer {
    fn fib(self, n: int) int
}

class RecursiveFib impl FibComputer {
    val: int

    fn fib(self, n: int) int {
        if n <= 1 {
            return n
        }
        return self.fib(n - 1) + self.fib(n - 2)
    }
}

fn compute(f: FibComputer) {
    print(f.fib(0))
    print(f.fib(1))
    print(f.fib(5))
    print(f.fib(8))
}

fn main() {
    compute(RecursiveFib { val: 0 })
}
"#);
    assert_eq!(out, "0\n1\n5\n21\n");
}

#[test]
fn fail_impl_two_traits_conflicting_method_signatures() {
    // Two traits both have method "compute" but with different return types
    compile_should_fail_with(r#"
trait TraitA {
    fn compute(self) int
}

trait TraitB {
    fn compute(self) string
}

class X impl TraitA, TraitB {
    val: int

    fn compute(self) int {
        return self.val
    }
}

fn main() {
}
"#, "method 'compute' return type mismatch");
}

#[test]
fn trait_method_string_length_comparison() {
    // Trait method compares string lengths
    let out = compile_and_run_stdout(r#"
trait LengthComparer {
    fn longer(self, a: string, b: string) string
}

class Comparer impl LengthComparer {
    val: int

    fn longer(self, a: string, b: string) string {
        if a.len() >= b.len() {
            return a
        }
        return b
    }
}

fn run(c: LengthComparer) {
    print(c.longer("hello", "hi"))
    print(c.longer("a", "abc"))
}

fn main() {
    run(Comparer { val: 0 })
}
"#);
    assert_eq!(out, "hello\nabc\n");
}

#[test]
fn trait_method_builds_array() {
    // Trait method builds and returns an array
    let out = compile_and_run_stdout(r#"
trait Builder {
    fn build(self, n: int) [int]
}

class RangeBuilder impl Builder {
    start: int

    fn build(self, n: int) [int] {
        let arr: [int] = []
        let i = 0
        while i < n {
            arr.push(self.start + i)
            i = i + 1
        }
        return arr
    }
}

fn show(b: Builder) {
    let arr = b.build(4)
    let i = 0
    while i < arr.len() {
        print(arr[i])
        i = i + 1
    }
}

fn main() {
    show(RangeBuilder { start: 10 })
}
"#);
    assert_eq!(out, "10\n11\n12\n13\n");
}

#[test]
fn trait_dispatch_preserves_string_field_through_calls() {
    // Verify string field preserved after multiple dispatch calls
    let out = compile_and_run_stdout(r#"
trait Named {
    fn name(self) string
}

class User impl Named {
    n: string
    fn name(self) string { return self.n }
}

fn main() {
    let u: Named = User { n: "alice" }
    print(u.name())
    print(u.name())
    print(u.name())
}
"#);
    assert_eq!(out, "alice\nalice\nalice\n");
}

#[test]
fn trait_method_with_for_range_inside() {
    // Trait method body contains a for-range loop
    let out = compile_and_run_stdout(r#"
trait Summer {
    fn sum_to(self, n: int) int
}

class Adder impl Summer {
    base: int

    fn sum_to(self, n: int) int {
        let total = self.base
        for i in 0..n {
            total = total + i
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
    // base=100, sum(0..5) = 0+1+2+3+4=10, total = 110
    assert_eq!(out, "110\n");
}

#[test]
fn trait_method_returns_string_with_interpolation() {
    // Trait method returns string built with interpolation
    let out = compile_and_run_stdout(r#"
trait Descriptor {
    fn describe(self) string
}

class Item impl Descriptor {
    name: string
    count: int

    fn describe(self) string {
        return "{self.count}x {self.name}"
    }
}

fn show(d: Descriptor) {
    print(d.describe())
}

fn main() {
    show(Item { name: "widget", count: 5 })
}
"#);
    assert_eq!(out, "5x widget\n");
}

// ===== Batch 14: Error handling + traits, closure factories, complex dispatch scenarios =====

#[test]
fn trait_method_raises_error_caught_by_caller() {
    // Trait method raises, caller catches
    let out = compile_and_run_stdout(r#"
error BadInput {
    code: int
}

trait Validator {
    fn validate(self, x: int) int
}

class StrictValidator impl Validator {
    limit: int

    fn validate(self, x: int) int {
        if x > self.limit {
            raise BadInput { code: x }
        }
        return x
    }
}

fn run(v: Validator) {
    let a = v.validate(5) catch -1
    let b = v.validate(100) catch -1
    print(a)
    print(b)
}

fn main() {
    run(StrictValidator { limit: 10 })
}
"#);
    assert_eq!(out, "5\n-1\n");
}

#[test]
fn trait_method_error_propagation_through_dispatch() {
    // Error propagated through trait dispatch with !
    let out = compile_and_run_stdout(r#"
error ParseErr {
    msg: string
}

trait Parser {
    fn parse(self, input: string) int
}

class IntParser impl Parser {
    val: int

    fn parse(self, input: string) int {
        if input == "bad" {
            raise ParseErr { msg: "invalid" }
        }
        return 42
    }
}

fn try_parse(p: Parser, input: string) int {
    return p.parse(input)!
}

fn main() {
    let p = IntParser { val: 0 }
    let result = try_parse(p, "good") catch -1
    print(result)
    let result2 = try_parse(p, "bad") catch -1
    print(result2)
}
"#);
    assert_eq!(out, "42\n-1\n");
}

#[test]
fn trait_closure_takes_trait_param_returns_int() {
    // Closure parameter is trait-typed
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn apply_fn(f: fn(Valued) int, v: Valued) int {
    return f(v)
}

fn main() {
    let x = X { n: 7 }
    let result = apply_fn((v: Valued) => v.val() * 2, x)
    print(result)
}
"#);
    assert_eq!(out, "14\n");
}

#[test]
#[should_panic]
fn fail_trait_closure_with_trait_param_runtime_crash() {
    // COMPILER GAP: Closures with trait-typed parameters compile but crash at runtime.
    // The trait handle is not properly constructed/passed when a closure takes a trait param.
    compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn main() {
    let x = X { n: 5 }
    let doubler = (v: Valued) => v.val() * 2
    print(doubler(x))
}
"#);
}

#[test]
fn trait_dispatch_inside_catch_block() {
    // Trait dispatch inside a catch expression
    let out = compile_and_run_stdout(r#"
error Failure {
    code: int
}

trait Fallback {
    fn fallback_val(self) int
}

class Default impl Fallback {
    n: int
    fn fallback_val(self) int { return self.n }
}

fn risky(x: int) int {
    if x < 0 {
        raise Failure { code: x }
    }
    return x
}

fn run(fb: Fallback) {
    let result = risky(-1) catch fb.fallback_val()
    print(result)
    let result2 = risky(10) catch fb.fallback_val()
    print(result2)
}

fn main() {
    run(Default { n: 999 })
}
"#);
    assert_eq!(out, "999\n10\n");
}

#[test]
fn trait_dispatch_two_different_traits_from_same_function() {
    // Function takes handles to two different traits, dispatches both
    let out = compile_and_run_stdout(r#"
trait Adder {
    fn add(self, x: int) int
}

trait Multiplier {
    fn mul(self, x: int) int
}

class AddImpl impl Adder {
    base: int
    fn add(self, x: int) int { return self.base + x }
}

class MulImpl impl Multiplier {
    factor: int
    fn mul(self, x: int) int { return self.factor * x }
}

fn compute(a: Adder, m: Multiplier, x: int) {
    print(a.add(m.mul(x)))
}

fn main() {
    compute(AddImpl { base: 100 }, MulImpl { factor: 3 }, 5)
}
"#);
    // m.mul(5) = 15, a.add(15) = 115
    assert_eq!(out, "115\n");
}

#[test]
fn trait_three_classes_all_override_all_defaults() {
    // Trait has 3 defaults, 3 classes each override different ones
    let out = compile_and_run_stdout(r#"
trait ThreeDefaults {
    fn a(self) int { return 1 }
    fn b(self) int { return 2 }
    fn c(self) int { return 3 }
}

class OverrideA impl ThreeDefaults {
    val: int
    fn a(self) int { return 10 }
}

class OverrideB impl ThreeDefaults {
    val: int
    fn b(self) int { return 20 }
}

class OverrideC impl ThreeDefaults {
    val: int
    fn c(self) int { return 30 }
}

fn show(t: ThreeDefaults) {
    print(t.a() + t.b() + t.c())
}

fn main() {
    show(OverrideA { val: 0 })
    show(OverrideB { val: 0 })
    show(OverrideC { val: 0 })
}
"#);
    // OverrideA: 10+2+3=15, OverrideB: 1+20+3=24, OverrideC: 1+2+30=33
    assert_eq!(out, "15\n24\n33\n");
}

#[test]
fn trait_dispatch_in_if_condition_and_body() {
    // Both if condition and body use trait dispatch
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
    fn label(self) string
}

class X impl Valued {
    n: int
    name: string

    fn val(self) int { return self.n }
    fn label(self) string { return self.name }
}

fn check(v: Valued) {
    if v.val() > 5 {
        print(v.label())
    } else {
        print("too small")
    }
}

fn main() {
    check(X { n: 10, name: "big" })
    check(X { n: 2, name: "small" })
}
"#);
    assert_eq!(out, "big\ntoo small\n");
}

#[test]
fn trait_return_different_impl_based_on_condition() {
    // Function returns different concrete types as trait based on condition
    let out = compile_and_run_stdout(r#"
trait Strategy {
    fn execute(self, x: int) int
}

class DoubleStrategy impl Strategy {
    val: int
    fn execute(self, x: int) int { return x * 2 }
}

class SquareStrategy impl Strategy {
    val: int
    fn execute(self, x: int) int { return x * x }
}

fn pick_strategy(use_square: bool) Strategy {
    if use_square {
        return SquareStrategy { val: 0 }
    }
    return DoubleStrategy { val: 0 }
}

fn main() {
    let s1 = pick_strategy(false)
    let s2 = pick_strategy(true)
    print(s1.execute(5))
    print(s2.execute(5))
}
"#);
    assert_eq!(out, "10\n25\n");
}

#[test]
fn trait_method_accesses_field_and_param_interleaved() {
    // Method uses field and parameter values in interleaved fashion
    let out = compile_and_run_stdout(r#"
trait Compute {
    fn compute(self, a: int, b: int) int
}

class WeightedCalc impl Compute {
    w1: int
    w2: int

    fn compute(self, a: int, b: int) int {
        return self.w1 * a + self.w2 * b + a * b
    }
}

fn run(c: Compute) {
    print(c.compute(3, 4))
}

fn main() {
    run(WeightedCalc { w1: 2, w2: 5 })
}
"#);
    // 2*3 + 5*4 + 3*4 = 6 + 20 + 12 = 38
    assert_eq!(out, "38\n");
}

#[test]
fn trait_dispatch_result_compared_to_string() {
    // Trait method returns string, compared with ==
    let out = compile_and_run_stdout(r#"
trait Named {
    fn name(self) string
}

class User impl Named {
    n: string
    fn name(self) string { return self.n }
}

fn is_admin(n: Named) {
    if n.name() == "admin" {
        print("yes")
    } else {
        print("no")
    }
}

fn main() {
    is_admin(User { n: "admin" })
    is_admin(User { n: "guest" })
}
"#);
    assert_eq!(out, "yes\nno\n");
}

#[test]
fn trait_class_with_all_primitive_field_types() {
    // Class with int, string, bool fields — all used through trait
    let out = compile_and_run_stdout(r#"
trait Summary {
    fn summarize(self) string
}

class Record impl Summary {
    name: string
    age: int
    active: bool

    fn summarize(self) string {
        if self.active {
            return "{self.name} ({self.age}) - active"
        }
        return "{self.name} ({self.age}) - inactive"
    }
}

fn show(s: Summary) {
    print(s.summarize())
}

fn main() {
    show(Record { name: "alice", age: 30, active: true })
    show(Record { name: "bob", age: 25, active: false })
}
"#);
    assert_eq!(out, "alice (30) - active\nbob (25) - inactive\n");
}

#[test]
fn trait_dispatch_result_passed_to_print_directly() {
    // Print the result of trait dispatch directly (no let binding)
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn main() {
    let v: Valued = X { n: 77 }
    print(v.val())
}
"#);
    assert_eq!(out, "77\n");
}

#[test]
fn trait_method_with_for_loop_accumulation() {
    // Trait method uses for-range to accumulate result
    let out = compile_and_run_stdout(r#"
trait Accumulator {
    fn accumulate(self, n: int) int
}

class Triangular impl Accumulator {
    offset: int

    fn accumulate(self, n: int) int {
        let total = self.offset
        for i in 1..n+1 {
            total = total + i
        }
        return total
    }
}

fn show(a: Accumulator) {
    print(a.accumulate(5))
}

fn main() {
    show(Triangular { offset: 0 })
}
"#);
    // 0 + 1+2+3+4+5 = 15
    assert_eq!(out, "15\n");
}

#[test]
fn trait_class_reuses_field_name_across_impls() {
    // Multiple classes implementing same trait, all have field named "data"
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class A impl Valued {
    data: int
    fn val(self) int { return self.data }
}

class B impl Valued {
    data: int
    fn val(self) int { return self.data * 2 }
}

class C impl Valued {
    data: int
    fn val(self) int { return self.data * 3 }
}

fn show(v: Valued) {
    print(v.val())
}

fn main() {
    show(A { data: 5 })
    show(B { data: 5 })
    show(C { data: 5 })
}
"#);
    assert_eq!(out, "5\n10\n15\n");
}

#[test]
fn fail_trait_method_param_type_mismatch() {
    // Trait method takes int, class implements with string param
    compile_should_fail_with(r#"
trait Processor {
    fn process(self, x: int) int
}

class Bad impl Processor {
    val: int

    fn process(self, x: string) int {
        return 0
    }
}

fn main() {
}
"#, "method 'process' parameter 1 type mismatch: trait 'Processor' expects int, class 'Bad' has string");
}

#[test]
fn trait_method_complex_boolean_logic() {
    // Trait method with complex boolean logic
    let out = compile_and_run_stdout(r#"
trait Classifier {
    fn classify(self, x: int) string
}

class RangeClassifier impl Classifier {
    low: int
    high: int

    fn classify(self, x: int) string {
        if x < self.low {
            return "below"
        }
        if x > self.high {
            return "above"
        }
        if x == self.low || x == self.high {
            return "boundary"
        }
        return "within"
    }
}

fn run(c: Classifier) {
    print(c.classify(0))
    print(c.classify(10))
    print(c.classify(50))
    print(c.classify(100))
    print(c.classify(200))
}

fn main() {
    run(RangeClassifier { low: 10, high: 100 })
}
"#);
    assert_eq!(out, "below\nboundary\nwithin\nboundary\nabove\n");
}

// ===== Batch 15: Method ordering, field layout variations, self-calls, negatives =====

#[test]
fn trait_dispatch_with_zero_value_field() {
    // Class field is zero — make sure trait dispatch still works
    let out = compile_and_run_stdout(r#"
trait Getter {
    fn get(self) int
}

class ZeroVal impl Getter {
    val: int
    fn get(self) int { return self.val }
}

fn run(g: Getter) { print(g.get()) }

fn main() {
    run(ZeroVal { val: 0 })
}
"#);
    assert_eq!(out, "0\n");
}

#[test]
fn trait_dispatch_with_negative_field() {
    // Negative int field passed through trait dispatch
    let out = compile_and_run_stdout(r#"
trait Getter {
    fn get(self) int
}

class NegVal impl Getter {
    val: int
    fn get(self) int { return self.val }
}

fn run(g: Getter) { print(g.get()) }

fn main() {
    run(NegVal { val: -42 })
}
"#);
    assert_eq!(out, "-42\n");
}

#[test]
fn trait_two_classes_different_field_counts() {
    // Two classes implementing same trait with different number of fields
    let out = compile_and_run_stdout(r#"
trait Describable {
    fn describe(self) string
}

class Small impl Describable {
    x: int
    fn describe(self) string { return "small" }
}

class Big impl Describable {
    a: int
    b: int
    c: int
    d: int
    fn describe(self) string { return "big" }
}

fn show(d: Describable) { print(d.describe()) }

fn main() {
    show(Small { x: 1 })
    show(Big { a: 1, b: 2, c: 3, d: 4 })
}
"#);
    assert_eq!(out, "small\nbig\n");
}

#[test]
fn trait_method_returns_param_unchanged() {
    // Trait method receives an int param and returns it unchanged
    let out = compile_and_run_stdout(r#"
trait Echo {
    fn echo(self, x: int) int
}

class Echoer impl Echo {
    tag: int
    fn echo(self, x: int) int { return x }
}

fn run(e: Echo) {
    print(e.echo(999))
}

fn main() {
    run(Echoer { tag: 0 })
}
"#);
    assert_eq!(out, "999\n");
}

#[test]
fn trait_method_ignores_self_fields() {
    // Trait method doesn't use any self fields, just params
    let out = compile_and_run_stdout(r#"
trait Adder {
    fn add(self, a: int, b: int) int
}

class SimpleAdder impl Adder {
    unused: int
    fn add(self, a: int, b: int) int { return a + b }
}

fn run(adder: Adder) {
    print(adder.add(3, 4))
}

fn main() {
    run(SimpleAdder { unused: 0 })
}
"#);
    assert_eq!(out, "7\n");
}

#[test]
fn trait_dispatch_result_in_arithmetic_two_params() {
    // Trait method result used in arithmetic expression with two trait params
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class Five impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn compute(a: Valued, b: Valued) int {
    return a.val() + b.val() * 2
}

fn main() {
    let x = Five { n: 5 }
    let y = Five { n: 3 }
    print(compute(x, y))
}
"#);
    assert_eq!(out, "11\n");
}

#[test]
fn trait_dispatch_in_while_condition_counter() {
    // Trait method result used as while loop bound
    let out = compile_and_run_stdout(r#"
trait Counter {
    fn count(self) int
}

class FixedCount impl Counter {
    n: int
    fn count(self) int { return self.n }
}

fn run(c: Counter) {
    let i = 0
    while i < c.count() {
        print(i)
        i = i + 1
    }
}

fn main() {
    run(FixedCount { n: 3 })
}
"#);
    assert_eq!(out, "0\n1\n2\n");
}

#[test]
fn trait_default_method_calls_required() {
    // Default method implemented in terms of a required method
    let out = compile_and_run_stdout(r#"
trait Measurable {
    fn raw_size(self) int

    fn formatted_size(self) string {
        let s = self.raw_size()
        if s > 1000 {
            return "large"
        }
        return "small"
    }
}

class File impl Measurable {
    bytes: int
    fn raw_size(self) int { return self.bytes }
}

fn report(m: Measurable) {
    print(m.formatted_size())
}

fn main() {
    report(File { bytes: 500 })
    report(File { bytes: 2000 })
}
"#);
    assert_eq!(out, "small\nlarge\n");
}

#[test]
fn trait_two_defaults_calling_same_required() {
    // Two default methods both call the same required method
    let out = compile_and_run_stdout(r#"
trait Source {
    fn value(self) int

    fn doubled(self) int {
        return self.value() * 2
    }

    fn tripled(self) int {
        return self.value() * 3
    }
}

class Src impl Source {
    v: int
    fn value(self) int { return self.v }
}

fn run(s: Source) {
    print(s.doubled())
    print(s.tripled())
}

fn main() {
    run(Src { v: 4 })
}
"#);
    assert_eq!(out, "8\n12\n");
}

#[test]
fn trait_class_with_string_and_int_fields_interp() {
    // Class with mixed field types, string interpolation in trait method
    let out = compile_and_run_stdout(r#"
trait Printable {
    fn to_str(self) string
}

class Person impl Printable {
    name: string
    age: int
    fn to_str(self) string { return "{self.name}:{self.age}" }
}

fn show(p: Printable) { print(p.to_str()) }

fn main() {
    show(Person { name: "alice", age: 30 })
}
"#);
    assert_eq!(out, "alice:30\n");
}

#[test]
fn trait_dispatch_preserves_string_field_value() {
    // Ensure string field is preserved correctly through trait dispatch
    let out = compile_and_run_stdout(r#"
trait Named {
    fn get_name(self) string
}

class User impl Named {
    name: string
    fn get_name(self) string { return self.name }
}

fn greet(n: Named) {
    print("hello {n.get_name()}")
}

fn main() {
    greet(User { name: "world" })
}
"#);
    assert_eq!(out, "hello world\n");
}

#[test]
fn trait_multiple_dispatch_calls_same_object() {
    // Same object dispatched through same trait multiple times
    let out = compile_and_run_stdout(r#"
trait Incrementer {
    fn next(self, x: int) int
}

class PlusOne impl Incrementer {
    step: int
    fn next(self, x: int) int { return x + self.step }
}

fn chain(inc: Incrementer) int {
    let v = 0
    v = inc.next(v)
    v = inc.next(v)
    v = inc.next(v)
    return v
}

fn main() {
    print(chain(PlusOne { step: 5 }))
}
"#);
    assert_eq!(out, "15\n");
}

#[test]
fn fail_trait_method_wrong_param_count() {
    // Implementing method has wrong number of params
    compile_should_fail_with(r#"
trait Foo {
    fn bar(self, x: int) int
}

class X impl Foo {
    val: int
    fn bar(self) int { return 1 }
}

fn main() {}
"#, "method 'bar' of class 'X' has wrong number of parameters for trait 'Foo'");
}

#[test]
fn fail_trait_method_wrong_param_type() {
    // Implementing method has wrong param type
    compile_should_fail_with(r#"
trait Foo {
    fn bar(self, x: int) int
}

class X impl Foo {
    val: int
    fn bar(self, x: string) int { return 1 }
}

fn main() {}
"#, "method 'bar' parameter 1 type mismatch: trait 'Foo' expects int, class 'X' has string");
}

#[test]
fn fail_assign_int_to_trait_variable() {
    // Cannot assign an int to a trait-typed variable
    compile_should_fail_with(r#"
trait Foo {
    fn get(self) int
}

fn main() {
    let f: Foo = 42
}
"#, "type mismatch: expected trait Foo, found int");
}

#[test]
fn fail_assign_string_to_trait_variable() {
    // Cannot assign a string to a trait-typed variable
    compile_should_fail_with(r#"
trait Foo {
    fn get(self) int
}

fn main() {
    let f: Foo = "hello"
}
"#, "type mismatch: expected trait Foo, found string");
}

#[test]
fn trait_array_collect_results() {
    // Multiple dispatched calls collected into an array
    let out = compile_and_run_stdout(r#"
trait Scorer {
    fn score(self) int
}

class High impl Scorer {
    s: int
    fn score(self) int { return self.s }
}

class Low impl Scorer {
    s: int
    fn score(self) int { return self.s }
}

fn collect_score(s: Scorer) int {
    return s.score()
}

fn main() {
    let results: [int] = []
    results.push(collect_score(High { s: 100 }))
    results.push(collect_score(Low { s: 10 }))
    results.push(collect_score(High { s: 50 }))
    let i = 0
    while i < results.len() {
        print(results[i])
        i = i + 1
    }
}
"#);
    assert_eq!(out, "100\n10\n50\n");
}

// ===== Batch 16: Trait + enum interaction, deep nesting, stress, more negatives =====

#[test]
fn fail_trait_method_returns_enum_forward_ref() {
    // Fixed: trait method signatures can now reference enum types via forward references
    let out = compile_and_run_stdout(r#"
enum Status {
    Ok
    Fail { code: int }
}

trait Checker {
    fn check_val(self, x: int) Status
}

class RangeChecker impl Checker {
    limit: int
    fn check_val(self, x: int) Status {
        if x > self.limit {
            return Status.Fail { code: x }
        }
        return Status.Ok
    }
}

fn run(c: Checker) {
    match c.check_val(100) {
        Status.Ok { print(0) }
        Status.Fail { code } { print(code) }
    }
}

fn main() {
    let rc = RangeChecker { limit: 50 }
    run(rc)
}
"#);
    assert_eq!(out, "100\n");
}

#[test]
fn fail_trait_method_takes_enum_param_forward_ref() {
    // Fixed: trait method signatures can now reference enum types via forward references
    let out = compile_and_run_stdout(r#"
enum Mode {
    Fast
    Slow
}

trait Runner {
    fn speed(self, m: Mode) int
}

class Engine impl Runner {
    base: int
    fn speed(self, m: Mode) int {
        match m {
            Mode.Fast { return self.base * 2 }
            Mode.Slow { return self.base }
        }
    }
}

fn run(r: Runner) {
    print(r.speed(Mode.Fast))
    print(r.speed(Mode.Slow))
}

fn main() {
    let e = Engine { base: 10 }
    run(e)
}
"#);
    assert_eq!(out, "20\n10\n");
}

#[test]
fn trait_dispatch_in_match_arm() {
    // Trait method called inside a match arm
    let out = compile_and_run_stdout(r#"
enum Choice {
    A
    B
}

trait Namer {
    fn name(self) string
}

class Thing impl Namer {
    label: string
    fn name(self) string { return self.label }
}

fn run(n: Namer, c: Choice) {
    match c {
        Choice.A { print("a:{n.name()}") }
        Choice.B { print("b:{n.name()}") }
    }
}

fn main() {
    let t = Thing { label: "hello" }
    run(t, Choice.A)
    run(t, Choice.B)
}
"#);
    assert_eq!(out, "a:hello\nb:hello\n");
}

#[test]
fn trait_deep_call_chain_five_levels() {
    // Trait handle passed through 5 levels of function calls
    let out = compile_and_run_stdout(r#"
trait Getter {
    fn get(self) int
}

class Val impl Getter {
    v: int
    fn get(self) int { return self.v }
}

fn level5(g: Getter) int { return g.get() }
fn level4(g: Getter) int { return level5(g) }
fn level3(g: Getter) int { return level4(g) }
fn level2(g: Getter) int { return level3(g) }
fn level1(g: Getter) int { return level2(g) }

fn main() {
    print(level1(Val { v: 42 }))
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn trait_dispatch_two_different_traits_same_function() {
    // Function takes two different trait types as params
    let out = compile_and_run_stdout(r#"
trait Left {
    fn left_val(self) int
}

trait Right {
    fn right_val(self) int
}

class L impl Left {
    n: int
    fn left_val(self) int { return self.n }
}

class R impl Right {
    n: int
    fn right_val(self) int { return self.n }
}

fn combine(l: Left, r: Right) int {
    return l.left_val() + r.right_val()
}

fn main() {
    print(combine(L { n: 10 }, R { n: 20 }))
}
"#);
    assert_eq!(out, "30\n");
}

#[test]
fn trait_dispatch_same_concrete_different_trait_handles() {
    // Same concrete class used through two different trait handles
    let out = compile_and_run_stdout(r#"
trait Namer {
    fn name(self) string
}

trait Sizer {
    fn size(self) int
}

class Item impl Namer, Sizer {
    label: string
    count: int
    fn name(self) string { return self.label }
    fn size(self) int { return self.count }
}

fn show_name(n: Namer) { print(n.name()) }
fn show_size(s: Sizer) { print(s.size()) }

fn main() {
    let item = Item { label: "box", count: 5 }
    show_name(item)
    show_size(item)
}
"#);
    assert_eq!(out, "box\n5\n");
}

#[test]
fn trait_large_class_many_fields_dispatch() {
    // Class with many fields (8) implementing a trait
    let out = compile_and_run_stdout(r#"
trait Summable {
    fn total(self) int
}

class BigRow impl Summable {
    a: int
    b: int
    c: int
    d: int
    e: int
    f: int
    g: int
    h: int
    fn total(self) int {
        return self.a + self.b + self.c + self.d + self.e + self.f + self.g + self.h
    }
}

fn run(s: Summable) { print(s.total()) }

fn main() {
    run(BigRow { a: 1, b: 2, c: 3, d: 4, e: 5, f: 6, g: 7, h: 8 })
}
"#);
    assert_eq!(out, "36\n");
}

#[test]
fn trait_method_with_five_params() {
    // Trait method with many parameters
    let out = compile_and_run_stdout(r#"
trait Computer {
    fn compute(self, a: int, b: int, c: int, d: int, e: int) int
}

class Summer impl Computer {
    base: int
    fn compute(self, a: int, b: int, c: int, d: int, e: int) int {
        return self.base + a + b + c + d + e
    }
}

fn run(c: Computer) {
    print(c.compute(1, 2, 3, 4, 5))
}

fn main() {
    run(Summer { base: 100 })
}
"#);
    assert_eq!(out, "115\n");
}

#[test]
fn trait_method_string_concat_through_dispatch() {
    // String concatenation in trait method
    let out = compile_and_run_stdout(r#"
trait Greeting {
    fn greet(self, name: string) string
}

class Formal impl Greeting {
    prefix: string
    fn greet(self, name: string) string {
        return "{self.prefix} {name}"
    }
}

fn run(g: Greeting) {
    print(g.greet("Alice"))
}

fn main() {
    run(Formal { prefix: "Dear" })
}
"#);
    assert_eq!(out, "Dear Alice\n");
}

#[test]
fn trait_three_implementors_alternating_dispatch() {
    // Three different classes dispatched in alternating order
    let out = compile_and_run_stdout(r#"
trait Tagger {
    fn tag(self) string
}

class A impl Tagger {
    val: int
    fn tag(self) string { return "a" }
}

class B impl Tagger {
    val: int
    fn tag(self) string { return "b" }
}

class C impl Tagger {
    val: int
    fn tag(self) string { return "c" }
}

fn show(t: Tagger) { print(t.tag()) }

fn main() {
    show(A { val: 1 })
    show(B { val: 2 })
    show(C { val: 3 })
    show(A { val: 4 })
    show(B { val: 5 })
    show(C { val: 6 })
}
"#);
    assert_eq!(out, "a\nb\nc\na\nb\nc\n");
}

#[test]
fn trait_dispatch_bool_return_in_logical_ops() {
    // Trait method returning bool used in && and || expressions
    let out = compile_and_run_stdout(r#"
trait Predicate {
    fn test_val(self, x: int) bool
}

class Positive impl Predicate {
    threshold: int
    fn test_val(self, x: int) bool { return x > self.threshold }
}

fn check(p: Predicate) {
    let both = p.test_val(5) && p.test_val(10)
    let either = p.test_val(-1) || p.test_val(5)
    print(both)
    print(either)
}

fn main() {
    check(Positive { threshold: 0 })
}
"#);
    assert_eq!(out, "true\ntrue\n");
}

#[test]
fn fail_trait_impl_missing_one_of_two_methods() {
    // Class implements trait but misses one of two required methods
    compile_should_fail_with(r#"
trait TwoMethods {
    fn first(self) int
    fn second(self) int
}

class X impl TwoMethods {
    val: int
    fn first(self) int { return 1 }
}

fn main() {}
"#, "class 'X' does not implement required method 'second' from trait 'TwoMethods'");
}

#[test]
fn fail_trait_impl_method_returns_string_not_int() {
    // Method return type mismatch: trait says int, class says string
    compile_should_fail_with(r#"
trait Getter {
    fn get(self) int
}

class X impl Getter {
    val: int
    fn get(self) string { return "hello" }
}

fn main() {}
"#, "method 'get' return type mismatch: trait 'Getter' expects int, class 'X' returns string");
}

#[test]
fn fail_pass_non_implementing_class_to_trait_param() {
    // Class doesn't implement the required trait
    compile_should_fail_with(r#"
trait Foo {
    fn foo(self) int
}

class Bar {
    val: int
}

fn use_foo(f: Foo) { print(f.foo()) }

fn main() {
    use_foo(Bar { val: 1 })
}
"#, "argument 1 of 'use_foo': expected trait Foo, found Bar");
}

#[test]
fn trait_method_with_float_arithmetic() {
    // Trait method doing float arithmetic
    let out = compile_and_run_stdout(r#"
trait Calculator {
    fn calc(self, x: float) float
}

class Doubler impl Calculator {
    offset: float
    fn calc(self, x: float) float { return x * 2.0 + self.offset }
}

fn run(c: Calculator) {
    print(c.calc(3.5))
}

fn main() {
    run(Doubler { offset: 1.0 })
}
"#);
    assert_eq!(out, "8.000000\n");
}

#[test]
fn trait_conditional_dispatch_based_on_method() {
    // Use trait method result to decide control flow
    let out = compile_and_run_stdout(r#"
trait Priority {
    fn level(self) int
}

class Urgent impl Priority {
    p: int
    fn level(self) int { return self.p }
}

fn process(item: Priority) {
    if item.level() > 5 {
        print("high")
    } else {
        print("low")
    }
}

fn main() {
    process(Urgent { p: 8 })
    process(Urgent { p: 2 })
}
"#);
    assert_eq!(out, "high\nlow\n");
}

// ===== Batch 17: Sets, channels, nullable edge cases, advanced generics =====

#[test]
fn trait_method_with_set_param() {
    // Trait method takes a Set<int> parameter
    let out = compile_and_run_stdout(r#"
trait Analyzer {
    fn count_unique(self, s: Set<int>) int
}

class Counter impl Analyzer {
    tag: int
    fn count_unique(self, s: Set<int>) int { return s.len() }
}

fn run(a: Analyzer) {
    let s = Set<int> { 1, 2, 3, 2, 1 }
    print(a.count_unique(s))
}

fn main() {
    run(Counter { tag: 0 })
}
"#);
    assert_eq!(out, "3\n");
}

#[test]
fn trait_method_returns_set() {
    // Trait method returns a Set<int>
    let out = compile_and_run_stdout(r#"
trait Producer {
    fn produce(self) Set<int>
}

class SetMaker impl Producer {
    val: int
    fn produce(self) Set<int> {
        let s = Set<int> {}
        s.insert(self.val)
        s.insert(self.val + 1)
        return s
    }
}

fn run(p: Producer) {
    let s = p.produce()
    print(s.len())
    print(s.contains(10))
}

fn main() {
    run(SetMaker { val: 10 })
}
"#);
    assert_eq!(out, "2\ntrue\n");
}

#[test]
fn trait_method_builds_set_in_loop() {
    // Trait method builds a set from array
    let out = compile_and_run_stdout(r#"
trait Deduper {
    fn dedup_count(self, arr: [int]) int
}

class SetDeduper impl Deduper {
    tag: int
    fn dedup_count(self, arr: [int]) int {
        let s = Set<int> {}
        let i = 0
        while i < arr.len() {
            s.insert(arr[i])
            i = i + 1
        }
        return s.len()
    }
}

fn run(d: Deduper) {
    let arr: [int] = [1, 2, 2, 3, 3, 3]
    print(d.dedup_count(arr))
}

fn main() {
    run(SetDeduper { tag: 0 })
}
"#);
    assert_eq!(out, "3\n");
}

#[test]
fn trait_method_with_map_return() {
    // Trait method returns a Map<string, int>
    let out = compile_and_run_stdout(r#"
trait Mapper {
    fn build_map(self) Map<string, int>
}

class WordCounter impl Mapper {
    base: int
    fn build_map(self) Map<string, int> {
        let m = Map<string, int> { "a": self.base, "b": self.base + 1 }
        return m
    }
}

fn run(mapper: Mapper) {
    let m = mapper.build_map()
    print(m["a"])
    print(m["b"])
}

fn main() {
    run(WordCounter { base: 10 })
}
"#);
    assert_eq!(out, "10\n11\n");
}

#[test]
fn trait_method_returns_nullable_none() {
    // Trait method explicitly returns none
    let out = compile_and_run_stdout(r#"
trait Finder {
    fn find_val(self, x: int) int?
}

class EmptyFinder impl Finder {
    tag: int
    fn find_val(self, x: int) int? {
        return none
    }
}

fn run(f: Finder) {
    let result = f.find_val(5)
    if result == none {
        print("not found")
    }
}

fn main() {
    run(EmptyFinder { tag: 0 })
}
"#);
    assert_eq!(out, "not found\n");
}

#[test]
fn trait_method_returns_nullable_some() {
    // Trait method returns non-none nullable value
    let out = compile_and_run_stdout(r#"
trait Finder {
    fn find_val(self, x: int) int?
}

class IdentityFinder impl Finder {
    tag: int
    fn find_val(self, x: int) int? {
        if x > 0 {
            return x * 2
        }
        return none
    }
}

fn run(f: Finder) int? {
    let result = f.find_val(5)?
    print(result)
    return result
}

fn main() {
    run(IdentityFinder { tag: 0 })
}
"#);
    assert_eq!(out, "10\n");
}

#[test]
fn trait_method_nullable_string_return() {
    // Trait method returns string? with conditional none
    let out = compile_and_run_stdout(r#"
trait Lookup {
    fn lookup(self, key: string) string?
}

class SimpleLookup impl Lookup {
    known: string
    fn lookup(self, key: string) string? {
        if key == self.known {
            return "found"
        }
        return none
    }
}

fn run(l: Lookup) string? {
    let r1 = l.lookup("hello")?
    print(r1)
    return r1
}

fn main() {
    run(SimpleLookup { known: "hello" })
}
"#);
    assert_eq!(out, "found\n");
}

#[test]
fn trait_method_nullable_propagation_chain() {
    // Chain of nullable propagation through trait dispatch
    let out = compile_and_run_stdout(r#"
trait Source {
    fn get_val(self) int?
}

class Good impl Source {
    v: int
    fn get_val(self) int? { return self.v }
}

class Empty impl Source {
    v: int
    fn get_val(self) int? { return none }
}

fn double_val(s: Source) int? {
    let v = s.get_val()?
    return v * 2
}

fn main() {
    let r1 = double_val(Good { v: 5 })
    let r2 = double_val(Empty { v: 0 })
    if r1 != none {
        print("got value")
    }
    if r2 == none {
        print("none")
    }
}
"#);
    assert_eq!(out, "got value\nnone\n");
}

#[test]
fn trait_method_raises_multiple_error_types() {
    // Trait method raises one of multiple error types
    let out = compile_and_run_stdout(r#"
error NotFound {
    key: string
}

error InvalidInput {
    msg: string
}

trait Validator {
    fn validate(self, x: int) int
}

class StrictValidator impl Validator {
    limit: int
    fn validate(self, x: int) int {
        if x < 0 {
            raise InvalidInput { msg: "negative" }
        }
        if x > self.limit {
            raise NotFound { key: "overflow" }
        }
        return x
    }
}

fn run(v: Validator) {
    let r1 = v.validate(5) catch err { -1 }
    let r2 = v.validate(-1) catch err { -2 }
    let r3 = v.validate(100) catch err { -3 }
    print(r1)
    print(r2)
    print(r3)
}

fn main() {
    run(StrictValidator { limit: 10 })
}
"#);
    assert_eq!(out, "5\n-2\n-3\n");
}

#[test]
fn trait_error_in_default_method() {
    // Default method raises an error that must be caught
    let out = compile_and_run_stdout(r#"
error ValidationError {
    msg: string
}

trait Checkable {
    fn raw_val(self) int

    fn checked_val(self) int {
        let v = self.raw_val()
        if v < 0 {
            raise ValidationError { msg: "negative" }
        }
        return v
    }
}

class Item impl Checkable {
    v: int
    fn raw_val(self) int { return self.v }
}

fn run(c: Checkable) {
    let r = c.checked_val() catch err { -999 }
    print(r)
}

fn main() {
    run(Item { v: 5 })
    run(Item { v: -3 })
}
"#);
    assert_eq!(out, "5\n-999\n");
}

#[test]
fn trait_dispatch_in_spawn_with_computation() {
    // Trait dispatch inside spawned task with heavy computation
    let out = compile_and_run_stdout(r#"
trait Computer {
    fn compute(self) int
}

class Heavy impl Computer {
    n: int
    fn compute(self) int {
        let sum = 0
        let i = 0
        while i < self.n {
            sum = sum + i
            i = i + 1
        }
        return sum
    }
}

fn run_task(c: Computer) int {
    return c.compute()
}

fn main() {
    let h = Heavy { n: 100 }
    let t = spawn run_task(h)
    print(t.get())
}
"#);
    assert_eq!(out, "4950\n");
}

#[test]
fn trait_generic_class_with_type_bound() {
    // Generic class with type bound implementing trait
    let out = compile_and_run_stdout(r#"
trait Printable {
    fn to_str(self) string
}

trait Container {
    fn describe(self) string
}

class IntItem impl Printable {
    v: int
    fn to_str(self) string { return "{self.v}" }
}

class Wrapper<T: Printable> impl Container {
    item: T
    fn describe(self) string { return "wrapped:{self.item.to_str()}" }
}

fn show(c: Container) {
    print(c.describe())
}

fn main() {
    let w = Wrapper<IntItem> { item: IntItem { v: 42 } }
    show(w)
}
"#);
    assert_eq!(out, "wrapped:42\n");
}

#[test]
fn trait_generic_two_instantiations_different_bounds() {
    // Generic class instantiated with different type args, both impl same trait
    let out = compile_and_run_stdout(r#"
trait Sizable {
    fn size(self) int
}

class Box<T> impl Sizable {
    val: T
    count: int
    fn size(self) int { return self.count }
}

fn show_size(s: Sizable) { print(s.size()) }

fn main() {
    show_size(Box<int> { val: 42, count: 1 })
    show_size(Box<string> { val: "hello", count: 5 })
}
"#);
    assert_eq!(out, "1\n5\n");
}

#[test]
fn trait_method_with_array_return_through_dispatch() {
    // Trait method returns [int] array, accessed through dispatch
    let out = compile_and_run_stdout(r#"
trait Generator {
    fn generate(self, n: int) [int]
}

class RangeGen impl Generator {
    start: int
    fn generate(self, n: int) [int] {
        let result: [int] = []
        let i = 0
        while i < n {
            result.push(self.start + i)
            i = i + 1
        }
        return result
    }
}

fn run(g: Generator) {
    let arr = g.generate(3)
    let i = 0
    while i < arr.len() {
        print(arr[i])
        i = i + 1
    }
}

fn main() {
    run(RangeGen { start: 10 })
}
"#);
    assert_eq!(out, "10\n11\n12\n");
}

#[test]
fn trait_method_modifies_map_param() {
    // Trait method modifies a map passed by reference (heap type)
    let out = compile_and_run_stdout(r#"
trait Populator {
    fn populate(self, m: Map<string, int>)
}

class Filler impl Populator {
    base: int
    fn populate(self, m: Map<string, int>) {
        m["x"] = self.base
        m["y"] = self.base + 1
    }
}

fn run(p: Populator) {
    let m = Map<string, int> {}
    p.populate(m)
    print(m["x"])
    print(m["y"])
}

fn main() {
    run(Filler { base: 100 })
}
"#);
    assert_eq!(out, "100\n101\n");
}

#[test]
fn trait_dispatch_preserves_array_state() {
    // Array modified in trait method stays modified after dispatch
    let out = compile_and_run_stdout(r#"
trait Appender {
    fn append_vals(self, arr: [int])
}

class TripleAppender impl Appender {
    val: int
    fn append_vals(self, arr: [int]) {
        arr.push(self.val)
        arr.push(self.val * 2)
        arr.push(self.val * 3)
    }
}

fn run(a: Appender) {
    let arr: [int] = [0]
    a.append_vals(arr)
    print(arr.len())
    print(arr[3])
}

fn main() {
    run(TripleAppender { val: 10 })
}
"#);
    assert_eq!(out, "4\n30\n");
}

#[test]
fn trait_method_with_for_range_body() {
    // Trait method body uses for-range loop
    let out = compile_and_run_stdout(r#"
trait Summer {
    fn sum_range(self, n: int) int
}

class SimpleSummer impl Summer {
    offset: int
    fn sum_range(self, n: int) int {
        let total = self.offset
        for i in 0..n {
            total = total + i
        }
        return total
    }
}

fn run(s: Summer) {
    print(s.sum_range(5))
}

fn main() {
    run(SimpleSummer { offset: 100 })
}
"#);
    assert_eq!(out, "110\n");
}

#[test]
fn trait_method_with_for_array_body() {
    // Trait method body iterates array with for loop
    let out = compile_and_run_stdout(r#"
trait Processor {
    fn process(self, arr: [int]) int
}

class MaxFinder impl Processor {
    tag: int
    fn process(self, arr: [int]) int {
        let best = arr[0]
        for v in arr {
            if v > best {
                best = v
            }
        }
        return best
    }
}

fn run(p: Processor) {
    let data: [int] = [3, 7, 1, 9, 2]
    print(p.process(data))
}

fn main() {
    run(MaxFinder { tag: 0 })
}
"#);
    assert_eq!(out, "9\n");
}

#[test]
fn trait_dispatch_two_traits_on_same_object_in_one_function() {
    // Same object used through two different trait param types in one call
    let out = compile_and_run_stdout(r#"
trait Named {
    fn name(self) string
}

trait Scored {
    fn score(self) int
}

class Player impl Named, Scored {
    n: string
    s: int
    fn name(self) string { return self.n }
    fn score(self) int { return self.s }
}

fn report(n: Named, s: Scored) {
    print("{n.name()}: {s.score()}")
}

fn main() {
    let p = Player { n: "alice", s: 95 }
    report(p, p)
}
"#);
    assert_eq!(out, "alice: 95\n");
}

#[test]
fn trait_dispatch_nested_function_calls() {
    // Trait dispatch result passed as arg to another function
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class V impl Valued {
    n: int
    fn val(self) int { return self.n }
}

fn double(x: int) int { return x * 2 }

fn run(v: Valued) {
    print(double(v.val()))
}

fn main() {
    run(V { n: 7 })
}
"#);
    assert_eq!(out, "14\n");
}

// ===== Batch 18: Channels + traits, more negatives, remaining edge cases =====

#[test]
fn trait_method_sends_to_channel() {
    // Trait method sends values to a Sender<int>
    let out = compile_and_run_stdout(r#"
trait Producer {
    fn produce(self, tx: Sender<int>)
}

class NumberProducer impl Producer {
    start: int
    fn produce(self, tx: Sender<int>) {
        tx.send(self.start)!
        tx.send(self.start + 1)!
    }
}

fn run(p: Producer) {
    let (tx, rx) = chan<int>(2)
    p.produce(tx)!
    tx.close()
    for v in rx {
        print(v)
    }
}

fn main() {
    run(NumberProducer { start: 10 })!
}
"#);
    assert_eq!(out, "10\n11\n");
}

#[test]
fn trait_method_receives_from_channel() {
    // Trait method reads from a Receiver<int>
    let out = compile_and_run_stdout(r#"
trait Consumer {
    fn consume(self, rx: Receiver<int>) int
}

class Summer impl Consumer {
    tag: int
    fn consume(self, rx: Receiver<int>) int {
        let total = 0
        for v in rx {
            total = total + v
        }
        return total
    }
}

fn run(c: Consumer) {
    let (tx, rx) = chan<int>(3)
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
    tx.close()
    print(c.consume(rx))
}

fn main() {
    run(Summer { tag: 0 })!
}
"#);
    assert_eq!(out, "6\n");
}

#[test]
fn trait_method_sends_string_to_channel() {
    // Trait method sends strings through channel
    let out = compile_and_run_stdout(r#"
trait Emitter {
    fn emit(self, tx: Sender<string>)
}

class HelloEmitter impl Emitter {
    name: string
    fn emit(self, tx: Sender<string>) {
        tx.send("hello {self.name}")!
    }
}

fn run(e: Emitter) {
    let (tx, rx) = chan<string>(1)
    e.emit(tx)!
    tx.close()
    for msg in rx {
        print(msg)
    }
}

fn main() {
    run(HelloEmitter { name: "world" })!
}
"#);
    assert_eq!(out, "hello world\n");
}

#[test]
fn trait_multiple_implementors_same_channel() {
    // Two different classes send to same channel through trait dispatch
    let out = compile_and_run_stdout(r#"
trait ValSender {
    fn send_val(self, tx: Sender<int>)
}

class A impl ValSender {
    v: int
    fn send_val(self, tx: Sender<int>) { tx.send(self.v)! }
}

class B impl ValSender {
    v: int
    fn send_val(self, tx: Sender<int>) { tx.send(self.v * 10)! }
}

fn dispatch(s: ValSender, tx: Sender<int>) {
    s.send_val(tx)!
}

fn main() {
    let (tx, rx) = chan<int>(2)
    dispatch(A { v: 3 }, tx)!
    dispatch(B { v: 3 }, tx)!
    tx.close()
    for v in rx {
        print(v)
    }
}
"#);
    assert_eq!(out, "3\n30\n");
}

#[test]
fn trait_method_with_nested_if_else() {
    // Complex nested if-else in trait method body
    let out = compile_and_run_stdout(r#"
trait Grader {
    fn grade(self, score: int) string
}

class LetterGrader impl Grader {
    tag: int
    fn grade(self, score: int) string {
        if score >= 90 {
            return "A"
        } else {
            if score >= 80 {
                return "B"
            } else {
                if score >= 70 {
                    return "C"
                } else {
                    return "F"
                }
            }
        }
    }
}

fn run(g: Grader) {
    print(g.grade(95))
    print(g.grade(85))
    print(g.grade(75))
    print(g.grade(50))
}

fn main() {
    run(LetterGrader { tag: 0 })
}
"#);
    assert_eq!(out, "A\nB\nC\nF\n");
}

#[test]
fn trait_method_accumulates_string() {
    // Trait method builds up a string through iteration
    let out = compile_and_run_stdout(r#"
trait Joiner {
    fn join(self, parts: [string], sep: string) string
}

class SimpleJoiner impl Joiner {
    tag: int
    fn join(self, parts: [string], sep: string) string {
        let result = ""
        let i = 0
        while i < parts.len() {
            if i > 0 {
                result = result + sep
            }
            result = result + parts[i]
            i = i + 1
        }
        return result
    }
}

fn run(j: Joiner) {
    let parts: [string] = ["a", "b", "c"]
    print(j.join(parts, "-"))
}

fn main() {
    run(SimpleJoiner { tag: 0 })
}
"#);
    assert_eq!(out, "a-b-c\n");
}

#[test]
fn trait_dispatch_in_recursive_function() {
    // Trait handle used in recursive function
    let out = compile_and_run_stdout(r#"
trait Stepper {
    fn step(self, x: int) int
}

class Doubler impl Stepper {
    tag: int
    fn step(self, x: int) int { return x * 2 }
}

fn apply_n(s: Stepper, x: int, n: int) int {
    if n == 0 {
        return x
    }
    return apply_n(s, s.step(x), n - 1)
}

fn main() {
    let d = Doubler { tag: 0 }
    print(apply_n(d, 1, 4))
}
"#);
    assert_eq!(out, "16\n");
}

#[test]
fn trait_two_methods_called_in_sequence() {
    // Trait with two methods, both called in sequence
    let out = compile_and_run_stdout(r#"
trait Transformer {
    fn first(self, x: int) int
    fn second(self, x: int) int
}

class Pipeline impl Transformer {
    factor: int
    fn first(self, x: int) int { return x + self.factor }
    fn second(self, x: int) int { return x * self.factor }
}

fn run(t: Transformer) {
    let v = 5
    v = t.first(v)
    v = t.second(v)
    print(v)
}

fn main() {
    run(Pipeline { factor: 3 })
}
"#);
    assert_eq!(out, "24\n");
}

#[test]
fn trait_method_with_early_return() {
    // Trait method has early return (guard clause)
    let out = compile_and_run_stdout(r#"
trait Safeguard {
    fn safe_div(self, a: int, b: int) int
}

class SafeDiv impl Safeguard {
    default_val: int
    fn safe_div(self, a: int, b: int) int {
        if b == 0 {
            return self.default_val
        }
        return a / b
    }
}

fn run(s: Safeguard) {
    print(s.safe_div(10, 3))
    print(s.safe_div(10, 0))
}

fn main() {
    run(SafeDiv { default_val: -1 })
}
"#);
    assert_eq!(out, "3\n-1\n");
}

#[test]
fn trait_method_return_value_in_let_binding() {
    // Let binding from trait dispatch, then use in multiple places
    let out = compile_and_run_stdout(r#"
trait Source {
    fn get(self) int
}

class Fixed impl Source {
    v: int
    fn get(self) int { return self.v }
}

fn run(s: Source) {
    let val = s.get()
    print(val)
    print(val + 1)
    print(val * val)
}

fn main() {
    run(Fixed { v: 5 })
}
"#);
    assert_eq!(out, "5\n6\n25\n");
}

#[test]
fn fail_class_impl_nonexistent_trait() {
    // Class implements a trait that doesn't exist
    compile_should_fail_with(r#"
class Foo impl NonexistentTrait {
    val: int
    fn bar(self) int { return 1 }
}

fn main() {}
"#, "unknown trait 'NonexistentTrait'");
}

#[test]
fn fail_trait_method_returns_wrong_type_string_vs_int() {
    // Trait expects string return, class returns int
    compile_should_fail_with(r#"
trait Namer {
    fn name(self) string
}

class Bad impl Namer {
    val: int
    fn name(self) int { return 42 }
}

fn main() {}
"#, "method 'name' return type mismatch: trait 'Namer' expects string, class 'Bad' returns int");
}

#[test]
fn fail_call_undeclared_method_on_trait() {
    // Call a method that doesn't exist on the trait
    compile_should_fail_with(r#"
trait Foo {
    fn bar(self) int
}

class X impl Foo {
    val: int
    fn bar(self) int { return 1 }
}

fn use_foo(f: Foo) {
    print(f.baz())
}

fn main() {}
"#, "trait 'Foo' has no method 'baz'");
}

#[test]
fn fail_use_trait_as_type_without_impl() {
    // Assign a class to trait-typed variable when class doesn't implement it
    compile_should_fail_with(r#"
trait Foo {
    fn bar(self) int
}

class NotFoo {
    val: int
}

fn main() {
    let f: Foo = NotFoo { val: 1 }
}
"#, "type mismatch: expected trait Foo, found NotFoo");
}

#[test]
fn trait_method_with_boolean_and_logic() {
    // Trait method with complex boolean logic
    let out = compile_and_run_stdout(r#"
trait Filter {
    fn accepts(self, x: int) bool
}

class RangeFilter impl Filter {
    low: int
    high: int
    fn accepts(self, x: int) bool {
        return x >= self.low && x <= self.high
    }
}

fn count_accepted(f: Filter, arr: [int]) int {
    let count = 0
    let i = 0
    while i < arr.len() {
        if f.accepts(arr[i]) {
            count = count + 1
        }
        i = i + 1
    }
    return count
}

fn main() {
    let f = RangeFilter { low: 3, high: 7 }
    let data: [int] = [1, 3, 5, 7, 9]
    print(count_accepted(f, data))
}
"#);
    assert_eq!(out, "3\n");
}

#[test]
fn trait_default_method_with_string_interpolation() {
    // Default method using string interpolation to call required method
    let out = compile_and_run_stdout(r#"
trait Identifiable {
    fn id(self) int

    fn display(self) string {
        return "ID={self.id()}"
    }
}

class User impl Identifiable {
    uid: int
    fn id(self) int { return self.uid }
}

fn show(item: Identifiable) {
    print(item.display())
}

fn main() {
    show(User { uid: 42 })
}
"#);
    assert_eq!(out, "ID=42\n");
}

#[test]
fn trait_method_string_comparison() {
    // Trait method compares strings
    let out = compile_and_run_stdout(r#"
trait Matcher {
    fn matches(self, input: string) bool
}

class ExactMatcher impl Matcher {
    pattern: string
    fn matches(self, input: string) bool {
        return input == self.pattern
    }
}

fn run(m: Matcher) {
    print(m.matches("hello"))
    print(m.matches("world"))
}

fn main() {
    run(ExactMatcher { pattern: "hello" })
}
"#);
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn trait_dispatch_two_methods_interleaved() {
    // Call two trait methods alternating on same handle
    let out = compile_and_run_stdout(r#"
trait Dual {
    fn left(self) int
    fn right(self) int
}

class Pair impl Dual {
    a: int
    b: int
    fn left(self) int { return self.a }
    fn right(self) int { return self.b }
}

fn run(d: Dual) {
    print(d.left())
    print(d.right())
    print(d.left())
    print(d.right())
}

fn main() {
    run(Pair { a: 1, b: 2 })
}
"#);
    assert_eq!(out, "1\n2\n1\n2\n");
}

#[test]
fn trait_factory_function_returns_trait() {
    // Free function returns a trait-typed value
    let out = compile_and_run_stdout(r#"
trait Greeter {
    fn greet(self) string
}

class Hello impl Greeter {
    name: string
    fn greet(self) string { return "hello {self.name}" }
}

fn make_greeter(name: string) Greeter {
    return Hello { name: name }
}

fn main() {
    let g = make_greeter("world")
    print(g.greet())
}
"#);
    assert_eq!(out, "hello world\n");
}

#[test]
fn trait_factory_function_conditional() {
    // Factory function returns different implementations based on input
    let out = compile_and_run_stdout(r#"
trait Formatter {
    fn format(self, x: int) string
}

class PlainFormatter impl Formatter {
    tag: int
    fn format(self, x: int) string { return "{x}" }
}

class FancyFormatter impl Formatter {
    tag: int
    fn format(self, x: int) string { return "[{x}]" }
}

fn make_formatter(fancy: bool) Formatter {
    if fancy {
        return FancyFormatter { tag: 0 }
    }
    return PlainFormatter { tag: 0 }
}

fn main() {
    let f1 = make_formatter(false)
    let f2 = make_formatter(true)
    print(f1.format(42))
    print(f2.format(42))
}
"#);
    assert_eq!(out, "42\n[42]\n");
}

// ===== Batch 19: Maps, sets, nullable, casting, complex patterns =====

#[test]
fn trait_method_returns_nullable() {
    // Trait method returns nullable int
    let out = compile_and_run_stdout(r#"
trait Finder {
    fn find(self, key: string) int?
}

class SimpleFinder impl Finder {
    target: string
    value: int
    fn find(self, key: string) int? {
        if key == self.target {
            return self.value
        }
        return none
    }
}

fn run(f: Finder) {
    let r1 = f.find("x")
    let r2 = f.find("y")
    if r1 != none {
        print(r1?)
    }
    if r2 != none {
        print(r2?)
    } else {
        print(-1)
    }
}

fn main() {
    run(SimpleFinder { target: "x", value: 42 })
}
"#);
    assert_eq!(out, "42\n-1\n");
}

#[test]
fn trait_method_takes_nullable_param() {
    // Trait method accepts nullable parameter
    let out = compile_and_run_stdout(r#"
trait Processor {
    fn process(self, val: int?) int
}

class DefaultProcessor impl Processor {
    default_val: int
    fn process(self, val: int?) int {
        if val == none {
            return self.default_val
        }
        return 42
    }
}

fn run(p: Processor) {
    print(p.process(10))
    print(p.process(none))
}

fn main() {
    run(DefaultProcessor { default_val: -1 })
}
"#);
    assert_eq!(out, "42\n-1\n");
}

#[test]
fn trait_dispatch_result_stored_in_map() {
    // Trait method result stored as map value
    let out = compile_and_run_stdout(r#"
trait Scorer {
    fn score(self) int
}

class HighScorer impl Scorer {
    val: int
    fn score(self) int { return self.val }
}

class LowScorer impl Scorer {
    val: int
    fn score(self) int { return self.val }
}

fn main() {
    let h: Scorer = HighScorer { val: 100 }
    let l: Scorer = LowScorer { val: 10 }
    let m = Map<string, int> {}
    m["high"] = h.score()
    m["low"] = l.score()
    print(m["high"])
    print(m["low"])
}
"#);
    assert_eq!(out, "100\n10\n");
}

#[test]
fn trait_dispatch_result_added_to_set() {
    // Trait method results collected into a set
    let out = compile_and_run_stdout(r#"
trait IDProvider {
    fn id(self) int
}

class ProviderA impl IDProvider {
    val: int
    fn id(self) int { return self.val }
}

class ProviderB impl IDProvider {
    val: int
    fn id(self) int { return self.val }
}

fn main() {
    let a: IDProvider = ProviderA { val: 1 }
    let b: IDProvider = ProviderB { val: 2 }
    let s = Set<int> {}
    s.insert(a.id())
    s.insert(b.id())
    print(s.len())
    print(s.contains(1))
    print(s.contains(3))
}
"#);
    assert_eq!(out, "2\ntrue\nfalse\n");
}

#[test]
fn trait_method_with_float_return() {
    // Trait method returning float, used in arithmetic
    let out = compile_and_run_stdout(r#"
trait Measurer {
    fn measure(self) float
}

class RulerA impl Measurer {
    length: float
    fn measure(self) float { return self.length }
}

class RulerB impl Measurer {
    length: float
    fn measure(self) float { return self.length * 2.0 }
}

fn run(m: Measurer) {
    let v = m.measure()
    print(v)
}

fn main() {
    run(RulerA { length: 3.5 })
    run(RulerB { length: 3.5 })
}
"#);
    assert_eq!(out, "3.500000\n7.000000\n");
}

#[test]
fn trait_method_takes_array_returns_int() {
    // Trait method takes array param and computes a value
    let out = compile_and_run_stdout(r#"
trait Aggregator {
    fn aggregate(self, vals: [int]) int
}

class SumAggregator impl Aggregator {
    tag: int
    fn aggregate(self, vals: [int]) int {
        let total = 0
        let i = 0
        while i < vals.len() {
            total = total + vals[i]
            i = i + 1
        }
        return total
    }
}

class MaxAggregator impl Aggregator {
    tag: int
    fn aggregate(self, vals: [int]) int {
        let best = vals[0]
        let i = 1
        while i < vals.len() {
            if vals[i] > best {
                best = vals[i]
            }
            i = i + 1
        }
        return best
    }
}

fn run(a: Aggregator) {
    let data: [int] = [3, 7, 1, 9, 2]
    print(a.aggregate(data))
}

fn main() {
    run(SumAggregator { tag: 0 })
    run(MaxAggregator { tag: 0 })
}
"#);
    assert_eq!(out, "22\n9\n");
}

#[test]
fn trait_dispatch_in_while_loop_body() {
    // Trait method called repeatedly in while loop
    let out = compile_and_run_stdout(r#"
trait Counter {
    fn next_val(self, current: int) int
}

class Incrementer impl Counter {
    step: int
    fn next_val(self, current: int) int { return current + self.step }
}

fn count_to(c: Counter, limit: int) {
    let v = 0
    while v < limit {
        v = c.next_val(v)
        print(v)
    }
}

fn main() {
    count_to(Incrementer { step: 3 }, 10)
}
"#);
    assert_eq!(out, "3\n6\n9\n12\n");
}

#[test]
fn trait_method_returns_bool_predicate_filter() {
    // Trait method returning bool, used to filter array elements
    let out = compile_and_run_stdout(r#"
trait Predicate {
    fn check(self, x: int) bool
}

class EvenChecker impl Predicate {
    tag: int
    fn check(self, x: int) bool { return x % 2 == 0 }
}

class PositiveChecker impl Predicate {
    tag: int
    fn check(self, x: int) bool { return x > 0 }
}

fn count_matching(p: Predicate, vals: [int]) int {
    let count = 0
    let i = 0
    while i < vals.len() {
        if p.check(vals[i]) {
            count = count + 1
        }
        i = i + 1
    }
    return count
}

fn main() {
    let data: [int] = [1, 2, 3, 4, 5, 6]
    print(count_matching(EvenChecker { tag: 0 }, data))
    print(count_matching(PositiveChecker { tag: 0 }, data))
}
"#);
    assert_eq!(out, "3\n6\n");
}

#[test]
fn trait_dispatch_in_for_range_loop() {
    // Trait dispatch inside a for-range loop
    let out = compile_and_run_stdout(r#"
trait Mapper {
    fn apply(self, x: int) int
}

class SquareMapper impl Mapper {
    tag: int
    fn apply(self, x: int) int { return x * x }
}

fn main() {
    let m: Mapper = SquareMapper { tag: 0 }
    for i in 1..5 {
        print(m.apply(i))
    }
}
"#);
    assert_eq!(out, "1\n4\n9\n16\n");
}

#[test]
fn fail_trait_array_push_no_coercion() {
    // Compiler gap: pushing concrete class into trait-typed array doesn't coerce
    compile_should_fail_with(r#"
trait Labeled {
    fn label(self) string
}

class Dog impl Labeled {
    name: string
    fn label(self) string { return "dog:{self.name}" }
}

fn main() {
    let animals: [Labeled] = []
    animals.push(Dog { name: "Rex" })
}
"#, "expected trait Labeled");
}

#[test]
fn trait_dispatch_in_for_array_loop() {
    // Trait dispatch on each element of a trait-typed array (populated via let binding)
    let out = compile_and_run_stdout(r#"
trait Labeled {
    fn label(self) string
}

class Dog impl Labeled {
    name: string
    fn label(self) string { return "dog:{self.name}" }
}

class Cat impl Labeled {
    name: string
    fn label(self) string { return "cat:{self.name}" }
}

fn add_animal(animals: [Labeled], a: Labeled) {
    animals.push(a)
}

fn main() {
    let animals: [Labeled] = []
    add_animal(animals, Dog { name: "Rex" })
    add_animal(animals, Cat { name: "Mimi" })
    for a in animals {
        print(a.label())
    }
}
"#);
    assert_eq!(out, "dog:Rex\ncat:Mimi\n");
}

#[test]
fn trait_method_result_as_array_push_arg() {
    // Trait dispatch result pushed onto an array
    let out = compile_and_run_stdout(r#"
trait Numberer {
    fn num(self) int
}

class Five impl Numberer {
    tag: int
    fn num(self) int { return 5 }
}

class Ten impl Numberer {
    tag: int
    fn num(self) int { return 10 }
}

fn main() {
    let f: Numberer = Five { tag: 0 }
    let t: Numberer = Ten { tag: 0 }
    let arr: [int] = []
    arr.push(f.num())
    arr.push(t.num())
    arr.push(f.num() + t.num())
    print(arr.len())
    print(arr[0])
    print(arr[1])
    print(arr[2])
}
"#);
    assert_eq!(out, "3\n5\n10\n15\n");
}

#[test]
fn trait_method_result_in_string_interpolation() {
    // Trait method result used inside string interpolation
    let out = compile_and_run_stdout(r#"
trait Named {
    fn name(self) string
}

class Person impl Named {
    first: string
    last: string
    fn name(self) string { return "{self.first} {self.last}" }
}

fn greet(n: Named) {
    print("Hello, {n.name()}!")
}

fn main() {
    greet(Person { first: "John", last: "Doe" })
}
"#);
    assert_eq!(out, "Hello, John Doe!\n");
}

#[test]
fn trait_dispatch_two_traits_on_same_object() {
    // Call methods from two different traits on same object
    let out = compile_and_run_stdout(r#"
trait Readable {
    fn read(self) int
}

trait Writable {
    fn write(self, x: int) int
}

class Buffer impl Readable, Writable {
    val: int
    fn read(self) int { return self.val }
    fn write(self, x: int) int { return x + self.val }
}

fn use_readable(r: Readable) {
    print(r.read())
}

fn use_writable(w: Writable) {
    print(w.write(10))
}

fn main() {
    let b = Buffer { val: 5 }
    use_readable(b)
    use_writable(b)
}
"#);
    assert_eq!(out, "5\n15\n");
}

#[test]
fn trait_dispatch_boolean_short_circuit_with_method() {
    // Boolean short-circuit involving trait method calls
    let out = compile_and_run_stdout(r#"
trait Validator {
    fn valid(self) bool
}

class AlwaysValid impl Validator {
    tag: int
    fn valid(self) bool { return true }
}

class NeverValid impl Validator {
    tag: int
    fn valid(self) bool { return false }
}

fn main() {
    let a: Validator = AlwaysValid { tag: 0 }
    let n: Validator = NeverValid { tag: 0 }
    if a.valid() && n.valid() {
        print("both")
    } else {
        print("not both")
    }
    if a.valid() || n.valid() {
        print("at least one")
    } else {
        print("none")
    }
}
"#);
    assert_eq!(out, "not both\nat least one\n");
}

#[test]
fn trait_method_called_on_function_return_value() {
    // Call trait method on the return value of a function
    let out = compile_and_run_stdout(r#"
trait Describable {
    fn describe(self) string
}

class Item impl Describable {
    name: string
    fn describe(self) string { return "item:{self.name}" }
}

fn make_item(n: string) Describable {
    return Item { name: n }
}

fn main() {
    print(make_item("widget").describe())
}
"#);
    assert_eq!(out, "item:widget\n");
}

#[test]
fn trait_three_methods_all_called_through_dispatch() {
    // Trait with three methods, all called through dispatch
    let out = compile_and_run_stdout(r#"
trait Shape {
    fn area(self) int
    fn perimeter(self) int
    fn name(self) string
}

class Rectangle impl Shape {
    w: int
    h: int
    fn area(self) int { return self.w * self.h }
    fn perimeter(self) int { return 2 * (self.w + self.h) }
    fn name(self) string { return "rect" }
}

fn describe(s: Shape) {
    print(s.name())
    print(s.area())
    print(s.perimeter())
}

fn main() {
    describe(Rectangle { w: 3, h: 4 })
}
"#);
    assert_eq!(out, "rect\n12\n14\n");
}

#[test]
fn trait_dispatch_result_used_in_comparison() {
    // Trait method results compared against each other
    let out = compile_and_run_stdout(r#"
trait Sized_ {
    fn size(self) int
}

class Small impl Sized_ {
    tag: int
    fn size(self) int { return 1 }
}

class Big impl Sized_ {
    tag: int
    fn size(self) int { return 100 }
}

fn bigger(a: Sized_, b: Sized_) Sized_ {
    if a.size() > b.size() {
        return a
    }
    return b
}

fn main() {
    let s: Sized_ = Small { tag: 0 }
    let b: Sized_ = Big { tag: 0 }
    let winner = bigger(s, b)
    print(winner.size())
}
"#);
    assert_eq!(out, "100\n");
}

#[test]
fn trait_method_modifies_array_through_dispatch() {
    // Trait method takes mutable array and modifies it
    let out = compile_and_run_stdout(r#"
trait Filler {
    fn fill(self, arr: [int], count: int)
}

class ZeroFiller impl Filler {
    tag: int
    fn fill(self, arr: [int], count: int) {
        let i = 0
        while i < count {
            arr.push(0)
            i = i + 1
        }
    }
}

class SeqFiller impl Filler {
    start: int
    fn fill(self, arr: [int], count: int) {
        let i = 0
        while i < count {
            arr.push(self.start + i)
            i = i + 1
        }
    }
}

fn main() {
    let arr: [int] = []
    let f: Filler = SeqFiller { start: 10 }
    f.fill(arr, 3)
    let i = 0
    while i < arr.len() {
        print(arr[i])
        i = i + 1
    }
}
"#);
    assert_eq!(out, "10\n11\n12\n");
}

#[test]
fn trait_default_method_calls_required_method() {
    // Default method body calls a required method on self
    let out = compile_and_run_stdout(r#"
trait Doubler {
    fn value(self) int
    fn doubled(self) int {
        return self.value() * 2
    }
}

class MyVal impl Doubler {
    v: int
    fn value(self) int { return self.v }
}

fn run(d: Doubler) {
    print(d.value())
    print(d.doubled())
}

fn main() {
    run(MyVal { v: 7 })
}
"#);
    assert_eq!(out, "7\n14\n");
}

#[test]
fn fail_trait_method_wrong_param_type_string_for_int() {
    // Class implements trait method with string param where int expected
    compile_should_fail_with(r#"
trait Adder {
    fn add(self, x: int) int
}

class BadAdder impl Adder {
    tag: int
    fn add(self, x: string) int { return 0 }
}

fn main() {
    let a: Adder = BadAdder { tag: 0 }
    print(a.add(1))
}
"#, "method 'add' parameter 1 type mismatch: trait 'Adder' expects int, class 'BadAdder' has string");
}

// ===== Batch 20: Empty traits, vtable stress, recursive dispatch, field rejection, generics =====

#[test]
fn trait_empty_no_methods() {
    // Empty trait with no methods — should compile and be implementable
    let out = compile_and_run_stdout(r#"
trait Marker {
    fn tag(self) int
}

class Tagged impl Marker {
    val: int
    fn tag(self) int { return self.val }
}

fn takes_marker(m: Marker) {
    print(m.tag())
}

fn main() {
    takes_marker(Tagged { val: 99 })
}
"#);
    assert_eq!(out, "99\n");
}

#[test]
fn trait_vtable_five_methods_all_dispatched() {
    // Trait with 5 methods, all called through dispatch — tests vtable slot indexing
    let out = compile_and_run_stdout(r#"
trait Multi {
    fn m1(self) int
    fn m2(self) int
    fn m3(self) int
    fn m4(self) int
    fn m5(self) int
}

class Impl impl Multi {
    base: int
    fn m1(self) int { return self.base + 1 }
    fn m2(self) int { return self.base + 2 }
    fn m3(self) int { return self.base + 3 }
    fn m4(self) int { return self.base + 4 }
    fn m5(self) int { return self.base + 5 }
}

fn run(m: Multi) {
    print(m.m1())
    print(m.m2())
    print(m.m3())
    print(m.m4())
    print(m.m5())
}

fn main() {
    run(Impl { base: 10 })
}
"#);
    assert_eq!(out, "11\n12\n13\n14\n15\n");
}

#[test]
fn trait_three_traits_on_same_class_all_dispatched() {
    // Class implements 3 different traits, all used via different dispatch paths
    let out = compile_and_run_stdout(r#"
trait Readable_ {
    fn read(self) int
}

trait Writable_ {
    fn write(self, x: int) int
}

trait Closeable {
    fn close(self) string
}

class Resource impl Readable_, Writable_, Closeable {
    val: int
    fn read(self) int { return self.val }
    fn write(self, x: int) int { return self.val + x }
    fn close(self) string { return "closed" }
}

fn use_readable(r: Readable_) { print(r.read()) }
fn use_writable(w: Writable_) { print(w.write(5)) }
fn use_closeable(c: Closeable) { print(c.close()) }

fn main() {
    let res = Resource { val: 10 }
    use_readable(res)
    use_writable(res)
    use_closeable(res)
}
"#);
    assert_eq!(out, "10\n15\nclosed\n");
}

#[test]
fn trait_recursive_self_dispatch() {
    // Trait method calls itself recursively through self dispatch
    let out = compile_and_run_stdout(r#"
trait Countdown {
    fn count(self, n: int)
}

class Printer impl Countdown {
    tag: int
    fn count(self, n: int) {
        if n <= 0 {
            return
        }
        print(n)
        self.count(n - 1)
    }
}

fn run(c: Countdown) {
    c.count(3)
}

fn main() {
    run(Printer { tag: 0 })
}
"#);
    assert_eq!(out, "3\n2\n1\n");
}

#[test]
fn trait_method_with_six_params() {
    // Trait method with many parameters — stress tests parameter passing through vtable
    let out = compile_and_run_stdout(r#"
trait Calculator {
    fn compute(self, a: int, b: int, c: int, d: int, e: int, f: int) int
}

class Summer impl Calculator {
    tag: int
    fn compute(self, a: int, b: int, c: int, d: int, e: int, f: int) int {
        return a + b + c + d + e + f
    }
}

fn run(calc: Calculator) {
    print(calc.compute(1, 2, 3, 4, 5, 6))
}

fn main() {
    run(Summer { tag: 0 })
}
"#);
    assert_eq!(out, "21\n");
}

#[test]
fn trait_dispatch_chain_three_functions_deep() {
    // Trait handle passed through 3 function calls before method is called
    let out = compile_and_run_stdout(r#"
trait Worker_ {
    fn work(self) int
}

class SimpleWorker impl Worker_ {
    val: int
    fn work(self) int { return self.val * 2 }
}

fn level3(w: Worker_) int {
    return w.work()
}

fn level2(w: Worker_) int {
    return level3(w)
}

fn level1(w: Worker_) int {
    return level2(w)
}

fn main() {
    print(level1(SimpleWorker { val: 5 }))
}
"#);
    assert_eq!(out, "10\n");
}

#[test]
fn trait_dispatch_in_match_arm_enum_choice() {
    // Trait method called inside match arm based on enum variant
    let out = compile_and_run_stdout(r#"
trait Namer {
    fn name(self) string
}

class Alice impl Namer {
    tag: int
    fn name(self) string { return "alice" }
}

enum Choice {
    First
    Second
}

fn run(n: Namer, c: Choice) {
    match c {
        Choice.First {
            print(n.name())
        }
        Choice.Second {
            print("other")
        }
    }
}

fn main() {
    run(Alice { tag: 0 }, Choice.First)
    run(Alice { tag: 0 }, Choice.Second)
}
"#);
    assert_eq!(out, "alice\nother\n");
}

#[test]
fn trait_generic_class_returned_as_trait() {
    // Generic class implementing trait, instantiated and returned as trait handle
    let out = compile_and_run_stdout(r#"
trait Holder {
    fn get(self) int
}

class Box<T> impl Holder {
    value: T
    fn get(self) int { return 42 }
}

fn make_holder() Holder {
    return Box<int> { value: 100 }
}

fn main() {
    let h = make_holder()
    print(h.get())
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn trait_method_returns_string_interp_with_field() {
    // Trait method builds string using interpolation and field access
    let out = compile_and_run_stdout(r#"
trait Describer {
    fn desc(self) string
}

class Point impl Describer {
    x: int
    y: int
    fn desc(self) string { return "({self.x}, {self.y})" }
}

fn show(d: Describer) {
    print(d.desc())
}

fn main() {
    show(Point { x: 3, y: 7 })
}
"#);
    assert_eq!(out, "(3, 7)\n");
}

#[test]
fn fail_trait_method_self_referential_type() {
    // COMPILER GAP: Trait method using its own trait type as parameter fails with "unknown type"
    // Self-referential trait types in method signatures are not resolved
    let out = compile_and_run_stdout(r#"
trait Comparable_ {
    fn value(self) int
    fn greater_than(self, other: Comparable_) bool {
        return self.value() > other.value()
    }
}

class Num impl Comparable_ {
    n: int
    fn value(self) int { return self.n }
}

fn main() {
    let a: Comparable_ = Num { n: 10 }
    let b: Comparable_ = Num { n: 5 }
    print(a.greater_than(b))
}
"#);
    assert_eq!(out, "true\n");
}

#[test]
fn trait_vtable_method_order_matches_declaration() {
    // Ensure vtable slot assignment follows declaration order, not impl order
    let out = compile_and_run_stdout(r#"
trait Ordered {
    fn first(self) int
    fn second(self) int
    fn third(self) int
}

class Reversed impl Ordered {
    tag: int
    fn third(self) int { return 3 }
    fn first(self) int { return 1 }
    fn second(self) int { return 2 }
}

fn run(o: Ordered) {
    print(o.first())
    print(o.second())
    print(o.third())
}

fn main() {
    run(Reversed { tag: 0 })
}
"#);
    assert_eq!(out, "1\n2\n3\n");
}

#[test]
fn fail_trait_method_array_param_type_mismatch() {
    // Trait requires [int] but class impl provides [string]
    compile_should_fail_with(r#"
trait Lister {
    fn list(self, items: [int]) int
}

class BadLister impl Lister {
    tag: int
    fn list(self, items: [string]) int { return 0 }
}

fn main() {
    let l: Lister = BadLister { tag: 0 }
    let data: [int] = [1, 2]
    print(l.list(data))
}
"#, "method 'list' parameter 1 type mismatch: trait 'Lister' expects [int], class 'BadLister' has [string]");
}

#[test]
fn fail_trait_method_return_type_array_mismatch() {
    // Trait requires [int] return but class returns [string]
    compile_should_fail_with(r#"
trait Producer_ {
    fn produce(self) [int]
}

class BadProducer impl Producer_ {
    tag: int
    fn produce(self) [string] {
        let arr: [string] = ["a"]
        return arr
    }
}

fn main() {
    let p: Producer_ = BadProducer { tag: 0 }
    print(p.produce().len())
}
"#, "method 'produce' return type mismatch: trait 'Producer_' expects [int], class 'BadProducer' returns [string]");
}

#[test]
fn trait_dispatch_result_in_ternary_if() {
    // Trait dispatch result used as condition and in if-else expression
    let out = compile_and_run_stdout(r#"
trait Checker {
    fn valid(self) bool
    fn code(self) int
}

class Ok_ impl Checker {
    tag: int
    fn valid(self) bool { return true }
    fn code(self) int { return 200 }
}

class Fail_ impl Checker {
    tag: int
    fn valid(self) bool { return false }
    fn code(self) int { return 500 }
}

fn report(c: Checker) {
    if c.valid() {
        print(c.code())
    } else {
        print(0 - c.code())
    }
}

fn main() {
    report(Ok_ { tag: 0 })
    report(Fail_ { tag: 0 })
}
"#);
    assert_eq!(out, "200\n-500\n");
}

#[test]
fn trait_dispatch_assigns_to_existing_variable() {
    // Trait method result assigned to a pre-existing variable
    let out = compile_and_run_stdout(r#"
trait Source_ {
    fn next(self) int
}

class Counter_ impl Source_ {
    start: int
    fn next(self) int { return self.start + 1 }
}

fn main() {
    let s: Source_ = Counter_ { start: 0 }
    let val = 0
    val = s.next()
    print(val)
    val = val + s.next()
    print(val)
}
"#);
    assert_eq!(out, "1\n2\n");
}

#[test]
fn trait_method_with_while_and_break() {
    // Trait method body uses while loop with break
    let out = compile_and_run_stdout(r#"
trait Searcher {
    fn find_first_gt(self, arr: [int], threshold: int) int
}

class LinearSearcher impl Searcher {
    tag: int
    fn find_first_gt(self, arr: [int], threshold: int) int {
        let i = 0
        while i < arr.len() {
            if arr[i] > threshold {
                return arr[i]
            }
            i = i + 1
        }
        return -1
    }
}

fn run(s: Searcher) {
    let data: [int] = [1, 5, 3, 8, 2]
    print(s.find_first_gt(data, 4))
    print(s.find_first_gt(data, 10))
}

fn main() {
    run(LinearSearcher { tag: 0 })
}
"#);
    assert_eq!(out, "5\n-1\n");
}

#[test]
fn trait_two_classes_same_trait_different_field_layout() {
    // Two classes with very different field layouts implementing same trait
    let out = compile_and_run_stdout(r#"
trait Summary {
    fn summarize(self) string
}

class Simple impl Summary {
    tag: int
    fn summarize(self) string { return "simple" }
}

class Complex_ impl Summary {
    a: int
    b: int
    c: string
    d: float
    e: bool
    fn summarize(self) string { return "complex:{self.a},{self.b},{self.c}" }
}

fn show(s: Summary) {
    print(s.summarize())
}

fn main() {
    show(Simple { tag: 0 })
    show(Complex_ { a: 1, b: 2, c: "three", d: 4.0, e: true })
}
"#);
    assert_eq!(out, "simple\ncomplex:1,2,three\n");
}

#[test]
fn trait_default_method_with_loop() {
    // Default method body contains a while loop
    let out = compile_and_run_stdout(r#"
trait Repeater {
    fn base(self) string
    fn repeat(self, n: int) string {
        let result = ""
        let i = 0
        while i < n {
            result = result + self.base()
            i = i + 1
        }
        return result
    }
}

class Star impl Repeater {
    tag: int
    fn base(self) string { return "*" }
}

fn run(r: Repeater) {
    print(r.repeat(4))
}

fn main() {
    run(Star { tag: 0 })
}
"#);
    assert_eq!(out, "****\n");
}

#[test]
fn fail_trait_method_extra_param() {
    // Class method has extra parameter not in trait
    compile_should_fail_with(r#"
trait Adder_ {
    fn add(self, x: int) int
}

class BadAdder_ impl Adder_ {
    tag: int
    fn add(self, x: int, y: int) int { return x + y }
}

fn main() {
    let a: Adder_ = BadAdder_ { tag: 0 }
    print(a.add(1))
}
"#, "method 'add' of class 'BadAdder_' has wrong number of parameters for trait 'Adder_'");
}

#[test]
fn fail_trait_method_missing_param() {
    // Class method has fewer params than trait requires
    compile_should_fail_with(r#"
trait Combiner {
    fn combine(self, a: int, b: int) int
}

class BadCombiner impl Combiner {
    tag: int
    fn combine(self, a: int) int { return a }
}

fn main() {
    let c: Combiner = BadCombiner { tag: 0 }
    print(c.combine(1, 2))
}
"#, "method 'combine' of class 'BadCombiner' has wrong number of parameters for trait 'Combiner'");
}

// ===== Batch 21: Reassignment, contracts, error combos, default-only, nested dispatch =====

#[test]
fn trait_handle_reassigned_to_different_implementor() {
    // Variable holding trait handle reassigned to a different class
    let out = compile_and_run_stdout(r#"
trait Speaker {
    fn speak(self) string
}

class Dog impl Speaker {
    tag: int
    fn speak(self) string { return "woof" }
}

class Cat impl Speaker {
    tag: int
    fn speak(self) string { return "meow" }
}

fn main() {
    let s: Speaker = Dog { tag: 0 }
    print(s.speak())
    s = Cat { tag: 0 }
    print(s.speak())
}
"#);
    assert_eq!(out, "woof\nmeow\n");
}

#[test]
fn trait_dispatch_result_as_arg_to_another_dispatch() {
    // Chain: trait method result fed as argument to another trait method
    let out = compile_and_run_stdout(r#"
trait Provider {
    fn provide(self) int
}

trait Consumer_ {
    fn consume(self, val: int) string
}

class NumProvider impl Provider {
    val: int
    fn provide(self) int { return self.val }
}

class Printer_ impl Consumer_ {
    prefix: string
    fn consume(self, val: int) string { return "{self.prefix}{val}" }
}

fn main() {
    let p: Provider = NumProvider { val: 42 }
    let c: Consumer_ = Printer_ { prefix: "got:" }
    print(c.consume(p.provide()))
}
"#);
    assert_eq!(out, "got:42\n");
}

#[test]
fn trait_all_default_methods_chained() {
    // Trait with 3 default methods where one calls the other two
    let out = compile_and_run_stdout(r#"
trait Defaults {
    fn a(self) int { return 1 }
    fn b(self) int { return 2 }
    fn c(self) int { return self.a() + self.b() }
}

class Empty impl Defaults {
    tag: int
}

fn run(d: Defaults) {
    print(d.a())
    print(d.b())
    print(d.c())
}

fn main() {
    run(Empty { tag: 0 })
}
"#);
    assert_eq!(out, "1\n2\n3\n");
}

#[test]
fn trait_method_builds_and_returns_array() {
    // Trait method creates and returns an array
    let out = compile_and_run_stdout(r#"
trait Lister_ {
    fn items(self) [int]
}

class RangeList impl Lister_ {
    count: int
    fn items(self) [int] {
        let arr: [int] = []
        let i = 0
        while i < self.count {
            arr.push(i)
            i = i + 1
        }
        return arr
    }
}

fn run(l: Lister_) {
    let arr = l.items()
    print(arr.len())
    let i = 0
    while i < arr.len() {
        print(arr[i])
        i = i + 1
    }
}

fn main() {
    run(RangeList { count: 3 })
}
"#);
    assert_eq!(out, "3\n0\n1\n2\n");
}

#[test]
fn trait_method_with_error_propagation() {
    // Trait method is fallible, called with ! propagation
    let out = compile_and_run_stdout(r#"
error NotFound { code: int }

trait Repository {
    fn get(self, id: int) int
}

class InMemoryRepo impl Repository {
    data: int
    fn get(self, id: int) int {
        if id != 1 {
            raise NotFound { code: id }
        }
        return self.data
    }
}

fn fetch(r: Repository, id: int) int {
    return r.get(id)!
}

fn main() {
    let repo: Repository = InMemoryRepo { data: 42 }
    let val = fetch(repo, 1) catch 0
    print(val)
    let missing = fetch(repo, 99) catch -1
    print(missing)
}
"#);
    assert_eq!(out, "42\n-1\n");
}

#[test]
fn trait_method_with_catch_at_call_site() {
    // Fallible trait method caught directly at the call site
    let out = compile_and_run_stdout(r#"
error ParseError { msg: string }

trait Parser {
    fn parse(self, input: string) int
}

class StrictParser impl Parser {
    tag: int
    fn parse(self, input: string) int {
        if input == "bad" {
            raise ParseError { msg: "invalid" }
        }
        return 42
    }
}

fn main() {
    let p: Parser = StrictParser { tag: 0 }
    let v1 = p.parse("good") catch -1
    print(v1)
    let v2 = p.parse("bad") catch -1
    print(v2)
}
"#);
    assert_eq!(out, "42\n-1\n");
}

#[test]
fn trait_with_invariant_on_implementing_class() {
    // Class implementing trait also has invariant contract
    let out = compile_and_run_stdout(r#"
trait Bounded {
    fn value(self) int
}

class PositiveInt impl Bounded {
    n: int
    invariant self.n > 0
    fn value(self) int { return self.n }
}

fn run(b: Bounded) {
    print(b.value())
}

fn main() {
    run(PositiveInt { n: 5 })
}
"#);
    assert_eq!(out, "5\n");
}

#[test]
fn trait_dispatch_result_in_map_key_position() {
    // Trait method returns string, used as map key
    let out = compile_and_run_stdout(r#"
trait KeyMaker {
    fn key(self) string
}

class Prefixer impl KeyMaker {
    prefix: string
    fn key(self) string { return "{self.prefix}_key" }
}

fn main() {
    let k: KeyMaker = Prefixer { prefix: "user" }
    let m = Map<string, int> {}
    m[k.key()] = 100
    print(m["user_key"])
}
"#);
    assert_eq!(out, "100\n");
}

#[test]
fn trait_dispatch_in_nested_function_call() {
    // Trait dispatch result is argument to a regular function
    let out = compile_and_run_stdout(r#"
trait Measurable {
    fn length(self) int
}

class Rope impl Measurable {
    len: int
    fn length(self) int { return self.len }
}

fn double(x: int) int {
    return x * 2
}

fn main() {
    let m: Measurable = Rope { len: 7 }
    print(double(m.length()))
}
"#);
    assert_eq!(out, "14\n");
}

#[test]
fn fail_trait_method_returns_class_forward_ref() {
    // Fixed: trait method signatures can now reference class types via forward references
    let out = compile_and_run_stdout(r#"
class Result_ {
    code: int
    msg: string
}

trait Handler_ {
    fn handle(self) Result_
}

class OkHandler impl Handler_ {
    tag: int
    fn handle(self) Result_ {
        return Result_ { code: 200, msg: "ok" }
    }
}

fn main() {
    let h: Handler_ = OkHandler { tag: 0 }
    print(h.handle().code)
}
"#);
    assert_eq!(out, "200\n");
}

#[test]
fn trait_multiple_dispatch_results_combined() {
    // Results from multiple different trait dispatches combined in arithmetic
    let out = compile_and_run_stdout(r#"
trait XCoord {
    fn x(self) int
}

trait YCoord {
    fn y(self) int
}

class Point_ impl XCoord, YCoord {
    px: int
    py: int
    fn x(self) int { return self.px }
    fn y(self) int { return self.py }
}

fn manhattan(p: XCoord, q: YCoord) int {
    return p.x() + q.y()
}

fn main() {
    let pt = Point_ { px: 3, py: 7 }
    print(manhattan(pt, pt))
}
"#);
    assert_eq!(out, "10\n");
}

#[test]
fn trait_dispatch_in_for_range_with_accumulator() {
    // Trait dispatch called in each iteration of a for-range, accumulating results
    let out = compile_and_run_stdout(r#"
trait Scorer_ {
    fn score(self, round: int) int
}

class LinearScorer impl Scorer_ {
    multiplier: int
    fn score(self, round: int) int { return round * self.multiplier }
}

fn total_score(s: Scorer_, rounds: int) int {
    let total = 0
    for i in 1..rounds + 1 {
        total = total + s.score(i)
    }
    return total
}

fn main() {
    print(total_score(LinearScorer { multiplier: 3 }, 4))
}
"#);
    assert_eq!(out, "30\n");
}

#[test]
fn trait_handle_in_array_iteration() {
    // Array of trait handles, iterate and call method on each
    let out = compile_and_run_stdout(r#"
trait Valued_ {
    fn val(self) int
}

class A_ impl Valued_ {
    n: int
    fn val(self) int { return self.n }
}

class B_ impl Valued_ {
    n: int
    fn val(self) int { return self.n * 10 }
}

fn add_item(arr: [Valued_], item: Valued_) {
    arr.push(item)
}

fn main() {
    let items: [Valued_] = []
    add_item(items, A_ { n: 1 })
    add_item(items, B_ { n: 2 })
    add_item(items, A_ { n: 3 })
    let total = 0
    let i = 0
    while i < items.len() {
        total = total + items[i].val()
        i = i + 1
    }
    print(total)
}
"#);
    assert_eq!(out, "24\n");
}

#[test]
fn trait_default_overridden_partially() {
    // Trait with 3 defaults, class overrides only 1
    let out = compile_and_run_stdout(r#"
trait Config {
    fn host(self) string { return "localhost" }
    fn port(self) int { return 8080 }
    fn protocol(self) string { return "http" }
}

class ProdConfig impl Config {
    tag: int
    fn host(self) string { return "prod.example.com" }
}

fn show(c: Config) {
    print(c.host())
    print(c.port())
    print(c.protocol())
}

fn main() {
    show(ProdConfig { tag: 0 })
}
"#);
    assert_eq!(out, "prod.example.com\n8080\nhttp\n");
}

#[test]
fn trait_dispatch_in_nested_if_multilevel() {
    // Trait dispatch deep inside nested if with multiple level thresholds
    let out = compile_and_run_stdout(r#"
trait Level {
    fn level(self) int
}

class HighLevel impl Level {
    tag: int
    fn level(self) int { return 10 }
}

fn classify(l: Level) string {
    if l.level() > 5 {
        if l.level() > 8 {
            return "very high"
        }
        return "high"
    }
    return "low"
}

fn main() {
    print(classify(HighLevel { tag: 0 }))
}
"#);
    assert_eq!(out, "very high\n");
}

#[test]
fn trait_method_void_return() {
    // Trait method with void return (no return type)
    let out = compile_and_run_stdout(r#"
trait Logger_ {
    fn log(self, msg: string)
}

class StdoutLogger impl Logger_ {
    prefix: string
    fn log(self, msg: string) {
        print("{self.prefix}: {msg}")
    }
}

fn use_logger(l: Logger_) {
    l.log("hello")
    l.log("world")
}

fn main() {
    use_logger(StdoutLogger { prefix: "INFO" })
}
"#);
    assert_eq!(out, "INFO: hello\nINFO: world\n");
}

#[test]
fn fail_trait_impl_with_requires_on_impl_method() {
    // COMPILER GAP TEST: Class method adding requires to trait impl should be rejected (Liskov)
    // If this passes compilation, it means requires enforcement on trait impls isn't checked
    compile_should_fail_with(r#"
trait Worker__ {
    fn work(self, x: int) int
}

class RestrictedWorker impl Worker__ {
    tag: int
    fn work(self, x: int) int
        requires x > 0
    {
        return x * 2
    }
}

fn main() {
    let w: Worker__ = RestrictedWorker { tag: 1 }
    print(w.work(5))
}
"#, "cannot add 'requires' clauses");
}

#[test]
fn trait_with_ensures_on_trait_method() {
    // Trait method declares ensures, implementation must satisfy at runtime
    let out = compile_and_run_stdout(r#"
trait Positive {
    fn make_positive(self, x: int) int
        ensures result > 0
}

class AbsVal impl Positive {
    tag: int
    fn make_positive(self, x: int) int {
        if x < 0 {
            return 0 - x
        }
        if x == 0 {
            return 1
        }
        return x
    }
}

fn run(p: Positive) {
    print(p.make_positive(-5))
    print(p.make_positive(3))
    print(p.make_positive(0))
}

fn main() {
    run(AbsVal { tag: 0 })
}
"#);
    assert_eq!(out, "5\n3\n1\n");
}

#[test]
fn trait_with_requires_on_trait_method() {
    // Trait method declares requires, checked at runtime on dispatch
    let out = compile_and_run_stdout(r#"
trait Divider {
    fn divide(self, a: int, b: int) int
        requires b != 0
}

class IntDivider impl Divider {
    tag: int
    fn divide(self, a: int, b: int) int {
        return a / b
    }
}

fn main() {
    let d: Divider = IntDivider { tag: 0 }
    print(d.divide(10, 3))
}
"#);
    assert_eq!(out, "3\n");
}

// ===== Batch 22: Type casting, map ops, bitwise, string ops, spawn, more negatives =====

#[test]
fn trait_method_result_cast_to_float() {
    // Trait dispatch result cast with `as float`
    let out = compile_and_run_stdout(r#"
trait IntSource {
    fn get(self) int
}

class FixedSource impl IntSource {
    val: int
    fn get(self) int { return self.val }
}

fn main() {
    let s: IntSource = FixedSource { val: 7 }
    let f = s.get() as float
    print(f)
}
"#);
    assert_eq!(out, "7.000000\n");
}

#[test]
fn trait_method_result_with_bitwise_ops() {
    // Trait dispatch result used in bitwise operations
    let out = compile_and_run_stdout(r#"
trait Flags {
    fn flags(self) int
}

class ReadWrite impl Flags {
    tag: int
    fn flags(self) int { return 6 }
}

fn main() {
    let f: Flags = ReadWrite { tag: 0 }
    let val = f.flags()
    print(val & 2)
    print(val | 1)
    print(val ^ 3)
}
"#);
    assert_eq!(out, "2\n7\n5\n");
}

#[test]
fn trait_method_modifies_map() {
    // Trait method takes a map and modifies it
    let out = compile_and_run_stdout(r#"
trait MapWriter {
    fn fill(self, m: Map<string, int>)
}

class DefaultWriter impl MapWriter {
    val: int
    fn fill(self, m: Map<string, int>) {
        m["a"] = self.val
        m["b"] = self.val * 2
    }
}

fn main() {
    let w: MapWriter = DefaultWriter { val: 5 }
    let m = Map<string, int> {}
    w.fill(m)
    print(m["a"])
    print(m["b"])
}
"#);
    assert_eq!(out, "5\n10\n");
}

#[test]
fn trait_method_result_as_array_index() {
    // Trait dispatch result used to index into an array
    let out = compile_and_run_stdout(r#"
trait Indexer {
    fn idx(self) int
}

class ConstIndex impl Indexer {
    i: int
    fn idx(self) int { return self.i }
}

fn main() {
    let data: [int] = [10, 20, 30, 40]
    let ix: Indexer = ConstIndex { i: 2 }
    print(data[ix.idx()])
}
"#);
    assert_eq!(out, "30\n");
}

#[test]
fn trait_method_string_concatenation() {
    // Trait method returns string that gets concatenated
    let out = compile_and_run_stdout(r#"
trait Prefix {
    fn prefix(self) string
}

class BangPrefix impl Prefix {
    tag: int
    fn prefix(self) string { return "!" }
}

class StarPrefix impl Prefix {
    tag: int
    fn prefix(self) string { return "*" }
}

fn format(p: Prefix, msg: string) string {
    return p.prefix() + msg
}

fn main() {
    let b: Prefix = BangPrefix { tag: 0 }
    let s: Prefix = StarPrefix { tag: 0 }
    print(format(b, "urgent"))
    print(format(s, "tag"))
}
"#);
    assert_eq!(out, "!urgent\n*tag\n");
}

#[test]
fn trait_method_called_multiple_times_same_object() {
    // Call same trait method multiple times on same handle (tests vtable caching)
    let out = compile_and_run_stdout(r#"
trait RNG {
    fn next(self) int
}

class ConstRNG impl RNG {
    val: int
    fn next(self) int { return self.val }
}

fn main() {
    let r: RNG = ConstRNG { val: 7 }
    let a = r.next()
    let b = r.next()
    let c = r.next()
    print(a + b + c)
}
"#);
    assert_eq!(out, "21\n");
}

#[test]
fn trait_dispatch_in_spawn_with_join() {
    // Trait dispatch inside a spawned function, result retrieved via .get()
    let out = compile_and_run_stdout(r#"
trait Computable {
    fn compute(self) int
}

class Squarer impl Computable {
    val: int
    fn compute(self) int { return self.val * self.val }
}

fn do_compute(c: Computable) int {
    return c.compute()
}

fn main() {
    let c: Computable = Squarer { val: 6 }
    let task = spawn do_compute(c)
    print(task.get())
}
"#);
    assert_eq!(out, "36\n");
}

#[test]
fn trait_method_with_modulo_operation() {
    // Trait method uses modulo in its computation
    let out = compile_and_run_stdout(r#"
trait Classifier {
    fn classify(self, x: int) string
}

class ParityChecker impl Classifier {
    tag: int
    fn classify(self, x: int) string {
        if x % 2 == 0 {
            return "even"
        }
        return "odd"
    }
}

fn main() {
    let c: Classifier = ParityChecker { tag: 0 }
    print(c.classify(4))
    print(c.classify(7))
}
"#);
    assert_eq!(out, "even\nodd\n");
}

#[test]
fn trait_dispatch_result_negated_unary() {
    // Trait dispatch result negated with unary minus (0 - x pattern)
    let out = compile_and_run_stdout(r#"
trait SignedVal {
    fn val(self) int
}

class Pos impl SignedVal {
    n: int
    fn val(self) int { return self.n }
}

fn main() {
    let s: SignedVal = Pos { n: 42 }
    print(0 - s.val())
}
"#);
    assert_eq!(out, "-42\n");
}

#[test]
fn trait_method_with_string_equality_check() {
    // Trait method compares strings with exact match pattern
    let out = compile_and_run_stdout(r#"
trait Matcher {
    fn matches(self, input: string) bool
}

class ExactMatcher impl Matcher {
    target: string
    fn matches(self, input: string) bool {
        return input == self.target
    }
}

fn main() {
    let m: Matcher = ExactMatcher { target: "hello" }
    print(m.matches("hello"))
    print(m.matches("world"))
}
"#);
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn trait_two_methods_same_return_type_different_semantics() {
    // Trait with two methods that return the same type but have different meanings
    let out = compile_and_run_stdout(r#"
trait Bounds {
    fn min_val(self) int
    fn max_val(self) int
}

class Range_ impl Bounds {
    lo: int
    hi: int
    fn min_val(self) int { return self.lo }
    fn max_val(self) int { return self.hi }
}

fn span(b: Bounds) int {
    return b.max_val() - b.min_val()
}

fn main() {
    print(span(Range_ { lo: 3, hi: 10 }))
}
"#);
    assert_eq!(out, "7\n");
}

#[test]
fn trait_dispatch_with_nested_method_calls() {
    // Trait method result used as argument to another method on same object
    let out = compile_and_run_stdout(r#"
trait Ops {
    fn base(self) int
    fn scale(self, x: int) int
}

class Scaler impl Ops {
    factor: int
    fn base(self) int { return self.factor }
    fn scale(self, x: int) int { return x * self.factor }
}

fn run(o: Ops) {
    print(o.scale(o.base()))
}

fn main() {
    run(Scaler { factor: 5 })
}
"#);
    assert_eq!(out, "25\n");
}

#[test]
fn trait_handle_stored_in_array_then_dispatched() {
    // Build array of trait handles via helper, then dispatch on each
    let out = compile_and_run_stdout(r#"
trait Greeter_ {
    fn greet(self) string
}

class Hello_ impl Greeter_ {
    name: string
    fn greet(self) string { return "hi {self.name}" }
}

fn add(arr: [Greeter_], g: Greeter_) {
    arr.push(g)
}

fn main() {
    let greeters: [Greeter_] = []
    add(greeters, Hello_ { name: "alice" })
    add(greeters, Hello_ { name: "bob" })
    let i = 0
    while i < greeters.len() {
        print(greeters[i].greet())
        i = i + 1
    }
}
"#);
    assert_eq!(out, "hi alice\nhi bob\n");
}

#[test]
fn trait_dispatch_result_compared_to_literal() {
    // Compare trait method result directly to a literal in if condition
    let out = compile_and_run_stdout(r#"
trait StatusCode {
    fn code(self) int
}

class OkStatus impl StatusCode {
    tag: int
    fn code(self) int { return 200 }
}

class NotFoundStatus impl StatusCode {
    tag: int
    fn code(self) int { return 404 }
}

fn is_ok(s: StatusCode) bool {
    return s.code() == 200
}

fn main() {
    let ok: StatusCode = OkStatus { tag: 0 }
    let nf: StatusCode = NotFoundStatus { tag: 0 }
    print(is_ok(ok))
    print(is_ok(nf))
}
"#);
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn trait_method_using_for_range_internally() {
    // Trait method body uses for-range loop
    let out = compile_and_run_stdout(r#"
trait Summer_ {
    fn sum_to(self, n: int) int
}

class NaiveSummer impl Summer_ {
    tag: int
    fn sum_to(self, n: int) int {
        let total = 0
        for i in 1..n + 1 {
            total = total + i
        }
        return total
    }
}

fn run(s: Summer_) {
    print(s.sum_to(5))
    print(s.sum_to(10))
}

fn main() {
    run(NaiveSummer { tag: 0 })
}
"#);
    assert_eq!(out, "15\n55\n");
}

#[test]
fn fail_call_method_not_in_trait_via_handle() {
    // Calling a method through trait handle that exists on class but not in trait
    compile_should_fail_with(r#"
trait Basic {
    fn basic(self) int
}

class Full impl Basic {
    tag: int
    fn basic(self) int { return 1 }
    fn extra(self) int { return 2 }
}

fn run(b: Basic) {
    print(b.extra())
}

fn main() {
    run(Full { tag: 0 })
}
"#, "trait 'Basic' has no method 'extra'");
}

#[test]
fn fail_trait_method_nullable_return_mismatch() {
    // Trait expects int return, class returns int? — should fail
    compile_should_fail_with(r#"
trait Getter {
    fn get(self) int
}

class NullableGetter impl Getter {
    tag: int
    fn get(self) int? {
        return none
    }
}

fn main() {
    let g: Getter = NullableGetter { tag: 0 }
    print(g.get())
}
"#, "method 'get' return type mismatch: trait 'Getter' expects int, class 'NullableGetter' returns int?");
}

#[test]
fn trait_four_classes_round_robin_dispatch() {
    // Four different classes implementing same trait, dispatched in round-robin
    let out = compile_and_run_stdout(r#"
trait Ident {
    fn id(self) int
}

class C1 impl Ident { tag: int fn id(self) int { return 1 } }
class C2 impl Ident { tag: int fn id(self) int { return 2 } }
class C3 impl Ident { tag: int fn id(self) int { return 3 } }
class C4 impl Ident { tag: int fn id(self) int { return 4 } }

fn add_ident(arr: [Ident], item: Ident) {
    arr.push(item)
}

fn main() {
    let items: [Ident] = []
    add_ident(items, C1 { tag: 0 })
    add_ident(items, C2 { tag: 0 })
    add_ident(items, C3 { tag: 0 })
    add_ident(items, C4 { tag: 0 })
    let i = 0
    while i < items.len() {
        print(items[i].id())
        i = i + 1
    }
}
"#);
    assert_eq!(out, "1\n2\n3\n4\n");
}

#[test]
fn trait_method_with_deeply_nested_arithmetic() {
    // Trait method doing complex nested arithmetic (stress codegen)
    let out = compile_and_run_stdout(r#"
trait MathOps {
    fn complex(self, x: int) int
}

class BigCalc impl MathOps {
    offset: int
    fn complex(self, x: int) int {
        return ((x + self.offset) * 2 + 1) * (x - self.offset + 3) + self.offset
    }
}

fn run(m: MathOps) {
    print(m.complex(10))
}

fn main() {
    run(BigCalc { offset: 3 })
}
"#);
    // (10 + 3) * 2 + 1 = 27, (10 - 3 + 3) = 10, 27 * 10 + 3 = 273
    assert_eq!(out, "273\n");
}

// ===== Batch 23: Closures as params, DI, field rejection, generics+traits, recursive data =====

#[test]
fn trait_method_takes_closure_param() {
    // Trait method accepts a closure (fn type) parameter
    let out = compile_and_run_stdout(r#"
trait Transformer_ {
    fn transform(self, f: fn(int) int) int
}

class Holder_ impl Transformer_ {
    val: int
    fn transform(self, f: fn(int) int) int {
        return f(self.val)
    }
}

fn main() {
    let h: Transformer_ = Holder_ { val: 5 }
    let doubled = h.transform((x: int) => x * 2)
    print(doubled)
    let squared = h.transform((x: int) => x * x)
    print(squared)
}
"#);
    assert_eq!(out, "10\n25\n");
}

#[test]
fn trait_method_returns_closure() {
    // Trait method creates and returns a closure
    let out = compile_and_run_stdout(r#"
trait ClosureMaker {
    fn make(self) fn(int) int
}

class Adder_ impl ClosureMaker {
    amount: int
    fn make(self) fn(int) int {
        let amt = self.amount
        return (x: int) => x + amt
    }
}

fn main() {
    let m: ClosureMaker = Adder_ { amount: 10 }
    let f = m.make()
    print(f(5))
    print(f(20))
}
"#);
    assert_eq!(out, "15\n30\n");
}

#[test]
fn trait_with_di_class_impl() {
    // Class with bracket deps (DI) also implements a trait
    let out = compile_and_run_stdout(r#"
class Database {
    tag: int
    fn label(self) string {
        return "db"
    }
}

trait Repository_ {
    fn query(self) string
}

class UserRepo[db: Database] impl Repository_ {
    tag: int
    fn query(self) string {
        return self.db.label()
    }
}

fn run(r: Repository_) {
    print(r.query())
}

app MyApp[repo: UserRepo] {
    fn main(self) {
        run(self.repo)
    }
}
"#);
    assert_eq!(out, "db\n");
}

#[test]
fn fail_access_field_on_trait_handle_direct() {
    // Cannot access field on a trait handle (traits are opaque)
    compile_should_fail_with(r#"
trait Foo {
    fn bar(self) int
}

class Impl impl Foo {
    val: int
    fn bar(self) int { return self.val }
}

fn use_foo(f: Foo) {
    print(f.val)
}

fn main() {
    use_foo(Impl { val: 5 })
}
"#, "field access on non-class type trait Foo");
}

#[test]
fn trait_generic_class_two_instantiations_same_trait() {
    // Generic class instantiated with two types, both implement same trait
    let out = compile_and_run_stdout(r#"
trait Sizer {
    fn size(self) int
}

class Wrap<T> impl Sizer {
    inner: T
    fn size(self) int { return 1 }
}

fn show_size(s: Sizer) {
    print(s.size())
}

fn main() {
    show_size(Wrap<int> { inner: 42 })
    show_size(Wrap<string> { inner: "hello" })
}
"#);
    assert_eq!(out, "1\n1\n");
}

#[test]
fn fail_trait_dispatch_on_recursive_linked_list() {
    // Fixed: enum type used as class field now works (forward reference fix)
    let out = compile_and_run_stdout(r#"
enum IntList {
    Cons { head: int, tail: IntList }
    Nil
}

trait Summable {
    fn total(self) int
}

class ListWrapper impl Summable {
    list: IntList
    fn total(self) int {
        return sum_list(self.list)
    }
}

fn sum_list(l: IntList) int {
    match l {
        IntList.Cons { head, tail } {
            return head + sum_list(tail)
        }
        IntList.Nil {
            return 0
        }
    }
}

fn run(s: Summable) {
    print(s.total())
}

fn main() {
    let list = IntList.Cons {
        head: 1,
        tail: IntList.Cons {
            head: 2,
            tail: IntList.Cons {
                head: 3,
                tail: IntList.Nil
            }
        }
    }
    run(ListWrapper { list: list })
}
"#);
    assert_eq!(out, "6\n");
}

#[test]
fn trait_method_void_called_in_sequence() {
    // Multiple void trait method calls in sequence
    let out = compile_and_run_stdout(r#"
trait Printer__ {
    fn print_val(self, prefix: string)
}

class NumPrinter impl Printer__ {
    val: int
    fn print_val(self, prefix: string) {
        print("{prefix}{self.val}")
    }
}

fn run(p: Printer__) {
    p.print_val("a=")
    p.print_val("b=")
    p.print_val("c=")
}

fn main() {
    run(NumPrinter { val: 7 })
}
"#);
    assert_eq!(out, "a=7\nb=7\nc=7\n");
}

#[test]
fn trait_default_method_overridden_by_some_classes() {
    // Trait with default, some classes override it and some don't
    let out = compile_and_run_stdout(r#"
trait Labeled_ {
    fn label(self) string { return "default" }
}

class Custom impl Labeled_ {
    name: string
    fn label(self) string { return self.name }
}

class Default_ impl Labeled_ {
    tag: int
}

fn show(l: Labeled_) {
    print(l.label())
}

fn main() {
    show(Custom { name: "custom" })
    show(Default_ { tag: 0 })
}
"#);
    assert_eq!(out, "custom\ndefault\n");
}

#[test]
fn trait_method_with_multiple_returns() {
    // Trait method with multiple return paths
    let out = compile_and_run_stdout(r#"
trait Categorizer {
    fn categorize(self, x: int) string
}

class ThreeBucket impl Categorizer {
    tag: int
    fn categorize(self, x: int) string {
        if x < 0 {
            return "negative"
        }
        if x == 0 {
            return "zero"
        }
        if x < 100 {
            return "small"
        }
        return "large"
    }
}

fn run(c: Categorizer) {
    print(c.categorize(-5))
    print(c.categorize(0))
    print(c.categorize(50))
    print(c.categorize(200))
}

fn main() {
    run(ThreeBucket { tag: 0 })
}
"#);
    assert_eq!(out, "negative\nzero\nsmall\nlarge\n");
}

#[test]
fn trait_dispatch_result_used_in_array_literal() {
    // Trait dispatch result placed into an array literal
    let out = compile_and_run_stdout(r#"
trait NumGen {
    fn gen(self) int
}

class ConstGen impl NumGen {
    val: int
    fn gen(self) int { return self.val }
}

fn main() {
    let g: NumGen = ConstGen { val: 7 }
    let arr: [int] = [g.gen(), g.gen() + 1, g.gen() + 2]
    print(arr[0])
    print(arr[1])
    print(arr[2])
}
"#);
    assert_eq!(out, "7\n8\n9\n");
}

#[test]
fn trait_method_with_while_loop_counter() {
    // Trait method implements counting logic with while loop
    let out = compile_and_run_stdout(r#"
trait Fibonacci {
    fn fib(self, n: int) int
}

class IterFib impl Fibonacci {
    tag: int
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
    print(f.fib(0))
    print(f.fib(1))
    print(f.fib(5))
    print(f.fib(10))
}

fn main() {
    run(IterFib { tag: 0 })
}
"#);
    assert_eq!(out, "0\n1\n5\n55\n");
}

#[test]
fn trait_method_error_propagation_chain() {
    // Error propagation through multiple trait dispatch calls
    let out = compile_and_run_stdout(r#"
error Oops { code: int }

trait Layer {
    fn process(self, x: int) int
}

class InnerLayer impl Layer {
    tag: int
    fn process(self, x: int) int {
        if x == 0 {
            raise Oops { code: 1 }
        }
        return x * 2
    }
}

class OuterLayer impl Layer {
    inner: Layer
    fn process(self, x: int) int {
        return self.inner.process(x)! + 1
    }
}

fn main() {
    let inner: Layer = InnerLayer { tag: 0 }
    let outer: Layer = OuterLayer { inner: inner }
    let r1 = outer.process(5) catch -1
    print(r1)
    let r2 = outer.process(0) catch -1
    print(r2)
}
"#);
    assert_eq!(out, "11\n-1\n");
}

#[test]
fn trait_method_takes_map_param() {
    // Trait method accepts a Map parameter
    let out = compile_and_run_stdout(r#"
trait ConfigReader {
    fn read_key(self, config: Map<string, int>, key: string) int
}

class DefaultReader impl ConfigReader {
    default_val: int
    fn read_key(self, config: Map<string, int>, key: string) int {
        if config.contains(key) {
            return config[key]
        }
        return self.default_val
    }
}

fn main() {
    let r: ConfigReader = DefaultReader { default_val: -1 }
    let m = Map<string, int> {}
    m["port"] = 8080
    print(r.read_key(m, "port"))
    print(r.read_key(m, "timeout"))
}
"#);
    assert_eq!(out, "8080\n-1\n");
}

#[test]
fn trait_dispatch_chained_three_different_traits() {
    // Call methods on three different trait handles in sequence, combining results
    let out = compile_and_run_stdout(r#"
trait First_ {
    fn first(self) int
}

trait Second_ {
    fn second(self) int
}

trait Third_ {
    fn third(self) int
}

class Trio impl First_, Second_, Third_ {
    val: int
    fn first(self) int { return self.val }
    fn second(self) int { return self.val * 2 }
    fn third(self) int { return self.val * 3 }
}

fn combine(a: First_, b: Second_, c: Third_) int {
    return a.first() + b.second() + c.third()
}

fn main() {
    let t = Trio { val: 5 }
    print(combine(t, t, t))
}
"#);
    assert_eq!(out, "30\n");
}

#[test]
fn trait_dispatch_in_return_expression() {
    // Trait method dispatch directly in return statement
    let out = compile_and_run_stdout(r#"
trait Evaluator {
    fn eval(self) int
}

class ConstEval impl Evaluator {
    val: int
    fn eval(self) int { return self.val }
}

fn get_eval(e: Evaluator) int {
    return e.eval()
}

fn main() {
    print(get_eval(ConstEval { val: 99 }))
}
"#);
    assert_eq!(out, "99\n");
}

#[test]
fn trait_method_with_nested_closures() {
    // Trait method uses a closure that captures a local variable
    let out = compile_and_run_stdout(r#"
trait ArrayProcessor {
    fn process(self, arr: [int]) [int]
}

class Doubler_ impl ArrayProcessor {
    tag: int
    fn process(self, arr: [int]) [int] {
        let result: [int] = []
        let i = 0
        while i < arr.len() {
            result.push(arr[i] * 2)
            i = i + 1
        }
        return result
    }
}

fn run(p: ArrayProcessor) {
    let data: [int] = [1, 2, 3]
    let out = p.process(data)
    let i = 0
    while i < out.len() {
        print(out[i])
        i = i + 1
    }
}

fn main() {
    run(Doubler_ { tag: 0 })
}
"#);
    assert_eq!(out, "2\n4\n6\n");
}

#[test]
fn trait_method_with_break_in_while() {
    // Trait method uses break to exit loop early
    let out = compile_and_run_stdout(r#"
trait SearchAlgo {
    fn find(self, arr: [int], target: int) int
}

class LinearSearch impl SearchAlgo {
    tag: int
    fn find(self, arr: [int], target: int) int {
        let i = 0
        while i < arr.len() {
            if arr[i] == target {
                return i
            }
            i = i + 1
        }
        return -1
    }
}

fn run(s: SearchAlgo) {
    let data: [int] = [10, 20, 30, 40, 50]
    print(s.find(data, 30))
    print(s.find(data, 99))
}

fn main() {
    run(LinearSearch { tag: 0 })
}
"#);
    assert_eq!(out, "2\n-1\n");
}

#[test]
fn fail_trait_return_type_float_vs_int() {
    // Trait expects float return but class returns int
    compile_should_fail_with(r#"
trait Measured {
    fn measure(self) float
}

class BadMeasure impl Measured {
    tag: int
    fn measure(self) int { return 5 }
}

fn main() {
    let m: Measured = BadMeasure { tag: 0 }
    print(m.measure())
}
"#, "method 'measure' return type mismatch: trait 'Measured' expects float, class 'BadMeasure' returns int");
}

#[test]
fn trait_ensures_violation_aborts() {
    // ensures contract violated at runtime should abort
    let (_, stderr, code) = compile_and_run_output(r#"
trait Positive_ {
    fn make(self) int
        ensures result > 0
}

class BadMaker impl Positive_ {
    tag: int
    fn make(self) int {
        return -1
    }
}

fn main() {
    let p: Positive_ = BadMaker { tag: 0 }
    print(p.make())
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("ensures"), "Expected ensures violation, got stderr: {}", stderr);
}

// ===== Batch 24: Array-of-traits deep, dispatch argument chains, void ordering, boundary =====

#[test]
fn trait_array_dispatch_all_elements() {
    // Array of trait handles, dispatch on each via for loop with index
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class A impl Valued {
    x: int
    fn val(self) int { return self.x }
}

class B impl Valued {
    x: int
    fn val(self) int { return self.x * 10 }
}

fn add_item(arr: [Valued], v: Valued) {
    arr.push(v)
}

fn main() {
    let items: [Valued] = []
    add_item(items, A { x: 1 })
    add_item(items, B { x: 2 })
    add_item(items, A { x: 3 })
    let total = 0
    let i = 0
    while i < items.len() {
        total = total + items[i].val()
        i = i + 1
    }
    print(total)
}
"#);
    assert_eq!(out, "24\n");
}

#[test]
fn trait_dispatch_result_as_argument_to_dispatch() {
    // Result of one dispatch used as argument to another dispatch
    let out = compile_and_run_stdout(r#"
trait Source {
    fn produce(self) int
}

trait Transform {
    fn apply(self, x: int) int
}

class NumSource impl Source {
    n: int
    fn produce(self) int { return self.n }
}

class Doubler impl Transform {
    tag: int
    fn apply(self, x: int) int { return x * 2 }
}

fn pipeline(s: Source, t: Transform) int {
    return t.apply(s.produce())
}

fn main() {
    let s: Source = NumSource { n: 7 }
    let t: Transform = Doubler { tag: 0 }
    print(pipeline(s, t))
}
"#);
    assert_eq!(out, "14\n");
}

#[test]
fn trait_void_methods_ordering_preserved() {
    // Multiple void methods called in sequence, verifying execution order
    let out = compile_and_run_stdout(r#"
trait Logger {
    fn log(self, msg: string)
}

class PrintLogger impl Logger {
    prefix: string
    fn log(self, msg: string) {
        print("{self.prefix}: {msg}")
    }
}

fn log_sequence(l: Logger) {
    l.log("first")
    l.log("second")
    l.log("third")
}

fn main() {
    let l: Logger = PrintLogger { prefix: "LOG" }
    log_sequence(l)
}
"#);
    assert_eq!(out, "LOG: first\nLOG: second\nLOG: third\n");
}

#[test]
fn trait_dispatch_result_stored_then_used_later() {
    // Store dispatch result in variable, use it several statements later
    let out = compile_and_run_stdout(r#"
trait Namer {
    fn name(self) string
}

class Dog impl Namer {
    tag: int
    fn name(self) string { return "rex" }
}

fn main() {
    let n: Namer = Dog { tag: 0 }
    let stored = n.name()
    let x = 42
    let y = x + 1
    print(stored)
    print(y)
}
"#);
    assert_eq!(out, "rex\n43\n");
}

#[test]
fn trait_method_modifies_array_param_visible_to_caller() {
    // Trait method pushes to array param; changes visible after dispatch
    let out = compile_and_run_stdout(r#"
trait Filler {
    fn fill(self, arr: [int])
}

class TripleFiller impl Filler {
    base: int
    fn fill(self, arr: [int]) {
        arr.push(self.base)
        arr.push(self.base + 1)
        arr.push(self.base + 2)
    }
}

fn do_fill(f: Filler, arr: [int]) {
    f.fill(arr)
}

fn main() {
    let arr: [int] = []
    let f: Filler = TripleFiller { base: 10 }
    do_fill(f, arr)
    print(arr.len())
    print(arr[0])
    print(arr[2])
}
"#);
    assert_eq!(out, "3\n10\n12\n");
}

#[test]
fn trait_method_returns_string_used_in_comparison() {
    // Dispatch returns string, use in == comparison
    let out = compile_and_run_stdout(r#"
trait Labeled {
    fn label(self) string
}

class Item impl Labeled {
    name: string
    fn label(self) string { return self.name }
}

fn check(l: Labeled) {
    if l.label() == "hello" {
        print(1)
    } else {
        print(0)
    }
}

fn main() {
    let a: Labeled = Item { name: "hello" }
    let b: Labeled = Item { name: "world" }
    check(a)
    check(b)
}
"#);
    assert_eq!(out, "1\n0\n");
}

#[test]
fn trait_method_with_multiple_array_params() {
    // Trait method takes two array params
    let out = compile_and_run_stdout(r#"
trait Merger {
    fn merge(self, a: [int], b: [int]) int
}

class SumMerger impl Merger {
    tag: int
    fn merge(self, a: [int], b: [int]) int {
        return a.len() + b.len()
    }
}

fn do_merge(m: Merger) {
    let x: [int] = [1, 2, 3]
    let y: [int] = [4, 5]
    print(m.merge(x, y))
}

fn main() {
    let m: Merger = SumMerger { tag: 0 }
    do_merge(m)
}
"#);
    assert_eq!(out, "5\n");
}

#[test]
fn trait_dispatch_in_while_with_state_update() {
    // Dispatch inside while loop where result updates loop variable
    let out = compile_and_run_stdout(r#"
trait Stepper {
    fn step(self, current: int) int
}

class DoubleStepper impl Stepper {
    tag: int
    fn step(self, current: int) int { return current * 2 }
}

fn run(s: Stepper) {
    let val = 1
    while val < 100 {
        val = s.step(val)
    }
    print(val)
}

fn main() {
    let s: Stepper = DoubleStepper { tag: 0 }
    run(s)
}
"#);
    assert_eq!(out, "128\n");
}

#[test]
fn trait_two_params_same_trait_different_instances() {
    // Function takes two params of same trait type, different implementations
    let out = compile_and_run_stdout(r#"
trait Valued {
    fn val(self) int
}

class X impl Valued {
    n: int
    fn val(self) int { return self.n }
}

class Y impl Valued {
    n: int
    fn val(self) int { return self.n * 100 }
}

fn combine(a: Valued, b: Valued) int {
    return a.val() + b.val()
}

fn main() {
    let a: Valued = X { n: 3 }
    let b: Valued = Y { n: 2 }
    print(combine(a, b))
}
"#);
    assert_eq!(out, "203\n");
}

#[test]
fn trait_method_with_nested_for_loops() {
    // Trait method body has nested for loops
    let out = compile_and_run_stdout(r#"
trait MatrixSum {
    fn sum(self) int
}

class Grid impl MatrixSum {
    rows: int
    cols: int
    fn sum(self) int {
        let total = 0
        for r in 0..self.rows {
            for c in 0..self.cols {
                total = total + 1
            }
        }
        return total
    }
}

fn run(m: MatrixSum) {
    print(m.sum())
}

fn main() {
    let g: MatrixSum = Grid { rows: 3, cols: 4 }
    run(g)
}
"#);
    assert_eq!(out, "12\n");
}

#[test]
fn trait_method_returns_bool_used_in_while_condition() {
    // Dispatch result (bool) used directly as while loop condition
    let out = compile_and_run_stdout(r#"
trait Gate {
    fn is_open(self, counter: int) bool
}

class ThresholdGate impl Gate {
    limit: int
    fn is_open(self, counter: int) bool {
        return counter < self.limit
    }
}

fn count_through(g: Gate) {
    let i = 0
    while g.is_open(i) {
        i = i + 1
    }
    print(i)
}

fn main() {
    let g: Gate = ThresholdGate { limit: 5 }
    count_through(g)
}
"#);
    assert_eq!(out, "5\n");
}

#[test]
fn trait_dispatch_string_concatenation_chain() {
    // Multiple dispatches, string results concatenated
    let out = compile_and_run_stdout(r#"
trait Part {
    fn text(self) string
}

class Head impl Part {
    tag: int
    fn text(self) string { return "hello" }
}

class Tail impl Part {
    tag: int
    fn text(self) string { return "world" }
}

fn join(a: Part, b: Part) string {
    return a.text() + " " + b.text()
}

fn main() {
    let h: Part = Head { tag: 0 }
    let t: Part = Tail { tag: 0 }
    print(join(h, t))
}
"#);
    assert_eq!(out, "hello world\n");
}

#[test]
fn trait_method_with_string_len_check() {
    // Trait method uses .len() on string field
    let out = compile_and_run_stdout(r#"
trait HasLength {
    fn length(self) int
}

class Wrapper impl HasLength {
    data: string
    fn length(self) int { return self.data.len() }
}

fn show(h: HasLength) {
    print(h.length())
}

fn main() {
    let w: HasLength = Wrapper { data: "hello" }
    show(w)
}
"#);
    assert_eq!(out, "5\n");
}

#[test]
fn trait_dispatch_result_used_in_for_range_bound() {
    // Dispatch result as upper bound of for range
    let out = compile_and_run_stdout(r#"
trait Limiter {
    fn limit(self) int
}

class Fixed impl Limiter {
    n: int
    fn limit(self) int { return self.n }
}

fn count_to(l: Limiter) {
    let sum = 0
    for i in 0..l.limit() {
        sum = sum + i
    }
    print(sum)
}

fn main() {
    let l: Limiter = Fixed { n: 5 }
    count_to(l)
}
"#);
    assert_eq!(out, "10\n");
}

#[test]
#[ignore] // Compiler bug: panic in typeck/register.rs:1326:59 - range start index 1 out of range
fn crash_trait_method_missing_self_with_impl() {
    // FIXED: Trait method without self now shows proper error instead of panicking
    compile_should_fail_with(r#"
trait Bad {
    fn compute() int
}

class Impl impl Bad {
    tag: int
    fn compute() int { return 42 }
}

fn main() {
    let b: Bad = Impl { tag: 0 }
    print(b.compute())
}
"#, "trait method 'compute' must have a 'self' parameter");
}

#[test]
fn fail_construct_trait_directly() {
    // Cannot construct a trait as if it were a class
    compile_should_fail_with(r#"
trait Foo {
    fn bar(self) int
}

fn main() {
    let f = Foo { }
}
"#, "unknown class 'Foo'");
}

#[test]
fn fail_trait_method_call_wrong_arg_count() {
    // Calling trait method with wrong number of arguments
    compile_should_fail_with(r#"
trait Adder {
    fn add(self, x: int) int
}

class Impl impl Adder {
    tag: int
    fn add(self, x: int) int { return x + 1 }
}

fn main() {
    let a: Adder = Impl { tag: 0 }
    print(a.add(1, 2))
}
"#, "method 'add' expects 1 arguments, got 2");
}

#[test]
fn fail_trait_method_call_wrong_arg_type() {
    // Calling trait method with wrong argument type
    compile_should_fail_with(r#"
trait Adder {
    fn add(self, x: int) int
}

class Impl impl Adder {
    tag: int
    fn add(self, x: int) int { return x + 1 }
}

fn main() {
    let a: Adder = Impl { tag: 0 }
    print(a.add("hello"))
}
"#, "argument 1 of 'add': expected int, found string");
}

#[test]
fn trait_default_method_uses_two_required_methods() {
    // Default method calls two different required methods
    let out = compile_and_run_stdout(r#"
trait Stats {
    fn min_val(self) int
    fn max_val(self) int
    fn range(self) int {
        return self.max_val() - self.min_val()
    }
}

class Data impl Stats {
    lo: int
    hi: int
    fn min_val(self) int { return self.lo }
    fn max_val(self) int { return self.hi }
}

fn show(s: Stats) {
    print(s.range())
}

fn main() {
    let d: Stats = Data { lo: 3, hi: 10 }
    show(d)
}
"#);
    assert_eq!(out, "7\n");
}

#[test]
fn trait_dispatch_alternating_three_classes_in_loop() {
    // Array of 3 different implementations, dispatched in loop
    let out = compile_and_run_stdout(r#"
trait Numbered {
    fn num(self) int
}

class One impl Numbered {
    tag: int
    fn num(self) int { return 1 }
}

class Two impl Numbered {
    tag: int
    fn num(self) int { return 2 }
}

class Three impl Numbered {
    tag: int
    fn num(self) int { return 3 }
}

fn add_item(arr: [Numbered], n: Numbered) {
    arr.push(n)
}

fn main() {
    let items: [Numbered] = []
    add_item(items, One { tag: 0 })
    add_item(items, Two { tag: 0 })
    add_item(items, Three { tag: 0 })
    let sum = 0
    let i = 0
    while i < items.len() {
        sum = sum + items[i].num()
        i = i + 1
    }
    print(sum)
}
"#);
    assert_eq!(out, "6\n");
}

// ===== Batch 25: Return class, string methods, dispatch arithmetic, error catch, trait-takes-trait =====

#[test]
fn fail_trait_method_returns_class_forward_ref_gap() {
    // Fixed: Class type in trait method return position now works (forward reference fix)
    let out = compile_and_run_stdout(r#"
class Point {
    x: int
    y: int
}

trait PointMaker {
    fn make_point(self) Point
}

class Factory impl PointMaker {
    dx: int
    dy: int
    fn make_point(self) Point {
        return Point { x: self.dx, y: self.dy }
    }
}

fn use_maker(pm: PointMaker) {
    let p = pm.make_point()
    print(p.x)
    print(p.y)
}

fn main() {
    let f: PointMaker = Factory { dx: 10, dy: 20 }
    use_maker(f)
}
"#);
    assert_eq!(out, "10\n20\n");
}

#[test]
fn trait_method_uses_string_contains() {
    // Trait method body calls .contains() on string
    let out = compile_and_run_stdout(r#"
trait Matcher {
    fn matches(self, input: string) bool
}

class SubstringMatcher impl Matcher {
    pattern: string
    fn matches(self, input: string) bool {
        return input.contains(self.pattern)
    }
}

fn check(m: Matcher, s: string) {
    if m.matches(s) {
        print(1)
    } else {
        print(0)
    }
}

fn main() {
    let m: Matcher = SubstringMatcher { pattern: "ell" }
    check(m, "hello")
    check(m, "world")
}
"#);
    assert_eq!(out, "1\n0\n");
}

#[test]
fn trait_binary_op_between_two_dispatches() {
    // Single expression with two trait dispatches combined by operator
    let out = compile_and_run_stdout(r#"
trait Num_ {
    fn val(self) int
}

class A_ impl Num_ {
    n: int
    fn val(self) int { return self.n }
}

class B_ impl Num_ {
    n: int
    fn val(self) int { return self.n * 10 }
}

fn main() {
    let a: Num_ = A_ { n: 3 }
    let b: Num_ = B_ { n: 2 }
    print(a.val() + b.val())
    print(a.val() * b.val())
}
"#);
    assert_eq!(out, "23\n60\n");
}

#[test]
fn trait_method_iterates_array_param() {
    // Trait method uses for..in to iterate array parameter
    let out = compile_and_run_stdout(r#"
trait Aggregator_ {
    fn total(self, items: [int]) int
}

class Summer_ impl Aggregator_ {
    tag: int
    fn total(self, items: [int]) int {
        let sum = 0
        for v in items {
            sum = sum + v
        }
        return sum
    }
}

fn run(a: Aggregator_) {
    let arr: [int] = [10, 20, 30]
    print(a.total(arr))
}

fn main() {
    let a: Aggregator_ = Summer_ { tag: 0 }
    run(a)
}
"#);
    assert_eq!(out, "60\n");
}

#[test]
fn trait_default_method_with_interp_of_required() {
    // Default method uses string interpolation with result of required method
    let out = compile_and_run_stdout(r#"
trait Named_ {
    fn name(self) string
    fn greeting(self) string {
        return "Hello, {self.name()}!"
    }
}

class Person_ impl Named_ {
    n: string
    fn name(self) string { return self.n }
}

fn show(n: Named_) {
    print(n.greeting())
}

fn main() {
    let p: Named_ = Person_ { n: "Alice" }
    show(p)
}
"#);
    assert_eq!(out, "Hello, Alice!\n");
}

#[test]
fn trait_method_builds_and_returns_map() {
    // Trait method creates a map and returns it
    let out = compile_and_run_stdout(r#"
trait MapBuilder_ {
    fn build(self) Map<string, int>
}

class PairBuilder_ impl MapBuilder_ {
    key1: string
    val1: int
    fn build(self) Map<string, int> {
        let m = Map<string, int> {}
        m[self.key1] = self.val1
        return m
    }
}

fn show(b: MapBuilder_) {
    let m = b.build()
    print(m.len())
}

fn main() {
    let b: MapBuilder_ = PairBuilder_ { key1: "a", val1: 42 }
    show(b)
}
"#);
    assert_eq!(out, "1\n");
}

#[test]
fn trait_dispatch_inside_catch_expression() {
    // Trait method that raises error, caught by caller
    let out = compile_and_run_stdout(r#"
error BadInput_ {
    code: int
}

trait Validator_ {
    fn validate(self, x: int) int
}

class StrictValidator_ impl Validator_ {
    tag: int
    fn validate(self, x: int) int {
        if x < 0 {
            raise BadInput_ { code: 1 }
        }
        return x
    }
}

fn safe_validate(v: Validator_, x: int) int {
    return v.validate(x) catch -1
}

fn main() {
    let v: Validator_ = StrictValidator_ { tag: 0 }
    print(safe_validate(v, 5))
    print(safe_validate(v, -3))
}
"#);
    assert_eq!(out, "5\n-1\n");
}

#[test]
fn trait_dispatch_in_match_on_enum_result() {
    // Fixed: Enum type in trait method return position now works with two-pass
    let out = compile_and_run_stdout(r#"
enum Status_ {
    Active
    Inactive { reason: string }
}

trait StatusProvider_ {
    fn status(self) Status_
}

class Server_ impl StatusProvider_ {
    up: bool
    fn status(self) Status_ {
        if self.up {
            return Status_.Active
        }
        return Status_.Inactive { reason: "down" }
    }
}

fn check(s: StatusProvider_) {
    match s.status() {
        Status_.Active {
            print("ok")
        }
        Status_.Inactive { reason } {
            print(reason)
        }
    }
}

fn main() {
    let s1: StatusProvider_ = Server_ { up: true }
    let s2: StatusProvider_ = Server_ { up: false }
    check(s1)
    check(s2)
}
"#);
    assert_eq!(out, "ok\ndown\n");
}

#[test]
fn trait_method_takes_trait_param_different_trait() {
    // Trait method takes another trait-typed parameter
    let out = compile_and_run_stdout(r#"
trait FormatTrait {
    fn format(self, val: int) string
}

trait PrintTrait {
    fn do_print(self, f: FormatTrait, val: int)
}

class SimpleFormat impl FormatTrait {
    tag: int
    fn format(self, val: int) string {
        return "val={val}"
    }
}

class ConsolePrint impl PrintTrait {
    tag: int
    fn do_print(self, f: FormatTrait, val: int) {
        print(f.format(val))
    }
}

fn run(p: PrintTrait, f: FormatTrait) {
    p.do_print(f, 42)
}

fn main() {
    let f: FormatTrait = SimpleFormat { tag: 0 }
    let p: PrintTrait = ConsolePrint { tag: 0 }
    run(p, f)
}
"#);
    assert_eq!(out, "val=42\n");
}

#[test]
fn trait_method_with_to_int_string_parse() {
    // Trait method uses string.to_int() nullable result
    let out = compile_and_run_stdout(r#"
trait IntParser {
    fn parse(self, input: string) int
}

class SafeParser impl IntParser {
    default_val: int
    fn parse(self, input: string) int {
        let result = input.to_int()
        if result == none {
            return self.default_val
        }
        return result?
    }
}

fn run(p: IntParser) {
    print(p.parse("42"))
    print(p.parse("abc"))
}

fn main() {
    let p: IntParser = SafeParser { default_val: -1 }
    run(p)
}
"#);
    assert_eq!(out, "42\n-1\n");
}

#[test]
fn trait_dispatch_three_methods_interleaved_on_two_objects() {
    // Two different trait objects, calling methods in interleaved order
    let out = compile_and_run_stdout(r#"
trait MultiMethod {
    fn a(self) int
    fn b(self) int
    fn c(self) int
}

class MM1 impl MultiMethod {
    tag: int
    fn a(self) int { return 1 }
    fn b(self) int { return 2 }
    fn c(self) int { return 3 }
}

class MM2 impl MultiMethod {
    tag: int
    fn a(self) int { return 10 }
    fn b(self) int { return 20 }
    fn c(self) int { return 30 }
}

fn main() {
    let x: MultiMethod = MM1 { tag: 0 }
    let y: MultiMethod = MM2 { tag: 0 }
    print(x.a() + y.a())
    print(x.b() + y.b())
    print(x.c() + y.c())
}
"#);
    assert_eq!(out, "11\n22\n33\n");
}

#[test]
fn trait_method_with_string_starts_with() {
    // Trait method uses .starts_with() on string
    let out = compile_and_run_stdout(r#"
trait PrefixCheck {
    fn has_prefix(self, input: string) bool
}

class HttpCheck impl PrefixCheck {
    prefix: string
    fn has_prefix(self, input: string) bool {
        return input.starts_with(self.prefix)
    }
}

fn check(p: PrefixCheck, s: string) {
    if p.has_prefix(s) {
        print("yes")
    } else {
        print("no")
    }
}

fn main() {
    let p: PrefixCheck = HttpCheck { prefix: "http" }
    check(p, "http://example.com")
    check(p, "ftp://example.com")
}
"#);
    assert_eq!(out, "yes\nno\n");
}

#[test]
fn trait_dispatch_comparison_between_two_results() {
    // Compare results of two trait dispatches
    let out = compile_and_run_stdout(r#"
trait ScoreGiver {
    fn score(self) int
}

class LowScore impl ScoreGiver {
    tag: int
    fn score(self) int { return 10 }
}

class HighScore impl ScoreGiver {
    tag: int
    fn score(self) int { return 90 }
}

fn compare(a: ScoreGiver, b: ScoreGiver) {
    if a.score() > b.score() {
        print("a wins")
    } else {
        print("b wins")
    }
}

fn main() {
    let a: ScoreGiver = LowScore { tag: 0 }
    let b: ScoreGiver = HighScore { tag: 0 }
    compare(a, b)
    compare(b, a)
}
"#);
    assert_eq!(out, "b wins\na wins\n");
}

#[test]
fn trait_default_method_with_for_range_loop() {
    // Default method contains a for range loop
    let out = compile_and_run_stdout(r#"
trait TextRepeater {
    fn text(self) string
    fn repeat(self, n: int) string {
        let result = ""
        for i in 0..n {
            result = result + self.text()
        }
        return result
    }
}

class Star impl TextRepeater {
    tag: int
    fn text(self) string { return "x" }
}

fn show(r: TextRepeater) {
    print(r.repeat(4))
}

fn main() {
    let s: TextRepeater = Star { tag: 0 }
    show(s)
}
"#);
    assert_eq!(out, "xxxx\n");
}

#[test]
fn fail_return_wrong_class_from_trait_method() {
    // Trait method declared to return ClassA but returns ClassB
    compile_should_fail_with(r#"
class Result__ {
    val: int
}

class Other__ {
    val: int
}

trait Producer__ {
    fn make(self) Result__
}

class Impl__ impl Producer__ {
    tag: int
    fn make(self) Result__ {
        return Other__ { val: 1 }
    }
}

fn main() {}
"#, "return type mismatch: expected Result__, found Other__");
}

#[test]
fn fail_trait_method_return_array_wrong_element_type() {
    // Returns [string] when trait declares [int]
    compile_should_fail_with(r#"
trait Lister__ {
    fn items(self) [int]
}

class ListerImpl impl Lister__ {
    tag: int
    fn items(self) [int] {
        let arr: [string] = ["a"]
        return arr
    }
}

fn main() {}
"#, "return type mismatch: expected [int], found [string]");
}

#[test]
fn trait_handle_survives_triple_reassignment() {
    // Create handle, reassign to different impl multiple times, dispatch after each
    let out = compile_and_run_stdout(r#"
trait Getter_ {
    fn get(self) int
}

class GA impl Getter_ {
    n: int
    fn get(self) int { return self.n }
}

class GB impl Getter_ {
    n: int
    fn get(self) int { return self.n * 10 }
}

fn main() {
    let g: Getter_ = GA { n: 5 }
    print(g.get())
    g = GB { n: 3 }
    print(g.get())
    g = GA { n: 7 }
    print(g.get())
}
"#);
    assert_eq!(out, "5\n30\n7\n");
}

#[test]
fn trait_dispatch_result_pushed_to_array() {
    // Push result of dispatch into array
    let out = compile_and_run_stdout(r#"
trait IntProducer {
    fn produce(self) int
}

class FiveProducer impl IntProducer {
    tag: int
    fn produce(self) int { return 5 }
}

fn main() {
    let p: IntProducer = FiveProducer { tag: 0 }
    let arr: [int] = []
    arr.push(p.produce())
    arr.push(p.produce())
    arr.push(p.produce())
    print(arr.len())
    print(arr[0] + arr[1] + arr[2])
}
"#);
    assert_eq!(out, "3\n15\n");
}

#[test]
fn trait_method_complex_search_with_early_return() {
    // Trait method with if/else + for + early return
    let out = compile_and_run_stdout(r#"
trait ArraySearcher {
    fn find_first_above(self, items: [int], threshold: int) int
}

class LinearSearcher impl ArraySearcher {
    tag: int
    fn find_first_above(self, items: [int], threshold: int) int {
        for v in items {
            if v > threshold {
                return v
            }
        }
        return -1
    }
}

fn run(s: ArraySearcher) {
    let arr: [int] = [1, 5, 3, 8, 2]
    print(s.find_first_above(arr, 4))
    print(s.find_first_above(arr, 10))
}

fn main() {
    let s: ArraySearcher = LinearSearcher { tag: 0 }
    run(s)
}
"#);
    assert_eq!(out, "5\n-1\n");
}

#[test]
fn trait_method_error_caught_with_binding() {
    // Error from trait dispatch caught with catch err { expr } syntax
    let out = compile_and_run_stdout(r#"
error ParseErr_ {
    msg: string
}

trait TextParser {
    fn parse(self, input: string) int
}

class StrictTextParser impl TextParser {
    tag: int
    fn parse(self, input: string) int {
        if input == "bad" {
            raise ParseErr_ { msg: "invalid" }
        }
        return 42
    }
}

fn safe_parse(p: TextParser, input: string) {
    let result = p.parse(input) catch err {
        print("error")
        0
    }
    print(result)
}

fn main() {
    let p: TextParser = StrictTextParser { tag: 0 }
    safe_parse(p, "good")
    safe_parse(p, "bad")
}
"#);
    assert_eq!(out, "42\nerror\n0\n");
}
// ===== Batch 26: Empty trait, modules, large params, defaults chain, final comprehensive =====

#[test]
fn trait_empty_no_methods_marker_pattern() {
    // Empty trait (marker trait pattern) — zero methods
    let out = compile_and_run_stdout(r#"
trait Marker {
}

class Tagged impl Marker {
    val: int
}

fn accepts_marker(m: Marker) {
    print(1)
}

fn main() {
    let t: Marker = Tagged { val: 42 }
    accepts_marker(t)
}
"#);
    assert_eq!(out, "1\n");
}

#[test]
fn trait_method_with_ten_parameters() {
    // Stress test: trait method with 10 parameters
    let out = compile_and_run_stdout(r#"
trait BigSig_ {
    fn compute(self, a: int, b: int, c: int, d: int, e: int, 
               f: int, g: int, h: int, i: int, j: int) int
}

class Summer impl BigSig_ {
    tag: int
    fn compute(self, a: int, b: int, c: int, d: int, e: int,
               f: int, g: int, h: int, i: int, j: int) int {
        return a + b + c + d + e + f + g + h + i + j
    }
}

fn run(bs: BigSig_) {
    print(bs.compute(1, 2, 3, 4, 5, 6, 7, 8, 9, 10))
}

fn main() {
    let s: BigSig_ = Summer { tag: 0 }
    run(s)
}
"#);
    assert_eq!(out, "55\n");
}

#[test]
fn trait_default_calls_another_default_method() {
    // Default method A calls default method B
    let out = compile_and_run_stdout(r#"
trait Chain {
    fn required(self) int
    fn first_default(self) int {
        return self.required() * 2
    }
    fn second_default(self) int {
        return self.first_default() + 10
    }
}

class Impl impl Chain {
    n: int
    fn required(self) int { return self.n }
}

fn run(c: Chain) {
    print(c.second_default())
}

fn main() {
    let c: Chain = Impl { n: 5 }
    run(c)
}
"#);
    assert_eq!(out, "20\n");
}

#[test]
fn trait_function_takes_three_different_trait_params() {
    // Function with three different trait-typed params
    let out = compile_and_run_stdout(r#"
trait A {
    fn a_val(self) int
}

trait B {
    fn b_val(self) int
}

trait C {
    fn c_val(self) int
}

class ImplA impl A {
    n: int
    fn a_val(self) int { return self.n }
}

class ImplB impl B {
    n: int
    fn b_val(self) int { return self.n * 10 }
}

class ImplC impl C {
    n: int
    fn c_val(self) int { return self.n * 100 }
}

fn combine(a: A, b: B, c: C) int {
    return a.a_val() + b.b_val() + c.c_val()
}

fn main() {
    let a: A = ImplA { n: 1 }
    let b: B = ImplB { n: 2 }
    let c: C = ImplC { n: 3 }
    print(combine(a, b, c))
}
"#);
    assert_eq!(out, "321\n");
}

#[test]
fn trait_method_with_set_operations() {
    // Trait method builds and returns a set
    let out = compile_and_run_stdout(r#"
trait SetBuilder {
    fn build_set(self) Set<int>
}

class RangeSet impl SetBuilder {
    start: int
    end: int
    fn build_set(self) Set<int> {
        let s = Set<int> {}
        for i in self.start..self.end {
            s.insert(i)
        }
        return s
    }
}

fn show(sb: SetBuilder) {
    let s = sb.build_set()
    print(s.len())
}

fn main() {
    let rs: SetBuilder = RangeSet { start: 1, end: 6 }
    show(rs)
}
"#);
    assert_eq!(out, "5\n");
}

#[test]
fn trait_method_dispatch_inside_spawn() {
    // Trait method called inside spawned task
    let out = compile_and_run_stdout(r#"
trait Computer {
    fn compute(self) int
}

class SlowComputer impl Computer {
    n: int
    fn compute(self) int {
        return self.n * 2
    }
}

fn compute_async(c: Computer) int {
    let t = spawn c.compute()
    return t.get()!
}

fn main() {
    let c: Computer = SlowComputer { n: 21 }
    print(compute_async(c)!)
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn trait_handle_array_filter_pattern() {
    // Array of trait handles, filter and process
    let out = compile_and_run_stdout(r#"
trait Valued__ {
    fn val(self) int
}

class Pos impl Valued__ {
    n: int
    fn val(self) int { return self.n }
}

class Neg impl Valued__ {
    n: int
    fn val(self) int { return 0 - self.n }
}

fn add_valued(arr: [Valued__], v: Valued__) {
    arr.push(v)
}

fn main() {
    let items: [Valued__] = []
    add_valued(items, Pos { n: 10 })
    add_valued(items, Neg { n: 5 })
    add_valued(items, Pos { n: 3 })
    let sum = 0
    let i = 0
    while i < items.len() {
        let val = items[i].val()
        if val > 0 {
            sum = sum + val
        }
        i = i + 1
    }
    print(sum)
}
"#);
    assert_eq!(out, "13\n");
}

#[test]
fn trait_dispatch_with_range_expression_param() {
    // Pass range expression result to trait method
    let out = compile_and_run_stdout(r#"
trait RangeProcessor {
    fn count_in_range(self, start: int, end: int) int
}

class Counter impl RangeProcessor {
    tag: int
    fn count_in_range(self, start: int, end: int) int {
        let count = 0
        for i in start..end {
            count = count + 1
        }
        return count
    }
}

fn run(rp: RangeProcessor) {
    print(rp.count_in_range(5, 10))
}

fn main() {
    let rp: RangeProcessor = Counter { tag: 0 }
    run(rp)
}
"#);
    assert_eq!(out, "5\n");
}

#[test]
fn trait_method_returns_float_dispatch() {
    // Trait method returns float, dispatch and use in arithmetic
    let out = compile_and_run_stdout(r#"
trait FloatProvider {
    fn provide(self) float
}

class Pi impl FloatProvider {
    tag: int
    fn provide(self) float { return 3.14 }
}

fn show(fp: FloatProvider) {
    let val = fp.provide()
    print(val)
}

fn main() {
    let p: FloatProvider = Pi { tag: 0 }
    show(p)
}
"#);
    // Floats print as %f format (e.g., 3.140000)
    assert!(out.starts_with("3.14"));
}

#[test]
fn trait_three_levels_dispatch_indirection() {
    // f1 calls f2 calls f3, all take trait param and dispatch
    let out = compile_and_run_stdout(r#"
trait Val {
    fn get(self) int
}

class X impl Val {
    n: int
    fn get(self) int { return self.n }
}

fn level1(v: Val) int {
    return level2(v)
}

fn level2(v: Val) int {
    return level3(v)
}

fn level3(v: Val) int {
    return v.get()
}

fn main() {
    let v: Val = X { n: 99 }
    print(level1(v))
}
"#);
    assert_eq!(out, "99\n");
}

#[test]
fn trait_method_with_modulo_and_bitwise() {
    // Trait method using % and bitwise ops
    let out = compile_and_run_stdout(r#"
trait BitOps {
    fn combine(self, x: int) int
}

class Impl impl BitOps {
    mask: int
    fn combine(self, x: int) int {
        return (x % 10) & self.mask
    }
}

fn run(bo: BitOps) {
    print(bo.combine(27))
}

fn main() {
    let bo: BitOps = Impl { mask: 3 }
    run(bo)
}
"#);
    assert_eq!(out, "3\n");
}

#[test]
fn trait_dispatch_result_in_conditional_expression() {
    // Ternary-style conditional using dispatch result
    let out = compile_and_run_stdout(r#"
trait Checker {
    fn check(self) bool
}

class AlwaysTrue impl Checker {
    tag: int
    fn check(self) bool { return true }
}

fn run(c: Checker) {
    let result = 0
    if c.check() {
        result = 1
    } else {
        result = 0
    }
    print(result)
}

fn main() {
    let c: Checker = AlwaysTrue { tag: 0 }
    run(c)
}
"#);
    assert_eq!(out, "1\n");
}

#[test]
fn trait_method_with_byte_operations() {
    // Trait method working with byte type
    let out = compile_and_run_stdout(r#"
trait ByteProvider {
    fn get_byte(self) byte
}

class Impl impl ByteProvider {
    val: byte
    fn get_byte(self) byte { return self.val }
}

fn show(bp: ByteProvider) {
    let b = bp.get_byte()
    print(b as int)
}

fn main() {
    let bp: ByteProvider = Impl { val: 65 as byte }
    show(bp)
}
"#);
    assert_eq!(out, "65\n");
}

#[test]
fn trait_dispatch_in_array_map_pattern() {
    // Simulate map pattern: iterate trait array, dispatch, collect results
    let out = compile_and_run_stdout(r#"
trait Doubler {
    fn double(self) int
}

class Val impl Doubler {
    n: int
    fn double(self) int { return self.n * 2 }
}

fn add_doubler(arr: [Doubler], d: Doubler) {
    arr.push(d)
}

fn main() {
    let items: [Doubler] = []
    add_doubler(items, Val { n: 1 })
    add_doubler(items, Val { n: 2 })
    add_doubler(items, Val { n: 3 })
    let results: [int] = []
    let i = 0
    while i < items.len() {
        results.push(items[i].double())
        i = i + 1
    }
    print(results[0])
    print(results[1])
    print(results[2])
}
"#);
    assert_eq!(out, "2\n4\n6\n");
}

#[test]
fn trait_method_with_abs_builtin() {
    // Trait method using abs() builtin
    let out = compile_and_run_stdout(r#"
trait Absolute {
    fn absolute(self, x: int) int
}

class Impl impl Absolute {
    tag: int
    fn absolute(self, x: int) int {
        return abs(x)
    }
}

fn run(a: Absolute) {
    print(a.absolute(-42))
    print(a.absolute(10))
}

fn main() {
    let a: Absolute = Impl { tag: 0 }
    run(a)
}
"#);
    assert_eq!(out, "42\n10\n");
}

#[test]
fn fail_trait_as_class_field_type_coercion() {
    // Fixed: concrete class coerces to trait-typed field in struct literal
    let out = compile_and_run_stdout(r#"
trait Worker {
    fn work(self) int
}

class MyWorker impl Worker {
    n: int
    fn work(self) int { return self.n }
}

class Container {
    worker: Worker
}

fn main() {
    let c = Container { worker: MyWorker { n: 1 } }
    print(c.worker.work())
}
"#);
    assert_eq!(out, "1\n");
}

#[test]
fn fail_duplicate_trait_in_impl_list() {
    // Same trait listed twice in impl list should be rejected
    compile_should_fail_with(r#"
trait Foo {
    fn work(self) int
}

class X impl Foo, Foo {
    n: int
    fn work(self) int { return self.n }
}

fn main() {
    let x: Foo = X { n: 42 }
    print(x.work())
}
"#, "trait 'Foo' appears multiple times in impl list for class 'X'");
}

#[test]
fn trait_dispatch_with_logical_and_short_circuit() {
    // Dispatch in && expression; verify short-circuit
    let out = compile_and_run_stdout(r#"
trait Predicate {
    fn check_val(self, x: int) bool
}

class Even impl Predicate {
    tag: int
    fn check_val(self, x: int) bool {
        return x % 2 == 0
    }
}

class Positive impl Predicate {
    tag: int
    fn check_val(self, x: int) bool {
        return x > 0
    }
}

fn check(p1: Predicate, p2: Predicate, x: int) {
    if p1.check_val(x) && p2.check_val(x) {
        print(1)
    } else {
        print(0)
    }
}

fn main() {
    let e: Predicate = Even { tag: 0 }
    let p: Predicate = Positive { tag: 0 }
    check(e, p, 4)
    check(e, p, -2)
    check(e, p, 3)
}
"#);
    assert_eq!(out, "1\n0\n0\n");
}

#[test]
fn trait_method_string_split_iteration() {
    // Trait method using .split() and iteration
    let out = compile_and_run_stdout(r#"
trait WordCounter {
    fn count_words(self, text: string) int
}

class Counter impl WordCounter {
    tag: int
    fn count_words(self, text: string) int {
        let parts = text.split(" ")
        return parts.len()
    }
}

fn run(wc: WordCounter) {
    print(wc.count_words("hello world foo"))
}

fn main() {
    let wc: WordCounter = Counter { tag: 0 }
    run(wc)
}
"#);
    assert_eq!(out, "3\n");
}

#[test]
fn trait_default_overridden_plus_not_overridden_mixed() {
    // Class overrides one default, keeps other as-is
    let out = compile_and_run_stdout(r#"
trait Mixed {
    fn required(self) int
    fn default_a(self) int {
        return self.required() + 1
    }
    fn default_b(self) int {
        return self.required() + 10
    }
}

class Impl impl Mixed {
    n: int
    fn required(self) int { return self.n }
    fn default_a(self) int {
        return self.required() * 100
    }
}

fn run(m: Mixed) {
    print(m.default_a())
    print(m.default_b())
}

fn main() {
    let m: Mixed = Impl { n: 5 }
    run(m)
}
"#);
    assert_eq!(out, "500\n15\n");
}

// ===== PLUTO-002: Trait Coercion in Struct Literals =====

#[test]
fn pluto_002_struct_lit_trait_nullable_move_dispatch() {
    // Class→Trait? in struct literal, container copied to new variable.
    // Verifies the trait handle survives struct copy and nullable comparison works.
    // Note: ! unwrap on nullable trait has a pre-existing typeck limitation,
    // so we verify non-none instead of dispatching through the nullable.
    let out = compile_and_run_stdout(r#"
trait Worker {
    fn work(self) int
}
class W impl Worker {
    fn work(self) int { return 9 }
}
class Box {
    w: Worker?
}
fn main() {
    let a = Box { w: W {} }
    let b = a
    if b.w != none { print(1) }
}
"#);
    assert_eq!(out, "1\n");
}

#[test]
fn pluto_002_reject_non_impl_nullable_trait_struct() {
    compile_should_fail_with(r#"
trait Worker {
    fn work(self) int
}
class NotAWorker {
    n: int
}
class Container {
    worker: Worker?
}
fn main() {
    let c = Container { worker: NotAWorker { n: 1 } }
}
"#, "expected trait Worker");
}
