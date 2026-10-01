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
    // NOTE (2026-10 perf audit): this benchmark previously declared
    // `fn might_fail() int!` and used a match-style catch — neither is Pluto
    // syntax (fallibility is inferred; `!` belongs to function types and call
    // sites). compile_to_object returned a parse error in ~30 µs and the
    // benchmark measured that error path. The source below actually compiles.
    let source = r#"
        error MyError { message: string }

        fn might_fail(x: int) int {
            if x < 0 {
                raise MyError { message: "oops" }
            }
            return x * 2
        }

        fn propagate(x: int) int {
            let v = might_fail(x)!
            return v + 1
        }

        fn main() {
            let x = propagate(-1) catch 0
            print(x)
        }
    "#;

    c.bench_function("compile_errors", |b| {
        b.iter(|| pluto::compile_to_object(black_box(source)))
    });
}

/// Scaling probe for the closure-compilation anomaly found in the 2026-10
/// perf audit: `compile_closures` (one closure) regressed +115% between
/// 885b934 and master in the `compile_to_object` path, while CLI compiles of
/// closure-heavy files showed no per-closure regression. Benchmarking 8
/// closures distinguishes a fixed "program contains closures" cost from a
/// per-closure cost.
fn bench_compile_closures_8(c: &mut Criterion) {
    let mut source = String::from("fn main() {\n    let mut acc = 0\n");
    for i in 0..8 {
        source.push_str(&format!("    let x{i} = {i}\n"));
        source.push_str(&format!("    let f{i} = (y: int) => x{i} + y\n"));
        source.push_str(&format!("    acc = acc + f{i}({i})\n"));
    }
    source.push_str("    print(acc)\n}\n");

    c.bench_function("compile_closures_8", |b| {
        b.iter(|| pluto::compile_to_object(black_box(&source)))
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
    bench_compile_closures_8,
    bench_compile_errors,
    bench_compile_large_program
);
criterion_main!(benches);
