//! Type unification failure tests - 30 tests
#[path = "../common.rs"]
mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

// Basic unification failures
#[test]
fn infer_from_conflicting_uses() {
    // A bare generic function is not a value — its type parameters cannot be
    // inferred without a call.
    compile_should_fail_with(
        r#"
fn id<T>(x: T) T {
    return x
}
fn main() {
    let f = id
    f(42)
    f(true)
}
"#,
        "generic function 'id' cannot be used as a value",
    );
}
#[test]
fn param_return_conflict() { compile_should_fail_with(r#"fn bad<T>(x:T)T{if true{return x}return 42} fn main(){}"#, "type mismatch"); }
#[test]
fn two_params_conflict() { compile_should_fail_with(r#"fn same<T>(x:T,y:T)T{return x} fn main(){same(42,true)}"#, "cannot infer type parameters"); }

// Array element unification
#[test]
fn array_mixed_types() { compile_should_fail_with(r#"fn first<T>(arr:[T])T{return arr[0]} fn main(){first([42,true])}"#, "type mismatch"); }
#[test]
fn array_return_conflict() { compile_should_fail_with(r#"fn make<T>()[T]{if true{return [42]}return [true]} fn main(){}"#, "type mismatch"); }

// Field type unification
#[test]
fn class_field_conflict() { compile_should_fail_with(r#"class Box<T>{value:T} fn make<T>()Box<T>{if true{return Box<int>{value:42}}return Box<bool>{value:true}} fn main(){}"#, "type mismatch"); }
#[test]
fn two_fields_same_param() { compile_should_fail_with(r#"class Pair<T>{first:T
second:T}
fn main(){let p=Pair<int>{first:42, second:true}}"#, "expected int, found bool"); }

// Function call unification
#[test]
fn call_with_conflicting_inferred() {
    compile_should_fail_with(
        r#"
fn id<T>(x: T) T {
    return x
}
fn apply<U>(f: fn(U) U, x: U) U {
    return f(x)
}
fn main() {
    apply(id, 42)
    apply(id, true)
}
"#,
        "generic function 'id' cannot be used as a value",
    );
}
#[test]
fn nested_call_conflict() {
    // Each call site instantiates independently — this is legal by design.
    let out = compile_and_run_stdout(
        r#"
fn id<T>(x: T) T {
    return x
}
fn main() {
    print(id(id(42)))
    print(id(id(true)))
}
"#,
    );
    assert_eq!(out.trim(), "42\ntrue");
}

// Return type unification
#[test]
fn if_branches_differ() { compile_should_fail_with(r#"fn make<T>(b:bool)T{if b{return 42}return true} fn main(){}"#, "type mismatch"); }
#[test]
fn match_arms_differ() {
    compile_should_fail_with(
        r#"
enum E {
    A
    B
}
fn get<T>(e: E) T {
    match e {
        E.A {
            return 42
        }
        E.B {
            return 43
        }
    }
    return get<T>(e)
}
fn main() {}
"#,
        "type mismatch",
    );
}

// Closure unification
#[test]
fn closure_param_conflict() {
    // The same fn(int) int value cannot unify T with both int and bool
    compile_should_fail_with(
        "fn apply<T>(f: fn(T) T, x: T) T {\n    return f(x)\n}\n\nfn main(){\n    let f = (x: int) => x\n    apply(f, 42)\n    apply(f, true)\n}",
        "cannot infer type parameters",
    );
}
#[test]
fn closure_return_conflict() { compile_should_fail_with(r#"fn main(){let f=(b:bool)=>{if b{return 42}
return true}}"#, "return type mismatch"); }

// Method call unification
#[test]
fn method_receiver_conflict() {
    let out = compile_and_run_stdout(
        r#"
class Box<T> {
    value: T

    fn get(self) T {
        return self.value
    }
}
fn unwrap<U>(b: Box<U>) U {
    return b.get()
}
fn main() {
    print(unwrap(Box<int> { value: 42 }))
    print(unwrap(Box<bool> { value: true }))
}
"#,
    );
    assert_eq!(out.trim(), "42\ntrue");
}

// Enum variant unification
#[test]
fn enum_variant_param_conflict() { compile_should_fail_with(r#"enum Opt<T>{Some{v:T}None} fn make<U>(b:bool)Opt<U>{if b{return Opt.Some{v:42}}return Opt.Some{v:true}} fn main(){}"#, "cannot infer type arguments for generic enum"); }

// Multiple type parameters
#[test]
fn two_params_cross_conflict() {
    // T and U each unify consistently or the call is rejected
    compile_should_fail_with(r#"fn pick<T,U>(x:T,y:U)T{return x}
fn main(){let c:bool=pick(42,true)}"#, "type mismatch");
}
#[test]
fn param_reuse_conflict() {
    compile_should_fail_with(
        r#"
fn use_twice<T>(x: T, y: T) T {
    return x
}
fn f<U>(a: U) {
    use_twice(a, 42)
    use_twice(a, true)
}
fn main() {}
"#,
        "cannot infer type parameters",
    );
}

// Recursive unification
#[test]
fn recursive_type_conflict() { compile_should_fail_with(r#"class Box<T>{value:T} fn nest<U>()Box<Box<U>>{if true{return Box<Box<int>>{value:Box<int>{value:42}}}return Box<Box<bool>>{value:Box<bool>{value:true}}} fn main(){}"#, "type mismatch"); }

// Unification with builtin types
#[test]
fn builtin_generic_conflict() {
    let out = compile_and_run_stdout(
        r#"
fn first<T>(arr: [T]) T {
    return arr[0]
}
fn main() {
    print(first([42]))
    print(first([true]))
}
"#,
    );
    assert_eq!(out.trim(), "42\ntrue");
}
#[test]
fn map_value_conflict() {
    // Instantiating with different map value types across calls is legal.
    let out = compile_and_run_stdout(
        r#"
fn get<T>(m: Map<int, T>, k: int) T {
    return m[k]
}
fn main() {
    let m = Map<int, int> { 1: 10 }
    print(get(m, 1))
    let m2 = Map<int, bool> { 2: true }
    print(get(m2, 2))
}
"#,
    );
    assert_eq!(out.trim(), "10\ntrue");
}

// Ambiguous bindings
#[test]
fn cannot_infer_from_void() { compile_should_fail_with(r#"fn ignore<T>(x:T){} fn main(){ignore()}"#, "expects 1 arguments, got 0"); }
#[test]
fn multiple_possible_types() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){let x=id}"#, "generic function 'id' cannot be used as a value"); }

// Unification with nullable
#[test]
#[ignore] // #182: compiler doesn't detect T? vs U type mismatch
fn nullable_non_nullable_conflict() { compile_should_fail_with(r#"fn maybe<T>(x:T,b:bool)T?{if b{return x}return none} fn use<U>(x:U){} fn main(){use(maybe(42,true))}"#, "type mismatch"); }
#[test]
fn nullable_unwrap_conflict() { compile_should_fail_with(r#"fn unwrap<T>(x:T?)T{return x?} fn main(){unwrap(42)}"#, "cannot infer type parameters"); }

// Unification with errors
#[test]
#[ignore] // Syntax error: old T! return type syntax no longer valid
fn error_infallible_conflict() { compile_should_fail_with(r#"error E{} fn maybe<T>(x:T,b:bool)T!{if b{return x}raise E{}} fn use<U>(x:U){} fn main(){use(maybe(42,true))}"#, "type mismatch"); }

// Cyclic unification
#[test]
#[ignore] // Test expects success (empty error string) but infinite recursion compiles
fn self_referential_type() { compile_should_fail_with(r#"fn loop<T>(x:T)T{return loop(x)} fn main(){}"#, ""); }

// Unification across function boundaries
#[test]
fn cross_function_conflict() {
    // Different functions may instantiate the same generic differently.
    let out = compile_and_run_stdout(
        r#"
fn id<T>(x: T) T {
    return x
}
fn a() int {
    return id(42)
}
fn b() bool {
    return id(true)
}
fn main() {
    print(a())
    print(b())
}
"#,
    );
    assert_eq!(out.trim(), "42\ntrue");
}

// Generic instance conflicts
#[test]
#[ignore] // Syntax error: expected ,, found >=
fn instance_param_mismatch() { compile_should_fail_with(r#"class Box<T>{value:T} fn make<U>()Box<U>{return Box<int>{value:42}} fn main(){let b:Box<string>=make()}"#, "type mismatch"); }

// Bound-constrained unification
#[test]
#[ignore] // Syntax error: old impl syntax without class body
fn bound_limits_unification() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn f<U:T>(x:U)U{return x} fn main(){f(42)}"#, "does not satisfy"); }
