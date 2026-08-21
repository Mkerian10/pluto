//! Generic error sets tests — error inference through generic functions,
//! classes, and enums. Call sites of fallible generics must handle with ! or
//! catch; unhandled fallible calls inside generic bodies are also rejected.
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

const MUST_HANDLE: &str = "must be handled with ! or catch";

// Generic functions with errors
#[test]
fn generic_fn_raises_no_handler() {
    compile_should_fail_with(
        r#"
error E{}
fn id<T>(x: T) T {
    if true {
        raise E{}
    }
    return x
}
fn main() {
    id(42)
}
"#,
        MUST_HANDLE,
    );
}

#[test]
fn generic_fn_raises_unconditionally() {
    compile_should_fail_with(
        r#"
error E{}
fn id<T>(x: T) T {
    raise E{}
    return x
}
fn main() {
    let y = id(42)
    print(y)
}
"#,
        MUST_HANDLE,
    );
}

#[test]
fn generic_different_instantiations() {
    compile_should_fail_with(
        r#"
error E{}
fn id<T>(x: T) T {
    raise E{}
    return x
}
fn main() {
    let a = id(42) catch 0
    let s = id("hi")
    print(a)
    print(s)
}
"#,
        MUST_HANDLE,
    );
}

// Generic classes with error-raising methods
#[test]
fn generic_class_method_raises() {
    compile_should_fail_with(
        r#"
error E{}
class Box<T> {
    value: T

    fn get(self) T {
        if true {
            raise E{}
        }
        return self.value
    }
}
fn main() {
    let b = Box<int> { value: 42 }
    b.get()
}
"#,
        MUST_HANDLE,
    );
}

#[test]
fn generic_class_multiple_instantiations() {
    compile_should_fail_with(
        r#"
error E{}
class Box<T> {
    value: T

    fn get(self) T {
        raise E{}
        return self.value
    }
}
fn main() {
    let b1 = Box<int> { value: 42 }
    let x = b1.get() catch 0
    print(x)
    let b2 = Box<string> { value: "hi" }
    b2.get()
}
"#,
        MUST_HANDLE,
    );
}

// Generic enums with errors
#[test]
fn generic_enum_match_raises() {
    compile_should_fail_with(
        r#"
error E{}
enum Opt<T> {
    Some { v: T }
    None
}
fn unwrap<T>(x: Opt<T>) T {
    match x {
        Opt.Some { v } {
            return v
        }
        Opt.None {
            raise E{}
        }
    }
    return unwrap(x)
}
fn main() {
    unwrap<int>(Opt<int>.None)
}
"#,
        MUST_HANDLE,
    );
}

// Error sets differ per instantiation
#[test]
fn different_errors_per_instantiation() {
    compile_should_fail_with(
        r#"
error E1{}
error E2{}
fn process<T>(x: T) T {
    if true {
        raise E1{}
    }
    if false {
        raise E2{}
    }
    return x
}
fn main() {
    process(42)
}
"#,
        MUST_HANDLE,
    );
}

// Unhandled fallible calls INSIDE a generic body are rejected
#[test]
fn generic_accumulates_errors() {
    compile_should_fail_with(
        r#"
error E1{}
error E2{}
fn a() int {
    raise E1{}
    return 1
}
fn b() int {
    raise E2{}
    return 2
}
fn combine<T>(x: T) T {
    a()
    b()
    return x
}
fn main() {
    let y = combine(42) catch err { 0 }
    print(y)
}
"#,
        MUST_HANDLE,
    );
}

// Generic type bounds with errors
#[test]
fn generic_bounded_fallible() {
    compile_should_fail_with(
        r#"
error E{}
trait Marker {
    fn tag(self) int
}
class C impl Marker {
    x: int

    fn tag(self) int {
        return self.x
    }
}
fn process<U: Marker>(x: U) U {
    raise E{}
    return x
}
fn main() {
    process(C { x: 1 })
}
"#,
        MUST_HANDLE,
    );
}

// Nested generics with errors
#[test]
fn nested_generic_fallible() {
    compile_should_fail_with(
        r#"
error E{}
class Box<T> {
    value: T
}
fn unbox<T>(b: Box<T>) T {
    raise E{}
    return b.value
}
fn main() {
    let b = Box<int> { value: 42 }
    unbox<int>(b)
}
"#,
        MUST_HANDLE,
    );
}

#[test]
fn generic_fn_returns_generic_fallible() {
    compile_should_fail_with(
        r#"
error E{}
class Box<T> {
    value: T
}
fn wrap<T>(x: T) Box<T> {
    raise E{}
    return Box<T> { value: x }
}
fn main() {
    wrap(42)
}
"#,
        MUST_HANDLE,
    );
}

// Generics with explicit type arguments and errors
#[test]
fn explicit_type_arg_fallible() {
    compile_should_fail_with(
        r#"
error E{}
fn id<T>(x: T) T {
    raise E{}
    return x
}
fn main() {
    id<int>(42)
}
"#,
        MUST_HANDLE,
    );
}

#[test]
fn explicit_type_arg_different_error_sets() {
    compile_should_fail_with(
        r#"
error E1{}
error E2{}
fn process<T>(x: T) T {
    if true {
        raise E1{}
    }
    if false {
        raise E2{}
    }
    return x
}
fn main() {
    let a = process<int>(42) catch 0
    print(a)
    process<string>("hi")
}
"#,
        MUST_HANDLE,
    );
}

// Generic function that takes a closure but raises in its own body
#[test]
fn generic_with_closure_fallible() {
    compile_should_fail_with(
        r#"
error E{}
fn apply<T>(f: fn(T) T, x: T) T {
    if true {
        raise E{}
    }
    return f(x)
}
fn main() {
    apply((n: int) => n + 1, 42)
}
"#,
        MUST_HANDLE,
    );
}

// Fallibility flows through fn types: `fn() T` is an infallible contract, so
// passing a fallible function into it is rejected at the boundary; `fn() T!`
// accepts fallible values and its calls must be handled.
#[test]
fn generic_fn_calls_fallible() {
    compile_should_fail_with(
        r#"
error E{}
fn f() int {
    raise E{}
    return 1
}
fn wrap<T>(maker: fn() T) T {
    return maker()
}
fn main() {
    wrap(f)
}
"#,
        "cannot pass fallible function 'f' where an infallible function type is expected",
    );
}

#[test]
fn generic_closure_param_fallible() {
    compile_should_fail_with(
        r#"
error E{}
fn id(x: int) int {
    raise E{}
    return x
}
fn apply<T>(f: fn(T) T, x: T) T {
    return f(x)
}
fn main() {
    apply(id, 42)
}
"#,
        "cannot pass fallible function 'id' where an infallible function type is expected",
    );
}

#[test]
fn generic_fallible_fn_contract_enforced_at_call_site() {
    compile_should_fail_with(
        r#"
error E{}
fn f() int {
    raise E{}
    return 1
}
fn wrap<T>(maker: fn() T!) T {
    return maker()!
}
fn main() {
    wrap(f)
}
"#,
        MUST_HANDLE,
    );
}
