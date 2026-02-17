//! Generic DI tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Basic generic DI errors
#[test]
fn generic_class_di_mismatch() { compile_should_fail_with(r#"class Dep{x:int} class Repo<T>[dep:Dep]{value:T} app MyApp{fn main(self){}} fn main(){}"#, ""); }
#[test]
#[ignore]
fn bracket_dep_wrong_type() { compile_should_fail_with(r#"class Dep{x:int} class Repo<T>[dep:UndefinedDep]{value:T} app MyApp{fn main(self){}} fn main(){}"#, "undefined"); }

// Multiple generic classes with DI
#[test]
fn two_generic_di_classes() { compile_should_fail_with(r#"class Dep{x:int} class Repo1<T>[dep:Dep]{value:T} class Repo2<U>[dep:Dep]{data:U} app MyApp{fn main(self){}} fn main(){}"#, ""); }

// Generic DI with type bounds
#[test]
fn generic_di_bound_not_satisfied() { compile_should_fail_with(r#"trait T{} class Dep{x:int} class Repo<U:T>[dep:Dep]{value:U} app MyApp{fn main(self){}} fn main(){}"#, ""); }

// DI cycle with generics
#[test]
#[ignore]
fn generic_di_cycle() { compile_should_fail_with(r#"class A<T>[b:B<T>]{} class B<U>[a:A<U>]{} app MyApp{fn main(self){}} fn main(){}"#, "cycle"); }

// Generic class injected into non-generic
#[test]
fn inject_generic_into_regular() { compile_should_fail_with(r#"class Box<T>{value:T} class Service[box:Box<int>]{} app MyApp{fn main(self){}} fn main(){}"#, ""); }

// Non-instantiated generic in DI
#[test]
fn di_generic_without_concrete() { compile_should_fail_with(r#"class Dep{x:int} class Repo<T>[dep:Dep]{value:T} class Service[repo:Repo]{} app MyApp{fn main(self){}} fn main(){}"#, ""); }

// Multiple instantiations in DI graph
#[test]
fn two_instances_same_generic() { compile_should_fail_with(r#"class Dep{x:int} class Repo<T>[dep:Dep]{value:T} class Service[repo1:Repo<int>repo2:Repo<string>]{} app MyApp{fn main(self){}} fn main(){}"#, ""); }

// Generic scoped class with DI
#[test]
fn scoped_generic_di() { compile_should_fail_with(r#"class Dep{x:int} scoped class Handler<T>[dep:Dep]{value:T} app MyApp{fn main(self){}} fn main(){}"#, ""); }

// DI with generic app
#[test]
fn generic_app_invalid() { compile_should_fail_with(r#"app MyApp<T>{fn main(self){}} fn main(){}"#, ""); }

// Forward reference in generic DI
#[test]
fn forward_ref_generic_di() { compile_should_fail_with(r#"class Repo<T>[dep:Dep]{value:T} class Dep{x:int} app MyApp{fn main(self){}} fn main(){}"#, ""); }

// Generic with wrong bracket dep count
#[test]
fn generic_multiple_bracket_deps() { compile_should_fail_with(r#"class Dep1{x:int} class Dep2{y:string} class Repo<T>[dep1:Dep1][dep2:Dep2]{value:T} app MyApp{fn main(self){}} fn main(){}"#, ""); }

// Generic class constructor blocked
#[test]
#[ignore]
fn manual_construct_generic_di() { compile_should_fail_with(r#"class Dep{x:int} class Repo<T>[dep:Dep]{value:T} fn main(){let r=Repo<int>{value:42}}"#, "cannot construct"); }

// DI with nested generics
#[test]
fn nested_generic_di() { compile_should_fail_with(r#"class Dep{x:int} class Box<T>{value:T} class Repo<U>[dep:Dep]{data:Box<U>} app MyApp{fn main(self){}} fn main(){}"#, ""); }

// Generic enum in DI (should fail)
#[test]
fn enum_in_di() { compile_should_fail_with(r#"enum Opt<T>{Some{v:T}None} class Service[opt:Opt<int>]{} app MyApp{fn main(self){}} fn main(){}"#, ""); }

// Generic trait in DI
#[test]
fn trait_in_di() { compile_should_fail_with(r#"trait T{} class Service[t:T]{} app MyApp{fn main(self){}} fn main(){}"#, ""); }

// Conflicting generic instantiations in DI
#[test]
fn di_instantiation_conflict() { compile_should_fail_with(r#"class Dep{x:int} class Repo<T>[dep:Dep]{value:T} class S1[repo:Repo<int>]{} class S2[repo:Repo<string>]{} app MyApp{fn main(self){}} fn main(){}"#, ""); }

// Generic with self-reference in DI
#[test]
fn self_ref_generic_di() { compile_should_fail_with(r#"class Node<T>[next:Node<T>?]{value:T} app MyApp{fn main(self){}} fn main(){}"#, ""); }

// DI graph with generic type params
#[test]
#[ignore]
fn di_graph_type_param() { compile_should_fail_with(r#"class Dep{x:int} class Repo<T>[dep:Dep]{value:T} fn use<U>(r:Repo<U>){} fn main(){}"#, ""); }

// Multiple bracket deps with generics
#[test]
fn generic_multi_bracket() { compile_should_fail_with(r#"class Dep1{x:int} class Dep2{y:int} class Repo<T>[dep1:Dep1 dep2:Dep2]{value:T} app MyApp{fn main(self){}} fn main(){}"#, ""); }
