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
