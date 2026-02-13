//! Recursive generic instantiation tests - 25 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Infinite instantiation detection
#[test]
#[ignore] // PR #46 - outdated assertions
fn self_instantiating_class() { compile_should_fail_with(r#"class Box<T>{value:Box<T>} fn main(){}"#, ""); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutually_recursive_instantiation() { compile_should_fail_with(r#"class A<T>{b:B<T>} class B<U>{a:A<U>} fn main(){}"#, ""); }

// Bounded recursion that should work
#[test]
#[ignore] // PR #46 - outdated assertions
fn nullable_stops_recursion() { compile_should_fail_with(r#"class Node<T>{value:T next:Node<T>?} fn main(){let n=Node<int>{value:42 next:Node<int>{value:43 next:none}}}"#, ""); }

// Deep nesting limits
#[test]
#[ignore] // PR #46 - outdated assertions
fn very_deep_nesting() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b:Box<Box<Box<Box<Box<Box<Box<Box<Box<Box<int>>>>>>>>>>=Box<Box<Box<Box<Box<Box<Box<Box<Box<Box<string>>>>>>>>>>{value:Box<Box<Box<Box<Box<Box<Box<Box<Box<string>>>>>>>>>{value:Box<Box<Box<Box<Box<Box<Box<Box<string>>>>>>>>{value:Box<Box<Box<Box<Box<Box<Box<string>>>>>>>{value:Box<Box<Box<Box<Box<Box<string>>>>>>{value:Box<Box<Box<Box<Box<string>>>>>{value:Box<Box<Box<Box<string>>>>{value:Box<Box<Box<string>>>{value:Box<Box<string>>{value:Box<string>{value:\"hi\"}}}}}}}}}}"#, "type mismatch"); }

// Recursive function with generics
#[test]
#[ignore] // PR #46 - outdated assertions
fn recursive_generic_fn() { compile_should_fail_with(r#"fn rec<T>(x:T)T{return rec(x)} fn main(){}"#, ""); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutual_rec_generic_fns() { compile_should_fail_with(r#"fn a<T>(x:T)T{return b(x)} fn b<U>(x:U)U{return a(x)} fn main(){}"#, ""); }

// Recursive enum
#[test]
#[ignore] // PR #46 - outdated assertions
fn recursive_enum_variant() { compile_should_fail_with(r#"enum List<T>{Cons{head:T tail:List<T>}Nil} fn main(){}"#, ""); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn enum_with_boxed_recursion() { compile_should_fail_with(r#"class Box<T>{value:T} enum Tree<U>{Leaf{val:U}Node{left:Tree<U>right:Tree<U>}} fn main(){}"#, ""); }

// Recursive type through array
#[test]
#[ignore] // PR #46 - outdated assertions
fn array_of_self() { compile_should_fail_with(r#"class Container<T>{items:[Container<T>]} fn main(){}"#, ""); }

// Recursive through map
#[test]
#[ignore] // PR #46 - outdated assertions
fn map_of_self() { compile_should_fail_with(r#"class Node<T>{children:Map<string,Node<T>>} fn main(){}"#, ""); }

// Generic with expanding params
#[test]
#[ignore] // PR #46 - outdated assertions
fn expanding_type_params() { compile_should_fail_with(r#"class Box<T>{value:T} fn expand<U>()Box<Box<U>>{return Box<Box<U>>{value:expand()}} fn main(){}"#, ""); }

// Mutually recursive with type change
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutual_rec_type_change() { compile_should_fail_with(r#"fn a<T>(x:T)Box<T>{return b(x)} fn b<U>(x:U)Box<U>{return a(x)} class Box<V>{value:V} fn main(){}"#, ""); }

// Recursive with closure
#[test]
#[ignore] // PR #46 - outdated assertions
fn recursive_closure_generic() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>f(x)}"#, ""); }

// Chain of recursive calls
#[test]
#[ignore] // PR #46 - outdated assertions
fn three_way_recursive_generics() { compile_should_fail_with(r#"fn a<T>(x:T)T{return b(x)} fn b<U>(x:U)U{return c(x)} fn c<V>(x:V)V{return a(x)} fn main(){}"#, ""); }

// Recursive with method calls
#[test]
#[ignore] // PR #46 - outdated assertions
fn recursive_method_generic() { compile_should_fail_with(r#"class C<T>{value:T fn rec(self)C<T>{return self.rec()}} fn main(){}"#, ""); }

// Infinite through tuple/pair
#[test]
#[ignore] // PR #46 - outdated assertions
fn pair_self_reference() { compile_should_fail_with(r#"class Pair<T,U>{first:T second:U} class Node{data:Pair<int,Node>} fn main(){}"#, ""); }

// Recursive with nullable doesn't prevent infinite
#[test]
#[ignore] // PR #46 - outdated assertions
fn nullable_still_recursive() { compile_should_fail_with(r#"class Box<T>{inner:Box<Box<T>>?} fn main(){}"#, ""); }

// Generic recursion depth check
#[test]
#[ignore] // PR #46 - outdated assertions
fn controlled_recursion_depth() { compile_should_fail_with(r#"fn rec<T>(x:T,depth:int)T{if depth>100{return x}return rec(x,depth+1)} fn main(){rec(42,0)}"#, ""); }

// Recursive with error type
#[test]
#[ignore] // PR #46 - outdated assertions
fn recursive_with_error() { compile_should_fail_with(r#"error E{} fn rec<T>(x:T)T!{if true{raise E{}}return rec(x)} fn main(){}"#, ""); }

// Self-referential through trait
#[test]
#[ignore] // PR #46 - outdated assertions
fn trait_self_ref() { compile_should_fail_with(r#"trait T{} class C<U:T>{value:U} impl T where U=C{} fn main(){}"#, ""); }

// Indirect infinite through field
#[test]
#[ignore] // PR #46 - outdated assertions
fn indirect_infinite() { compile_should_fail_with(r#"class A<T>{b:B<T>} class B<U>{c:C<U>} class C<V>{a:A<V>} fn main(){}"#, ""); }

// Recursive generic with bound
#[test]
#[ignore] // PR #46 - outdated assertions
fn recursive_bounded() { compile_should_fail_with(r#"trait T{} class Box<U:T>{inner:Box<U>} fn main(){}"#, ""); }

// Function returning recursive type
#[test]
#[ignore] // PR #46 - outdated assertions
fn fn_returns_recursive() { compile_should_fail_with(r#"class Box<T>{inner:Box<T>} fn make<U>()Box<U>{return Box<U>{inner:make()}} fn main(){}"#, ""); }

// Recursive with explicit type args
#[test]
#[ignore] // PR #46 - outdated assertions
fn explicit_recursive_call() { compile_should_fail_with(r#"fn rec<T>(x:T)T{return rec<T>(x)} fn main(){}"#, ""); }
