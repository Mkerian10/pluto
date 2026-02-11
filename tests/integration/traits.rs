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
