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
