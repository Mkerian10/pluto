mod common;
use common::*;

// ── Basic generators ──────────────────────────────────────────────────

#[test]
fn generator_single_yield() {
    let out = compile_and_run_stdout(r#"
fn single() stream int {
    yield 42
}

fn main() {
    for x in single() {
        print(x)
    }
}
"#);
    assert_eq!(out.trim(), "42");
}

#[test]
fn generator_multiple_yields() {
    let out = compile_and_run_stdout(r#"
fn three() stream int {
    yield 1
    yield 2
    yield 3
}

fn main() {
    for x in three() {
        print(x)
    }
}
"#);
    assert_eq!(out.trim(), "1\n2\n3");
}

#[test]
fn generator_with_while_loop_simple() {
    let out = compile_and_run_stdout(r#"
fn count_three() stream int {
    let mut i = 0
    while i < 3 {
        yield i
        i = i + 1
    }
}

fn main() {
    for n in count_three() {
        print(n)
    }
}
"#);
    assert_eq!(out.trim(), "0\n1\n2");
}

#[test]
fn generator_with_while_loop() {
    let out = compile_and_run_stdout(r#"
fn range(start: int, end: int) stream int {
    let mut i = start
    while i < end {
        yield i
        i = i + 1
    }
}

fn main() {
    for n in range(0, 5) {
        print(n)
    }
}
"#);
    assert_eq!(out.trim(), "0\n1\n2\n3\n4");
}

#[test]
fn generator_with_params() {
    let out = compile_and_run_stdout(r#"
fn count_from(start: int, count: int) stream int {
    let mut i = 0
    while i < count {
        yield start + i
        i = i + 1
    }
}

fn main() {
    for n in count_from(10, 3) {
        print(n)
    }
}
"#);
    assert_eq!(out.trim(), "10\n11\n12");
}

#[test]
fn generator_with_if() {
    let out = compile_and_run_stdout(r#"
fn evens(limit: int) stream int {
    let mut i = 0
    while i < limit {
        if i % 2 == 0 {
            yield i
        }
        i = i + 1
    }
}

fn main() {
    for n in evens(10) {
        print(n)
    }
}
"#);
    assert_eq!(out.trim(), "0\n2\n4\n6\n8");
}

#[test]
fn generator_early_return() {
    let out = compile_and_run_stdout(r#"
fn up_to_three() stream int {
    yield 1
    yield 2
    yield 3
    return
    yield 4
}

fn main() {
    for x in up_to_three() {
        print(x)
    }
}
"#);
    assert_eq!(out.trim(), "1\n2\n3");
}

#[test]
fn generator_empty() {
    let out = compile_and_run_stdout(r#"
fn empty_gen() stream int {
    return
}

fn main() {
    let mut count = 0
    for x in empty_gen() {
        count = count + 1
    }
    print(count)
}
"#);
    assert_eq!(out.trim(), "0");
}

#[test]
fn generator_string_values() {
    let out = compile_and_run_stdout(r#"
fn greetings() stream string {
    yield "hello"
    yield "world"
}

fn main() {
    for s in greetings() {
        print(s)
    }
}
"#);
    assert_eq!(out.trim(), "hello\nworld");
}

#[test]
fn generator_break_in_consumer() {
    let out = compile_and_run_stdout(r#"
fn naturals() stream int {
    let mut i = 0
    while true {
        yield i
        i = i + 1
    }
}

fn main() {
    for n in naturals() {
        if n >= 3 {
            break
        }
        print(n)
    }
}
"#);
    assert_eq!(out.trim(), "0\n1\n2");
}

#[test]
fn generator_continue_in_consumer() {
    let out = compile_and_run_stdout(r#"
fn range(start: int, end: int) stream int {
    let mut i = start
    while i < end {
        yield i
        i = i + 1
    }
}

fn main() {
    for n in range(0, 5) {
        if n % 2 == 0 {
            continue
        }
        print(n)
    }
}
"#);
    assert_eq!(out.trim(), "1\n3");
}

#[test]
fn generator_as_variable() {
    let out = compile_and_run_stdout(r#"
fn three() stream int {
    yield 10
    yield 20
    yield 30
}

fn main() {
    let g = three()
    for x in g {
        print(x)
    }
}
"#);
    assert_eq!(out.trim(), "10\n20\n30");
}

#[test]
fn generator_multiple_in_sequence() {
    let out = compile_and_run_stdout(r#"
fn ab() stream int {
    yield 1
    yield 2
}

fn cd() stream int {
    yield 3
    yield 4
}

fn main() {
    for x in ab() {
        print(x)
    }
    for x in cd() {
        print(x)
    }
}
"#);
    assert_eq!(out.trim(), "1\n2\n3\n4");
}

#[test]
fn generator_nested_while() {
    let out = compile_and_run_stdout(r#"
fn matrix() stream int {
    let mut i = 0
    while i < 3 {
        let mut j = 0
        while j < 2 {
            yield i * 10 + j
            j = j + 1
        }
        i = i + 1
    }
}

fn main() {
    for v in matrix() {
        print(v)
    }
}
"#);
    assert_eq!(out.trim(), "0\n1\n10\n11\n20\n21");
}

#[test]
fn generator_fibonacci() {
    let out = compile_and_run_stdout(r#"
fn fibonacci() stream int {
    let mut a = 0
    let mut b = 1
    while true {
        yield a
        let next = a + b
        a = b
        b = next
    }
}

fn main() {
    for f in fibonacci() {
        if f > 20 {
            break
        }
        print(f)
    }
}
"#);
    assert_eq!(out.trim(), "0\n1\n1\n2\n3\n5\n8\n13");
}

// ── Compile errors ────────────────────────────────────────────────────

#[test]
fn yield_outside_generator() {
    compile_should_fail_with(r#"
fn not_a_generator() int {
    yield 42
    return 0
}

fn main() {
    not_a_generator()
}
"#, "yield can only be used inside a generator");
}

#[test]
fn yield_type_mismatch() {
    compile_should_fail_with(r#"
fn gen() stream int {
    yield "hello"
}

fn main() {
    for x in gen() {
        print(x)
    }
}
"#, "yield type mismatch");
}

#[test]
fn return_value_in_generator() {
    compile_should_fail_with(r#"
fn gen() stream int {
    yield 1
    return 42
}

fn main() {
    for x in gen() {
        print(x)
    }
}
"#, "return with a value is not allowed in generator");
}

#[test]
fn yield_inside_closure() {
    compile_should_fail_with(r#"
fn gen() stream int {
    let f = () => {
        yield 1
    }
}

fn main() {
    for x in gen() {
        print(x)
    }
}
"#, "yield can only be used inside a generator");
}

#[test]
fn generator_with_closures_in_body() {
    let out = compile_and_run_stdout(r#"
fn gen(n: int) stream int {
    let double = (x: int) => x * 2
    let mut i = 0
    while i < n {
        yield double(i)
        i = i + 1
    }
}

fn main() {
    for x in gen(4) {
        print(x)
    }
}
"#);
    assert_eq!(out.trim(), "0\n2\n4\n6");
}

#[test]
fn generator_bool_values() {
    let out = compile_and_run_stdout(r#"
fn bools() stream bool {
    yield true
    yield false
    yield true
}

fn main() {
    for b in bools() {
        print(b)
    }
}
"#);
    assert_eq!(out.trim(), "true\nfalse\ntrue");
}

#[test]
fn generator_float_values() {
    let out = compile_and_run_stdout(r#"
fn floats() stream float {
    yield 1.5
    yield 2.5
    yield 3.5
}

fn main() {
    for f in floats() {
        print(f)
    }
}
"#);
    assert_eq!(out.trim(), "1.5\n2.5\n3.5");
}

#[test]
fn generator_conditional_yield() {
    let out = compile_and_run_stdout(r#"
fn positive_only(limit: int) stream int {
    let mut i = 0
    while i < limit {
        if i > 0 {
            yield i
        }
        i = i + 1
    }
}

fn main() {
    for x in positive_only(4) {
        print(x)
    }
}
"#);
    assert_eq!(out.trim(), "1\n2\n3");
}

#[test]
fn generator_if_else_yield() {
    let out = compile_and_run_stdout(r#"
fn classify(n: int) stream string {
    let mut i = 0
    while i < n {
        if i % 2 == 0 {
            yield "even"
        } else {
            yield "odd"
        }
        i = i + 1
    }
}

fn main() {
    for s in classify(4) {
        print(s)
    }
}
"#);
    assert_eq!(out.trim(), "even\nodd\neven\nodd");
}

// ── Loops, matches and channels that suspend ─────────────────────────
//
// A yield returns from the generator's next function and the resume block
// re-enters from the state dispatch, so every piece of state a loop or arm
// needs after the yield must live in a saved slot, not an SSA value.

#[test]
fn generator_for_range_with_yield() {
    let out = compile_and_run_stdout(r#"
fn squares(n: int) stream int {
    for i in 0..n {
        yield i * i
    }
    for j in 1..=2 {
        yield j
    }
}

fn main() {
    for v in squares(4) {
        print(v)
    }
}
"#);
    assert_eq!(out.trim(), "0\n1\n4\n9\n1\n2");
}

#[test]
fn generator_for_array_param_with_yield() {
    let out = compile_and_run_stdout(r#"
fn halves(xs: [float]) stream float {
    for x in xs {
        yield x / 2.0
    }
}

fn tags(xs: [string]) stream string {
    for x in xs {
        let t = f"<{x}>"
        yield t
    }
}

fn main() {
    for h in halves([1.0, 3.0]) {
        print(h)
    }
    for t in tags(["a", "b"]) {
        print(t)
    }
}
"#);
    assert_eq!(out.trim(), "0.5\n1.5\n<a>\n<b>");
}

#[test]
fn generator_for_string_and_bytes_with_yield() {
    let out = compile_and_run_stdout(r#"
fn chars(s: string) stream string {
    for c in s {
        yield c
    }
}

fn codes(b: bytes) stream int {
    for x in b {
        yield x.to_int()
    }
}

fn main() {
    for c in chars("ab") {
        print(c)
    }
    for n in codes("AB".to_bytes()) {
        print(n)
    }
}
"#);
    assert_eq!(out.trim(), "a\nb\n65\n66");
}

#[test]
fn generator_for_over_stream_with_yield() {
    let out = compile_and_run_stdout(r#"
fn upto(n: int) stream int {
    let mut i = 0
    while i < n {
        yield i
        i = i + 1
    }
}

fn pairs() stream int {
    for a in upto(3) {
        for b in upto(a) {
            yield a * 10 + b
        }
    }
}

fn main() {
    for v in pairs() {
        print(v)
    }
}
"#);
    assert_eq!(out.trim(), "10\n20\n21");
}

#[test]
fn generator_for_continue_and_break_with_yield() {
    let out = compile_and_run_stdout(r#"
fn odds_below(xs: [int], limit: int) stream int {
    for x in xs {
        if x >= limit {
            break
        }
        if x % 2 == 0 {
            continue
        }
        yield x
    }
    yield 99
}

fn main() {
    for v in odds_below([1, 2, 3, 4, 5, 6, 7], 6) {
        print(v)
    }
}
"#);
    assert_eq!(out.trim(), "1\n3\n5\n99");
}

#[test]
fn generator_for_receiver_with_yield() {
    let out = compile_and_run_stdout(r#"
fn put(tx: Sender<int>, v: int) {
    tx.send(v) catch err { print("send failed") }
}

fn drain() stream int {
    let (tx, rx) = chan<int>(4)
    put(tx, 5)
    yield 1
    put(tx, 6)
    tx.close()
    for v in rx {
        yield v * 10
    }
}

fn main() {
    for v in drain() {
        print(v)
    }
}
"#);
    assert_eq!(out.trim(), "1\n50\n60");
}

#[test]
fn generator_reused_loop_name_with_different_types() {
    let out = compile_and_run_stdout(r#"
fn mixed() stream string {
    for x in [1, 2] {
        yield f"{x}"
    }
    for x in [0.5] {
        yield f"{x}"
    }
    for x in ["s"] {
        yield x
    }
}

fn main() {
    for v in mixed() {
        print(v)
    }
}
"#);
    assert_eq!(out.trim(), "1\n2\n0.5\ns");
}

#[test]
fn generator_match_arms_with_yield() {
    let out = compile_and_run_stdout(r#"
enum Shape {
    Circle { r: float }
    Rect { w: int, h: int }
    Dot
}

fn describe(shapes: [Shape]) stream string {
    for s in shapes {
        match s {
            Shape.Circle { r } {
                yield f"circle {r}"
                yield f"again {r}"
            }
            Shape.Rect { w, h } {
                yield f"rect {w}"
                yield f"area {w * h}"
            }
            Shape.Dot {
                yield "dot"
            }
        }
    }
}

fn main() {
    let shapes = [Shape.Rect { w: 2, h: 3 }, Shape.Circle { r: 1.5 }, Shape.Dot]
    for d in describe(shapes) {
        print(d)
    }
}
"#);
    assert_eq!(out.trim(), "rect 2\narea 6\ncircle 1.5\nagain 1.5\ndot");
}

#[test]
fn generator_match_wildcard_and_return() {
    let out = compile_and_run_stdout(r#"
enum E {
    A
    B
    C
}

fn g(e: E) stream int {
    match e {
        E.A {
            yield 1
            return
        }
        _ {
            yield 0
        }
    }
    yield 9
}

fn main() {
    for v in g(E.A) {
        print(v)
    }
    for v in g(E.C) {
        print(v)
    }
}
"#);
    assert_eq!(out.trim(), "1\n0\n9");
}

#[test]
fn generator_suspending_loop_survives_gc() {
    // Hundreds of thousands of yields from inside nested loops, each
    // allocating, so collections run while the loop state sits in the
    // generator object between calls.
    let out = compile_and_run_stdout(r#"
fn words(n: int) stream string {
    let base = ["alpha", "beta", "gamma"]
    for i in 0..n {
        for w in base {
            yield f"{w}{i}"
        }
    }
}

fn main() {
    let mut total = 0
    for s in words(100000) {
        total = total + s.len()
    }
    print(total)
}
"#);
    assert_eq!(out.trim(), "2866670");
}

// ── Generic generators ───────────────────────────────────────────────

#[test]
fn generic_generator() {
    let out = compile_and_run_stdout(r#"
fn repeat<T>(x: T, n: int) stream T {
    let mut i = 0
    while i < n {
        yield x
        i = i + 1
    }
}

fn each<T>(xs: [T]) stream T {
    for x in xs {
        yield x
    }
}

fn mapped<T, U>(xs: [T], f: fn(T) U) stream U {
    for x in xs {
        yield f(x)
    }
}

fn main() {
    for s in repeat("hi", 2) {
        print(s)
    }
    for v in each([1.5, 2.5]) {
        print(v)
    }
    for s in mapped([1, 2], (x: int) => f"<{x}>") {
        print(s)
    }
}
"#);
    assert_eq!(out.trim(), "hi\nhi\n1.5\n2.5\n<1>\n<2>");
}

#[test]
fn stream_param_in_generic_function() {
    let out = compile_and_run_stdout(r#"
fn nums() stream int {
    yield 1
    yield 2
}

fn total<T>(s: stream T, f: fn(T) int) int {
    let mut t = 0
    for x in s {
        t = t + f(x)
    }
    return t
}

fn main() {
    print(total(nums(), (x: int) => x * 10))
}
"#);
    assert_eq!(out.trim(), "30");
}
