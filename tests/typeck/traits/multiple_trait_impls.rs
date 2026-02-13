//! Multiple trait implementation tests - 25 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Conflicting method signatures
#[test]
fn two_traits_same_method_diff_sig() { compile_should_fail_with(r#"trait T1{fn foo(self)int} trait T2{fn foo(self)string} class C{} impl T1{fn foo(self)int{return 1}} impl T2{fn foo(self)string{return "hi"}} fn main(){}"#, ""); }
#[test]
fn two_traits_same_method_diff_params() { compile_should_fail_with(r#"trait T1{fn foo(self,x:int)} trait T2{fn foo(self,x:string)} class C{} impl T1{fn foo(self,x:int){}} impl T2{fn foo(self,x:string){}} fn main(){}"#, ""); }

// Conflicting method names
#[test]
fn three_traits_name_collision() { compile_should_fail_with(r#"trait T1{fn foo(self)} trait T2{fn foo(self)} trait T3{fn foo(self)} class C{} impl T1{fn foo(self){}} impl T2{fn foo(self){}} impl T3{fn foo(self){}} fn main(){}"#, ""); }

// One impl missing from multiple
#[test]
fn two_traits_one_incomplete() { compile_should_fail_with(r#"trait T1{fn foo(self)} trait T2{fn bar(self)} class C{} impl T1{fn foo(self){}} impl T2{} fn main(){}"#, "missing method"); }

// Overlapping method requirements
#[test]
fn two_traits_compatible_methods() { compile_should_fail_with(r#"trait T1{fn foo(self)int} trait T2{fn foo(self)int} class C{} impl T1{fn foo(self)int{return 1}} impl T2{fn foo(self)int{return 2}} fn main(){}"#, ""); }

// Generic traits with same method
#[test]
fn two_generic_traits_conflict() { compile_should_fail_with(r#"trait T1<U>{fn foo(self)U} trait T2<V>{fn foo(self)V} class C{} impl T1<int>{fn foo(self)int{return 1}} impl T2<string>{fn foo(self)string{return "hi"}} fn main(){}"#, ""); }

// Contract conflicts between traits
#[test]
fn two_traits_conflicting_contracts() { compile_should_fail_with(r#"trait T1{fn foo(self)int requires true} trait T2{fn foo(self)int requires false} class C{} impl T1{fn foo(self)int{return 1}} impl T2{fn foo(self)int{return 1}} fn main(){}"#, ""); }

// Method from one trait, wrong impl
#[test]
fn impl_t1_method_for_t2() { compile_should_fail_with(r#"trait T1{fn foo(self)} trait T2{fn bar(self)} class C{} impl T1{fn bar(self){}} impl T2{fn foo(self){}} fn main(){}"#, "missing method"); }

// Diamond problem (if traits could extend)
#[test]
fn diamond_trait_hierarchy() { compile_should_fail_with(r#"trait Base{fn foo(self)} trait Left{fn foo(self)} trait Right{fn foo(self)} class C{} impl Base{fn foo(self){}} impl Left{fn foo(self){}} impl Right{fn foo(self){}} fn main(){}"#, ""); }

// Trait implemented twice
#[test]
fn duplicate_trait_impl() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn foo(self){}} impl T{fn foo(self){}} fn main(){}"#, ""); }

// Generic class with multiple traits
#[test]
fn generic_class_two_traits() { compile_should_fail_with(r#"trait T1{fn foo(self)} trait T2{fn bar(self)} class Box<U>{value:U} impl T1{fn foo(self){}} impl T2{} fn main(){}"#, "missing method"); }

// Trait with contract, impl for multiple classes
#[test]
fn contract_trait_two_classes() { compile_should_fail_with(r#"trait T{fn foo(self)int ensures result>0} class C1{} impl T{fn foo(self)int{return -1}} class C2{} impl T{fn foo(self)int{return 1}} fn main(){}"#, ""); }

// Multiple traits, some missing methods
#[test]
fn three_traits_partial_impl() { compile_should_fail_with(r#"trait T1{fn a(self)} trait T2{fn b(self)} trait T3{fn c(self)} class C{} impl T1{fn a(self){}} impl T2{fn b(self){}} impl T3{} fn main(){}"#, "missing method"); }

// Trait composition with method overlap
#[test]
fn composed_traits_overlap() { compile_should_fail_with(r#"trait T1{fn foo(self) fn bar(self)} trait T2{fn bar(self) fn baz(self)} class C{} impl T1{fn foo(self){} fn bar(self){}} impl T2{fn bar(self){} fn baz(self){}} fn main(){}"#, ""); }

// Nullable method in one trait, non-nullable in another
#[test]
fn nullable_conflict() { compile_should_fail_with(r#"trait T1{fn foo(self)int?} trait T2{fn foo(self)int} class C{} impl T1{fn foo(self)int?{return none}} impl T2{fn foo(self)int{return 1}} fn main(){}"#, ""); }

// Error method in one trait, infallible in another
#[test]
fn error_conflict() { compile_should_fail_with(r#"error E{} trait T1{fn foo(self)int!} trait T2{fn foo(self)int} class C{} impl T1{fn foo(self)int!{raise E{}}} impl T2{fn foo(self)int{return 1}} fn main(){}"#, ""); }

// Generic method in both traits
#[test]
fn two_traits_generic_methods() { compile_should_fail_with(r#"trait T1{fn foo<U>(self,x:U)U} trait T2{fn foo<V>(self,x:V)V} class C{} impl T1{fn foo<U>(self,x:U)U{return x}} impl T2{fn foo<V>(self,x:V)V{return x}} fn main(){}"#, ""); }

// Mut self in one, non-mut in another
#[test]
fn mut_self_conflict() { compile_should_fail_with(r#"trait T1{fn foo(mut self)} trait T2{fn foo(self)} class C{} impl T1{fn foo(mut self){}} impl T2{fn foo(self){}} fn main(){}"#, ""); }

// Static method collision (if supported)
#[test]
fn static_method_conflict() { compile_should_fail_with(r#"trait T1{fn create()C} trait T2{fn create()C} class C{} impl T1{fn create()C{return C{}}} impl T2{fn create()C{return C{}}} fn main(){}"#, ""); }

// Trait with same name, different packages (if supported)
#[test]
fn same_trait_name_collision() { compile_should_fail_with(r#"trait T{fn foo(self)} trait T{fn bar(self)} class C{} impl T{fn foo(self){}} fn main(){}"#, ""); }

// Multiple traits on multiple classes
#[test]
fn cross_class_trait_error() { compile_should_fail_with(r#"trait T1{fn foo(self)} trait T2{fn bar(self)} class C1{} impl T1{fn foo(self){}} class C2{} impl T2{} fn main(){}"#, "missing method"); }

// Trait method overloading (not supported)
#[test]
fn trait_method_overload() { compile_should_fail_with(r#"trait T{fn foo(self) fn foo(self,x:int)} class C{} impl T{fn foo(self){} fn foo(self,x:int){}} fn main(){}"#, ""); }

// Trait impl on enum
#[test]
fn multiple_traits_on_enum() { compile_should_fail_with(r#"trait T1{fn foo(self)} trait T2{fn bar(self)} enum E{A} impl T1{fn foo(self){}} impl T2{} fn main(){}"#, ""); }

// Trait with invariant vs impl method
#[test]
fn trait_method_violates_invariant() { compile_should_fail_with(r#"trait T{fn foo(self)int ensures result>0} class C{x:int invariant self.x<0} impl T{fn foo(self)int{return self.x}} fn main(){}"#, ""); }

// Partial overlap in method sets
#[test]
fn traits_partial_overlap() { compile_should_fail_with(r#"trait T1{fn foo(self) fn bar(self)} trait T2{fn bar(self) fn baz(self)} class C{} impl T1{fn foo(self){}} impl T2{fn baz(self){}} fn main(){}"#, "missing method"); }
