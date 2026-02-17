mod common;
use common::compile_and_run_stdout;

#[test]
fn gc_string_pressure() {
    // 10k string concatenations in a loop — old strings become garbage
    let out = compile_and_run_stdout(r#"
fn main() {
    let s = "start"
    let i = 0
    while i < 10000 {
        s = s + "x"
        i = i + 1
    }
    print(s.len())
}
"#);
    assert_eq!(out.trim(), "10005");
}

#[test]
fn gc_class_allocation_loop() {
    // Allocate 10k class instances in a loop; only the last one is retained
    let out = compile_and_run_stdout(r#"
class Box {
    value: int
}

fn main() {
    let b = Box { value: 0 }
    let i = 0
    while i < 10000 {
        b = Box { value: i }
        i = i + 1
    }
    print(b.value)
}
"#);
    assert_eq!(out.trim(), "9999");
}

#[test]
fn gc_array_of_classes() {
    // Array holding class pointers — GC must trace through array to keep classes alive
    let out = compile_and_run_stdout(r#"
class Point {
    x: int
    y: int
}

fn main() {
    let first = Point { x: 0, y: 0 }
    let arr = [first]
    let i = 1
    while i < 100 {
        arr.push(Point { x: i, y: i * 2 })
        i = i + 1
    }
    // Create garbage to trigger GC
    let j = 0
    while j < 10000 {
        let tmp = Point { x: j, y: j }
        j = j + 1
    }
    // Array elements must survive
    let p = arr[99]
    print(p.x)
    print(p.y)
}
"#);
    assert_eq!(out.trim(), "99\n198");
}

#[test]
fn gc_closure_captures_survive() {
    // Closure captures heap objects; throwaway closures create garbage
    let out = compile_and_run_stdout(r#"
fn make_adder(n: int) fn(int) int {
    return (x: int) => n + x
}

fn main() {
    let add5 = make_adder(5)
    // Create garbage closures
    let i = 0
    while i < 10000 {
        let tmp = make_adder(i)
        i = i + 1
    }
    // Original closure must still work
    print(add5(10))
}
"#);
    assert_eq!(out.trim(), "15");
}

#[test]
fn gc_enum_allocation_pressure() {
    // Allocate many enum variants in a loop
    let out = compile_and_run_stdout(r#"
enum Shape {
    Circle { radius: int }
    Rect { w: int, h: int }
}

fn area(s: Shape) int {
    match s {
        Shape.Circle { radius } {
            return radius * radius
        }
        Shape.Rect { w, h } {
            return w * h
        }
    }
}

fn main() {
    let total = 0
    let i = 0
    while i < 10000 {
        let s = Shape.Circle { radius: 1 }
        total = total + area(s)
        i = i + 1
    }
    print(total)
}
"#);
    assert_eq!(out.trim(), "10000");
}

#[test]
fn gc_string_interpolation_pressure() {
    // String interpolation creates intermediate strings that become garbage
    let out = compile_and_run_stdout(r#"
fn main() {
    let i = 0
    let last = ""
    while i < 5000 {
        last = f"item_{i}"
        i = i + 1
    }
    print(last)
}
"#);
    assert_eq!(out.trim(), "item_4999");
}

#[test]
fn gc_di_app_with_pressure() {
    // DI app with GC pressure inside app method
    let out = compile_and_run_stdout(r#"
class Counter {
    count: int

    fn increment(mut self) {
        self.count = self.count + 1
    }

    fn get(self) int {
        return self.count
    }
}

app MyApp[counter: Counter] {
    fn main(self) {
        let i = 0
        while i < 10000 {
            // Create garbage strings
            let s = f"garbage_{i}"
            self.counter.increment()
            i = i + 1
        }
        print(self.counter.get())
    }
}
"#);
    assert_eq!(out.trim(), "10000");
}

#[test]
fn gc_deep_object_graph() {
    // Build a chain of 10k nodes via array, validates worklist-based (non-recursive) mark
    let out = compile_and_run_stdout(r#"
class Node {
    value: int
    next_idx: int
}

fn main() {
    let first = Node { value: 0, next_idx: -1 }
    let nodes = [first]
    let i = 1
    while i < 10000 {
        let prev_idx = i - 1
        nodes.push(Node { value: i, next_idx: prev_idx })
        i = i + 1
    }
    // Create garbage to trigger GC
    let j = 0
    while j < 5000 {
        let tmp = Node { value: j, next_idx: -1 }
        j = j + 1
    }
    // Walk the chain via indices to verify integrity
    let count = 0
    let idx = 9999
    while idx >= 0 {
        let n = nodes[idx]
        count = count + 1
        idx = n.next_idx
    }
    print(count)
}
"#);
    assert_eq!(out.trim(), "10000");
}

#[test]
fn gc_string_concat_pressure() {
    // Many string concatenations creating lots of intermediate garbage
    let out = compile_and_run_stdout(r#"
fn main() {
    let i = 0
    let total_len = 0
    while i < 5000 {
        let s = "a" + "b" + "c" + "d" + "e"
        total_len = total_len + s.len()
        i = i + 1
    }
    print(total_len)
}
"#);
    assert_eq!(out.trim(), "25000");
}

#[test]
fn gc_retained_objects_survive() {
    // Multiple objects retained across GC cycles
    let out = compile_and_run_stdout(r#"
class Pair {
    a: string
    b: string
}

fn main() {
    let p1 = Pair { a: "hello", b: "world" }
    let p2 = Pair { a: "foo", b: "bar" }
    // Create garbage to trigger GC
    let i = 0
    while i < 10000 {
        let tmp = Pair { a: "x", b: "y" }
        i = i + 1
    }
    // Retained objects must survive
    print(p1.a)
    print(p1.b)
    print(p2.a)
    print(p2.b)
}
"#);
    assert_eq!(out.trim(), "hello\nworld\nfoo\nbar");
}

#[test]
fn gc_array_growth_under_pressure() {
    // Array that grows (realloc) while GC is active
    let out = compile_and_run_stdout(r#"
fn main() {
    let arr = ["seed"]
    let i = 1
    while i < 5000 {
        arr.push(f"item_{i}")
        i = i + 1
    }
    print(arr.len())
    print(arr[0])
    print(arr[4999])
}
"#);
    assert_eq!(out.trim(), "5000\nseed\nitem_4999");
}

#[test]
fn gc_trait_objects_survive() {
    // Trait objects (data_ptr + vtable_ptr) must be traced correctly
    let out = compile_and_run_stdout(r#"
trait Greeter {
    fn greet(self) string
}

class English impl Greeter {
    tag: int

    fn greet(self) string {
        return "hello"
    }
}

class Spanish impl Greeter {
    tag: int

    fn greet(self) string {
        return "hola"
    }
}

fn get_greeting(g: Greeter) string {
    return g.greet()
}

fn main() {
    let e = English { tag: 1 }
    let s = Spanish { tag: 2 }
    // Create garbage to potentially trigger GC
    let i = 0
    while i < 10000 {
        let tmp = English { tag: i }
        i = i + 1
    }
    print(get_greeting(e))
    print(get_greeting(s))
}
"#);
    assert_eq!(out.trim(), "hello\nhola");
}

#[test]
fn gc_heap_size_returns_positive() {
    // After allocating a class, gc_heap_size() should return > 0
    let out = compile_and_run_stdout(r#"
class Obj {
    value: int
}

fn main() {
    let o = Obj { value: 42 }
    let size = gc_heap_size()
    if size > 0 {
        print("positive")
    } else {
        print("zero")
    }
    print(o.value)
}
"#);
    assert_eq!(out.trim(), "positive\n42");
}

#[test]
fn gc_heap_size_bounded_after_churn() {
    // Allocate and discard 10K objects; heap size should stay bounded (under 1MB)
    let out = compile_and_run_stdout(r#"
class Obj {
    value: int
}

fn main() {
    let i = 0
    while i < 10000 {
        let tmp = Obj { value: i }
        i = i + 1
    }
    let size = gc_heap_size()
    if size < 1048576 {
        print("bounded")
    } else {
        print(f"LEAK: heap size = {size}")
    }
}
"#);
    assert_eq!(out.trim(), "bounded");
}
