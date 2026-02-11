//! Dependency resolution errors - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Unresolved dependency type
#[test] fn unresolved_dep_type() { compile_should_fail_with(r#"class A[b:NonExistent]{} app MyApp{fn main(self){}}"#, "undefined"); }

// Wrong dependency type
#[test] fn wrong_dep_type() { compile_should_fail_with(r#"class A[b:int]{} app MyApp{fn main(self){}}"#, ""); }

// Dependency type is trait
#[test] fn dep_type_trait() { compile_should_fail_with(r#"trait T{} class A[t:T]{} app MyApp{fn main(self){}}"#, ""); }

// Dependency type is enum
#[test] fn dep_type_enum() { compile_should_fail_with(r#"enum E{A} class A[e:E]{} app MyApp{fn main(self){}}"#, ""); }

// Generic dependency unresolved
#[test] fn generic_dep_unresolved() { compile_should_fail_with(r#"class Box<T>[t:T]{value:T} app MyApp{fn main(self){}}"#, ""); }

// Ambiguous dependency resolution
#[test] fn ambiguous_dep() { compile_should_fail_with(r#"class A{} class A{} class B[a:A]{} app MyApp{fn main(self){}}"#, ""); }

// Dependency on abstract class (if supported)
#[test] fn dep_on_abstract() { compile_should_fail_with(r#"abstract class A{} class B[a:A]{} app MyApp{fn main(self){}}"#, ""); }

// Nullable dependency
#[test] fn nullable_dep() { compile_should_fail_with(r#"class A{} class B[a:A?]{} app MyApp{fn main(self){}}"#, ""); }

// Array dependency (not supported)
#[test] fn array_dep() { compile_should_fail_with(r#"class A{} class B[arr:[A]]{} app MyApp{fn main(self){}}"#, ""); }

// Map dependency (not supported)
#[test] fn map_dep() { compile_should_fail_with(r#"class A{} class B[m:Map<string,A>]{} app MyApp{fn main(self){}}"#, ""); }

// Function dependency (not supported)
#[test] fn function_dep() { compile_should_fail_with(r#"class A[f:(int)int]{} app MyApp{fn main(self){}}"#, ""); }

// Dependency name collision with field
#[test] fn dep_field_collision() { compile_should_fail_with(r#"class A{} class B[a:A]{a:int} app MyApp{fn main(self){}}"#, "already declared"); }

// Generic class dep missing type args
#[test] fn generic_dep_no_args() { compile_should_fail_with(r#"class Box<T>{value:T} class A[b:Box]{} app MyApp{fn main(self){}}"#, ""); }

// Generic class dep wrong type args
#[test] fn generic_dep_wrong_args() { compile_should_fail_with(r#"class Box<T>{value:T} class A[b:Box<int,string>]{} app MyApp{fn main(self){}}"#, ""); }

// Dependency on error type
#[test] fn dep_on_error() { compile_should_fail_with(r#"error E{} class A[e:E]{} app MyApp{fn main(self){}}"#, ""); }
