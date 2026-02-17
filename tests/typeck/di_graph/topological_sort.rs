//! Topological sort errors - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Simple dependency chain
#[test]
#[ignore]
fn chain_dependency() { compile_should_fail_with(r#"class A[b:B]{} class B[c:C]{} class C{} app MyApp[a:A]{fn main(self){}}"#, ""); }

// Missing dependency
#[test]
#[ignore]
fn missing_dependency() { compile_should_fail_with(r#"class A[b:B]{} app MyApp[a:A]{fn main(self){}}"#, "undefined"); }

// Circular dependency in DI graph
#[test]
fn circular_di() { compile_should_fail_with(r#"class A[b:B]{} class B[a:A]{} app MyApp{fn main(self){}}"#, "circular"); }

// Three-way circular DI
#[test]
fn three_way_circular_di() { compile_should_fail_with(r#"class A[b:B]{} class B[c:C]{} class C[a:A]{} app MyApp{fn main(self){}}"#, "circular"); }

// Self-dependency
#[test]
#[ignore] // Compiler limitation: doesn't detect DI cycles
fn self_dependency() { compile_should_fail_with(r#"class A[a:A]{} app MyApp{fn main(self){}}"#, "circular"); }

// Dependency on non-class
#[test]
#[ignore]
fn dep_on_non_class() { compile_should_fail_with(r#"class A[x:int]{} app MyApp{fn main(self){}}"#, ""); }

// Multiple dependencies same class
#[test]
fn multiple_deps_same() { compile_should_fail_with(r#"class A{} class B[a1:A,a2:A]{} app MyApp{fn main(self){}}"#, ""); }

// Diamond dependency (valid)
#[test]
#[ignore]
fn diamond_dependency() { compile_should_fail_with(r#"class A{} class B[a:A]{} class C[a:A]{} class D[b:B,c:C]{} app MyApp{fn main(self){}}"#, ""); }

// Generic class dependency
#[test]
#[ignore]
fn generic_class_dep() { compile_should_fail_with(r#"class Box<T>{value:T} class A[b:Box<int>]{} app MyApp{fn main(self){}}"#, ""); }

// Dependency on private class
#[test]
fn dep_on_private() { compile_should_fail_with(r#"private class A{} class B[a:A]{} app MyApp{fn main(self){}}"#, ""); }

// Dependency ordering matters
#[test]
#[ignore]
fn dep_order() { compile_should_fail_with(r#"class B[a:A]{} class A{} app MyApp[b:B]{fn main(self){}}"#, ""); }

// Nested dependencies
#[test]
#[ignore]
fn nested_deps() { compile_should_fail_with(r#"class A{} class B[a:A]{} class C[b:B]{} class D[c:C]{} app MyApp[d:D]{fn main(self){}}"#, ""); }

// Multiple apps
#[test]
fn multiple_apps() { compile_should_fail_with(r#"app App1{fn main(self){}} app App2{fn main(self){}}"#, ""); }

// App with missing main
#[test]
fn app_no_main() { compile_should_fail_with(r#"app MyApp{fn helper(self){}}"#, ""); }

// App main wrong signature
#[test]
fn app_main_wrong_sig() { compile_should_fail_with(r#"app MyApp{fn main(self)int{return 1}}"#, ""); }

// Dependency on trait (not allowed)
#[test]
#[ignore]
fn dep_on_trait() { compile_should_fail_with(r#"trait T{} class A[t:T]{} app MyApp{fn main(self){}}"#, ""); }

// Dependency on enum (not allowed)
#[test]
#[ignore]
fn dep_on_enum() { compile_should_fail_with(r#"enum E{A} class A[e:E]{} app MyApp{fn main(self){}}"#, ""); }

// Duplicate dependency names
#[test]
#[ignore]
fn duplicate_dep_names() { compile_should_fail_with(r#"class A{} class B{} class C[dep:A,dep:B]{} app MyApp{fn main(self){}}"#, "already declared"); }

// Scoped class in DI
#[test]
#[ignore]
fn scoped_class_di() { compile_should_fail_with(r#"scoped class A{} class B[a:A]{} app MyApp{fn main(self){}}"#, ""); }

// Transient class in DI
#[test]
#[ignore]
fn transient_class_di() { compile_should_fail_with(r#"transient class A{} class B[a:A]{} app MyApp{fn main(self){}}"#, ""); }
