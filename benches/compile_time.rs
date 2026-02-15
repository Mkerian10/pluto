//! Compiler performance benchmarks.
//!
//! Measures compilation speed (not runtime speed - see benchmarks/ directory for that).
//! Run with: cargo bench

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_compile_hello_world(c: &mut Criterion) {
    let source = r#"
        fn main() {
            print("Hello, world!")
        }
    "#;

    c.bench_function("compile_hello_world", |b| {
        b.iter(|| pluto::compile_to_object(black_box(source)))
    });
}

fn bench_compile_generics(c: &mut Criterion) {
    let source = r#"
        class Box<T> {
            value: T
        }

        fn main() {
            let b1 = Box<int> { value: 42 }
            let b2 = Box<string> { value: "hi" }
            let b3 = Box<float> { value: 3.14 }
        }
    "#;

    c.bench_function("compile_generics", |b| {
        b.iter(|| pluto::compile_to_object(black_box(source)))
    });
}

fn bench_compile_closures(c: &mut Criterion) {
    let source = r#"
        fn main() {
            let x = 10
            let f = (y: int) => x + y
            let result = f(32)
            print(result)
        }
    "#;

    c.bench_function("compile_closures", |b| {
        b.iter(|| pluto::compile_to_object(black_box(source)))
    });
}

fn bench_compile_errors(c: &mut Criterion) {
    let source = r#"
        error MyError { message: string }

        fn might_fail() int! {
            raise MyError { message: "oops" }
        }

        fn main() {
            let x = might_fail() catch {
                MyError(e) => 0
            }
            print(x)
        }
    "#;

    c.bench_function("compile_errors", |b| {
        b.iter(|| pluto::compile_to_object(black_box(source)))
    });
}

fn bench_compile_large_program(c: &mut Criterion) {
    // Simulate a larger program with multiple classes and functions
    let source = r#"
        class Point {
            x: int
            y: int
        }

        class Rectangle {
            top_left: Point
            bottom_right: Point
        }

        fn distance(p1: Point, p2: Point) float {
            let dx = (p2.x - p1.x) as float
            let dy = (p2.y - p1.y) as float
            return sqrt(dx * dx + dy * dy)
        }

        fn area(r: Rectangle) int {
            let width = r.bottom_right.x - r.top_left.x
            let height = r.bottom_right.y - r.top_left.y
            return width * height
        }

        fn main() {
            let p1 = Point { x: 0, y: 0 }
            let p2 = Point { x: 10, y: 10 }
            let r = Rectangle { top_left: p1, bottom_right: p2 }
            let a = area(r)
            let d = distance(p1, p2)
            print(a)
            print(d)
        }
    "#;

    c.bench_function("compile_large_program", |b| {
        b.iter(|| pluto::compile_to_object(black_box(source)))
    });
}

criterion_group!(
    benches,
    bench_compile_hello_world,
    bench_compile_generics,
    bench_compile_closures,
    bench_compile_errors,
    bench_compile_large_program
);
criterion_main!(benches);
