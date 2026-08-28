//! Generic DI tests.
//!
//! Rewritten by the vacuous-test audit: the old sources declared both an app
//! and a top-level main, so every should-fail expectation was satisfied by
//! that scaffolding error instead of the behavior in the test name.
#[path = "../common.rs"]
mod common;
use common::{compile_and_run, compile_should_fail_with};

// Injecting a generic instantiation and using its typed field
#[test]
fn generic_class_di_mismatch() { compile_should_fail_with(r#"class Dep{x:int} class Repo<T>[dep:Dep]{value:T} app MyApp[repo:Repo<int>]{fn main(self){let s:string=self.repo.value}}"#, "type mismatch"); }
#[test]
fn bracket_dep_wrong_type() { compile_should_fail_with(r#"class Dep{x:int} class Repo<T>[dep:UndefinedDep]{value:T} app MyApp{fn main(self){}}"#, "unknown type 'UndefinedDep'"); }

// Multiple generic DI classes coexist
#[test]
fn two_generic_di_classes() {
    assert_eq!(compile_and_run(r#"class Dep{x:int} class Repo1<T>[dep:Dep]{value:T} class Repo2<U>[dep:Dep]{data:U} app MyApp[r1:Repo1<int>, r2:Repo2<string>]{fn main(self){}}"#), 0);
}

// Generic DI with type bounds — enforced at the injection site
#[test]
fn generic_di_bound_not_satisfied() { compile_should_fail_with(r#"trait T{fn m(self) int} class Dep{x:int} class Repo<U:T>[dep:Dep]{value:U} app MyApp[repo:Repo<int>]{fn main(self){}}"#, "does not satisfy"); }

// DI cycle with generics
#[test]
fn generic_di_cycle() { compile_should_fail_with(r#"class A<T>[b:B<T>]{}
class B<U>[a:A<U>]{}
app MyApp[a: A<int>]{fn main(self){}}"#, "circular"); }

// Generic class injected into non-generic — legal
#[test]
fn inject_generic_into_regular() {
    assert_eq!(compile_and_run(r#"class Box<T>{value:T} class Service[box:Box<int>]{} app MyApp[s:Service]{fn main(self){}}"#), 0);
}

// Non-instantiated generic in DI
#[test]
fn di_generic_without_concrete() { compile_should_fail_with(r#"class Dep{x:int} class Repo<T>[dep:Dep]{value:T} class Service[repo:Repo]{} app MyApp{fn main(self){}}"#, "unknown type 'Repo'"); }

// Two instantiations of the same generic in one DI graph — legal
#[test]
fn two_instances_same_generic() {
    assert_eq!(compile_and_run(r#"class Dep{x:int} class Repo<T>[dep:Dep]{value:T} class Service[repo1:Repo<int>, repo2:Repo<string>]{} app MyApp[s:Service]{fn main(self){}}"#), 0);
}

// Declaring a generic scoped DI class is legal (created via scope blocks)
#[test]
fn scoped_generic_di() {
    assert_eq!(compile_and_run(r#"class Dep{x:int} scoped class Handler<T>[dep:Dep]{value:T} app MyApp{fn main(self){}}"#), 0);
}

// Generic apps are not supported
#[test]
fn generic_app_invalid() { compile_should_fail_with(r#"app MyApp<T>{fn main(self){}}"#, "expected {, found <"); }

// Forward references in generic DI are legal (registration is order-independent)
#[test]
fn forward_ref_generic_di() {
    assert_eq!(compile_and_run(r#"class Repo<T>[dep:Dep]{value:T} class Dep{x:int} app MyApp[r:Repo<int>]{fn main(self){}}"#), 0);
}

// A second bracket group is invalid syntax
#[test]
fn generic_multiple_bracket_deps() { compile_should_fail_with(r#"class Dep1{x:int} class Dep2{y:string} class Repo<T>[dep1:Dep1][dep2:Dep2]{value:T} app MyApp{fn main(self){}}"#, "expected {, found ["); }

// Generic class constructor blocked
#[test]
fn manual_construct_generic_di() { compile_should_fail_with(r#"class Dep{x:int} class Repo<T>[dep:Dep]{value:T}
fn main(){let r=Repo<int>{value:42}}"#, "cannot manually construct"); }

// DI with nested generics — legal
#[test]
fn nested_generic_di() {
    assert_eq!(compile_and_run(r#"class Dep{x:int} class Box<T>{value:T} class Repo<U>[dep:Dep]{data:Box<U>} app MyApp[r:Repo<int>]{fn main(self){}}"#), 0);
}

// Enums cannot be injected: DI wires class singletons
#[test]
fn enum_in_di() { compile_should_fail_with(r#"enum Opt<T>{Some{v:T}None} class Service[opt:Opt<int>]{} app MyApp[s:Service]{fn main(self){}}"#, "must be a class type"); }

// Traits cannot be injected: DI is auto-wired by specific class type
#[test]
fn trait_in_di() { compile_should_fail_with(r#"trait T{fn m(self) int} class Service[t:T]{} app MyApp[s:Service]{fn main(self){}}"#, "must be a class type"); }

// Distinct instantiations in different services — legal, distinct singletons
#[test]
fn di_instantiation_conflict() {
    assert_eq!(compile_and_run(r#"class Dep{x:int} class Repo<T>[dep:Dep]{value:T} class S1[repo:Repo<int>]{} class S2[repo:Repo<string>]{} app MyApp[a:S1, b:S2]{fn main(self){}}"#), 0);
}

// Self-referential generic dep is a DI cycle
#[test]
fn self_ref_generic_di() { compile_should_fail_with(r#"class Node<T>[next:Node<T>]{value:T} app MyApp[n:Node<int>]{fn main(self){}}"#, "circular"); }

// DI graph with generic type params
#[test]
fn di_graph_type_param() {
    // A generic fn over a DI-bearing generic class is fine to declare
    assert_eq!(compile_and_run(r#"class Dep{x:int}
class Repo<T>[dep:Dep]{value:T}
fn consume<U>(r:Repo<U>){}
fn main(){}"#), 0);
}

// Multiple bracket deps with generics — legal with comma separation
#[test]
fn generic_multi_bracket() {
    assert_eq!(compile_and_run(r#"class Dep1{x:int} class Dep2{y:int} class Repo<T>[dep1:Dep1, dep2:Dep2]{value:T} app MyApp[r:Repo<int>]{fn main(self){}}"#), 0);
}
