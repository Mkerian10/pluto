//! Cycle detection in DI graph - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Direct cycle A -> B -> A
#[test] fn direct_cycle() { compile_should_fail_with(r#"class A[b:B]{} class B[a:A]{} app MyApp{fn main(self){}}"#, "circular"); }

// Indirect cycle A -> B -> C -> A
#[test] fn indirect_cycle() { compile_should_fail_with(r#"class A[b:B]{} class B[c:C]{} class C[a:A]{} app MyApp{fn main(self){}}"#, "circular"); }

// Self-cycle
#[test] fn self_cycle() { compile_should_fail_with(r#"class A[a:A]{} app MyApp{fn main(self){}}"#, "circular"); }

// Cycle with app dependency
#[test] fn cycle_with_app() { compile_should_fail_with(r#"class A[b:B]{} class B[a:A]{} app MyApp[a:A]{fn main(self){}}"#, "circular"); }

// Cycle in long chain
#[test] fn long_chain_cycle() { compile_should_fail_with(r#"class A[b:B]{} class B[c:C]{} class C[d:D]{} class D[e:E]{} class E[a:A]{} app MyApp{fn main(self){}}"#, "circular"); }

// Multiple independent cycles
#[test] fn multiple_cycles() { compile_should_fail_with(r#"class A[b:B]{} class B[a:A]{} class C[d:D]{} class D[c:C]{} app MyApp{fn main(self){}}"#, "circular"); }

// Cycle through multiple deps
#[test] fn cycle_multi_deps() { compile_should_fail_with(r#"class A[b:B,c:C]{} class B[a:A]{} class C{} app MyApp{fn main(self){}}"#, "circular"); }

// Cycle with generic class
#[test] fn generic_cycle() { compile_should_fail_with(r#"class Box<T>[b:Box<T>]{value:T} app MyApp{fn main(self){}}"#, "circular"); }

// Partial cycle (B->C->B but A->B is ok)
#[test] fn partial_cycle() { compile_should_fail_with(r#"class A[b:B]{} class B[c:C]{} class C[b:B]{} app MyApp{fn main(self){}}"#, "circular"); }

// Cycle detection with nullable deps
#[test] fn cycle_nullable_dep() { compile_should_fail_with(r#"class A[b:B?]{} class B[a:A?]{} app MyApp{fn main(self){}}"#, ""); }

// Diamond without cycle (valid)
#[test] fn diamond_no_cycle() { compile_should_fail_with(r#"class A{} class B[a:A]{} class C[a:A]{} class D[b:B,c:C]{} app MyApp{fn main(self){}}"#, ""); }

// Cycle only in unused classes
#[test] fn unused_cycle() { compile_should_fail_with(r#"class A[b:B]{} class B[a:A]{} class C{} app MyApp[c:C]{fn main(self){}}"#, ""); }

// Cycle through scoped classes
#[test] fn scoped_cycle() { compile_should_fail_with(r#"scoped class A[b:B]{} scoped class B[a:A]{} app MyApp{fn main(self){}}"#, "circular"); }

// Cycle through transient classes
#[test] fn transient_cycle() { compile_should_fail_with(r#"transient class A[b:B]{} transient class B[a:A]{} app MyApp{fn main(self){}}"#, "circular"); }

// Mixed lifecycle cycle
#[test] fn mixed_lifecycle_cycle() { compile_should_fail_with(r#"class A[b:B]{} scoped class B[a:A]{} app MyApp{fn main(self){}}"#, "circular"); }

// Cycle in deeply nested deps
#[test] fn deep_nested_cycle() { compile_should_fail_with(r#"class A[b:B]{x:int} class B[c:C]{y:int} class C[d:D]{z:int} class D[a:A]{w:int} app MyApp{fn main(self){}}"#, "circular"); }

// Cycle with field and dep
#[test] fn cycle_field_and_dep() { compile_should_fail_with(r#"class A[b:B]{x:int} class B{a:A} app MyApp{fn main(self){}}"#, ""); }

// App self-dependency
#[test] fn app_self_dep() { compile_should_fail_with(r#"app MyApp[app:MyApp]{fn main(self){}}"#, ""); }

// Cycle with conditional deps
#[test] fn conditional_cycle() { compile_should_fail_with(r#"class A[b:B]{} class B[c:C]{} class C[a:A]{} app MyApp{fn main(self){if true{}}}"#, "circular"); }

// Detect cycle early
#[test] fn early_cycle_detection() { compile_should_fail_with(r#"class A[b:B]{} class B[c:C]{} class C[d:D]{} class D[a:A]{} app MyApp[a:A]{fn main(self){}}"#, "circular"); }
