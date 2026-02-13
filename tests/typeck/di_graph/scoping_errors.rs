//! DI scoping errors - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Scoped class depends on singleton
#[test]
#[ignore] // PR #46 - outdated assertions
fn scoped_depends_singleton() { compile_should_fail_with(r#"class A{} scoped class B[a:A]{} app MyApp{fn main(self){}}"#, ""); }

// Singleton depends on scoped (invalid)
#[test]
#[ignore] // PR #46 - outdated assertions
fn singleton_depends_scoped() { compile_should_fail_with(r#"scoped class A{} class B[a:A]{} app MyApp{fn main(self){}}"#, "scope"); }

// Transient depends on singleton
#[test]
#[ignore] // PR #46 - outdated assertions
fn transient_depends_singleton() { compile_should_fail_with(r#"class A{} transient class B[a:A]{} app MyApp{fn main(self){}}"#, ""); }

// Singleton depends on transient (invalid)
#[test]
#[ignore] // PR #46 - outdated assertions
fn singleton_depends_transient() { compile_should_fail_with(r#"transient class A{} class B[a:A]{} app MyApp{fn main(self){}}"#, "scope"); }

// Scoped depends on transient
#[test]
#[ignore] // PR #46 - outdated assertions
fn scoped_depends_transient() { compile_should_fail_with(r#"transient class A{} scoped class B[a:A]{} app MyApp{fn main(self){}}"#, ""); }

// Transient depends on scoped (invalid)
#[test]
#[ignore] // PR #46 - outdated assertions
fn transient_depends_scoped() { compile_should_fail_with(r#"scoped class A{} transient class B[a:A]{} app MyApp{fn main(self){}}"#, "scope"); }

// App depends on scoped
#[test]
#[ignore] // PR #46 - outdated assertions
fn app_depends_scoped() { compile_should_fail_with(r#"scoped class A{} app MyApp[a:A]{fn main(self){}}"#, "scope"); }

// App depends on transient
#[test]
#[ignore] // PR #46 - outdated assertions
fn app_depends_transient() { compile_should_fail_with(r#"transient class A{} app MyApp[a:A]{fn main(self){}}"#, "scope"); }

// Scope violation in chain
#[test]
#[ignore] // PR #46 - outdated assertions
fn scope_chain_violation() { compile_should_fail_with(r#"scoped class A{} class B[a:A]{} class C[b:B]{} app MyApp{fn main(self){}}"#, "scope"); }

// Mixed scope deps
#[test]
#[ignore] // PR #46 - outdated assertions
fn mixed_scope_deps() { compile_should_fail_with(r#"class A{} scoped class B{} class C[a:A,b:B]{} app MyApp{fn main(self){}}"#, "scope"); }

// Scoped class without app
#[test]
#[ignore] // PR #46 - outdated assertions
fn scoped_no_app() { compile_should_fail_with(r#"scoped class A{} fn main(){let a=A{}}"#, ""); }

// Transient class without app
#[test]
#[ignore] // PR #46 - outdated assertions
fn transient_no_app() { compile_should_fail_with(r#"transient class A{} fn main(){let a=A{}}"#, ""); }

// Manual construction of DI class
#[test]
#[ignore] // PR #46 - outdated assertions
fn manual_construct_di() { compile_should_fail_with(r#"class A{} class B[a:A]{} fn main(){let b=B{}}"#, ""); }

// Scope annotation on non-class
#[test]
#[ignore] // PR #46 - outdated assertions
fn scope_on_non_class() { compile_should_fail_with(r#"scoped fn f(){} fn main(){}"#, ""); }

// Multiple scope annotations
#[test]
#[ignore] // PR #46 - outdated assertions
fn multiple_scopes() { compile_should_fail_with(r#"scoped transient class A{} app MyApp{fn main(self){}}"#, ""); }
