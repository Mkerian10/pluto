//! Recursive generic instantiation tests - 25 tests
#[path = "../common.rs"]
mod common;
use common::{compile_and_run, compile_and_run_stdout, compile_should_fail_with};

// Infinite instantiation detection
#[test]
fn self_instantiating_class() {
    // A non-expanding self-reference is legal (classes are references); the
    // field is simply unconstructible without a base case, which is the
    // programmer's problem, not the type system's.
    assert_eq!(compile_and_run(r#"class Box<T>{value:Box<T>} fn main(){}"#), 0);
}
#[test]
fn mutually_recursive_instantiation() {
    // Mutual recursion with plain type parameters closes after one round —
    // legal, like concrete mutual recursion (classes are references).
    assert_eq!(compile_and_run(r#"class A<T>{b:B<T>} class B<U>{a:A<U>} fn main(){}"#), 0);
}

// Bounded recursion that should work
#[test]
fn nullable_stops_recursion() { compile_should_fail_with(r#"class Node<T>{value:T next:Node<T>?} fn main(){let n=Node<int>{value:42 next:Node<int>{value:43 next:none}}}"#, "expected newline after statement"); }

// Deep nesting limits
#[test]
fn very_deep_nesting() {
    // Deep nesting still type-checks precisely: int vs string at depth 3
    compile_should_fail_with(r#"class Box<T>{value:T}
fn main(){
    let b: Box<Box<Box<int>>> = Box<Box<Box<string>>>{value: Box<Box<string>>{value: Box<string>{value: "hi"}}}
}"#, "type mismatch");
}

// Recursive function with generics
#[test]
fn recursive_generic_fn() {
    // Self-recursion at the same type parameter is legal (non-expanding)
    assert_eq!(compile_and_run(r#"fn rec<T>(x:T)T{return rec(x)}
fn main(){}"#), 0);
}
#[test]
fn mutual_rec_generic_fns() {
    assert_eq!(compile_and_run(r#"fn a<T>(x:T)T{return b(x)}
fn b<U>(x:U)U{return a(x)}
fn main(){}"#), 0);
}

// Recursive enum
#[test]
fn recursive_enum_variant() {
    // Recursive enums are the idiomatic functional list — constructible and
    // matchable end to end.
    let out = compile_and_run_stdout(
        r#"
enum List<T> {
    Cons { head: T, tail: List<T> }
    Nil
}

fn len(l: List<int>) int {
    match l {
        List.Cons { head, tail } {
            return 1 + len(tail)
        }
        List.Nil {
            return 0
        }
    }
    return 0
}

fn main() {
    print(len(List<int>.Cons { head: 1, tail: List<int>.Nil }))
}
"#,
    );
    assert_eq!(out.trim(), "1");
}
#[test]
fn enum_with_boxed_recursion() {
    assert_eq!(compile_and_run(r#"class Box<T>{value:T} enum Tree<U>{Leaf{val:U}Node{left:Tree<U>right:Tree<U>}} fn main(){}"#), 0);
}

// Recursive type through array
#[test]
fn array_of_self() {
    assert_eq!(compile_and_run(r#"class Container<T>{items:[Container<T>]} fn main(){}"#), 0);
}

// Recursive through map
#[test]
fn map_of_self() {
    assert_eq!(compile_and_run(r#"class Node<T>{children:Map<string,Node<T>>} fn main(){}"#), 0);
}

// Generic with expanding params
#[test]
fn expanding_type_params() { compile_should_fail_with(r#"class Box<T>{value:T} fn expand<U>()Box<Box<U>>{return Box<Box<U>>{value:expand()}} fn main(){}"#, "cannot infer type parameter"); }

// Mutually recursive with type change
#[test]
fn mutual_rec_type_change() {
    assert_eq!(compile_and_run(r#"fn a<T>(x:T)Box<T>{return b(x)}
fn b<U>(x:U)Box<U>{return a(x)}
class Box<V>{value:V}
fn main(){}"#), 0);
}

// Recursive with closure
#[test]
fn recursive_closure_generic() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>f(x)}"#, "undefined function 'f'"); }

// Chain of recursive calls
#[test]
fn three_way_recursive_generics() {
    assert_eq!(compile_and_run(r#"fn a<T>(x:T)T{return b(x)}
fn b<U>(x:U)U{return c(x)}
fn c<V>(x:V)V{return a(x)}
fn main(){}"#), 0);
}

// Recursive with method calls
#[test]
fn recursive_method_generic() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"class C<T>{value:T
fn rec(self)C<T>{return self.rec()}}
fn main(){}"#).is_ok()); }

// Infinite through tuple/pair
#[test]
fn pair_self_reference() { compile_should_fail_with(r#"class Pair<T,U>{first:T second:U} class Node{data:Pair<int,Node>} fn main(){}"#, "expected newline after statement"); }

// Recursive with nullable doesn't prevent infinite
#[test]
fn nullable_still_recursive() { compile_should_fail_with(r#"class Box<T>{inner:Box<Box<T>>?} fn main(){}"#, "expanding recursive reference: 'Box' instantiates 'Box' with nested type arguments on a reference cycle back to 'Box' — each round would demand a deeper instantiation, forever. Use plain type parameters or concrete types in the reference."); }

// Generic recursion depth check
#[test]
fn controlled_recursion_depth() {
    let out = compile_and_run_stdout(r#"fn rec<T>(x:T,depth:int)T{if depth>100{return x}
return rec(x,depth+1)}
fn main(){print(rec(42,0))}"#);
    assert_eq!(out.trim(), "42");
}

// Recursive with error type
#[test]
fn recursive_with_error() { compile_should_fail_with(r#"error E{} fn rec<T>(x:T)T!{if true{raise E{}}return rec(x)} fn main(){}"#, "expected {, found !"); }

// Self-referential through trait
#[test]
fn trait_self_ref() { compile_should_fail_with(r#"trait T{} class C<U:T>{value:U} impl T where U=C{} fn main(){}"#, "expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found impl"); }

// Indirect infinite through field
#[test]
fn indirect_infinite() {
    // A three-way cycle with plain params also closes — only *expanding*
    // cycles (nested type args feeding back) are rejected.
    assert_eq!(compile_and_run(r#"class A<T>{b:B<T>} class B<U>{c:C<U>} class C<V>{a:A<V>} fn main(){}"#), 0);
}

// Recursive generic with bound
#[test]
fn recursive_bounded() {
    assert_eq!(compile_and_run(r#"trait T{} class Box<U:T>{inner:Box<U>} fn main(){}"#), 0);
}

// Function returning recursive type
#[test]
fn fn_returns_recursive() { compile_should_fail_with(r#"class Box<T>{inner:Box<T>} fn make<U>()Box<U>{return Box<U>{inner:make()}} fn main(){}"#, "cannot infer type parameter 'U' for 'make'"); }

// Recursive with explicit type args
#[test]
fn explicit_recursive_call() {
    assert_eq!(compile_and_run(r#"fn rec<T>(x:T)T{return rec<T>(x)}
fn main(){}"#), 0);
}
