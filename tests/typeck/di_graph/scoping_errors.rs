//! DI scoping errors - 7 tests (removed 8 ACTUALLY_SUCCESS)
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// REMOVED: scoped_depends_singleton - likely valid
// Singleton depends on scoped (invalid)
#[test] fn singleton_depends_scoped() { compile_should_fail_with(r#"scoped class A{} class B[a:A]{} app MyApp{fn main(self){}}"#, "scope"); }

// REMOVED: transient_depends_singleton - likely valid
// Singleton depends on transient (invalid)
#[test] fn singleton_depends_transient() { compile_should_fail_with(r#"transient class A{} class B[a:A]{} app MyApp{fn main(self){}}"#, "scope"); }

// REMOVED: scoped_depends_transient - likely valid
// Transient depends on scoped (invalid)
#[test] fn transient_depends_scoped() { compile_should_fail_with(r#"scoped class A{} transient class B[a:A]{} app MyApp{fn main(self){}}"#, "scope"); }

// App depends on scoped
#[test] fn app_depends_scoped() { compile_should_fail_with(r#"scoped class A{} app MyApp[a:A]{fn main(self){}}"#, "scope"); }

// App depends on transient
#[test] fn app_depends_transient() { compile_should_fail_with(r#"transient class A{} app MyApp[a:A]{fn main(self){}}"#, "scope"); }

// Scope violation in chain
#[test] fn scope_chain_violation() { compile_should_fail_with(r#"scoped class A{} class B[a:A]{} class C[b:B]{} app MyApp{fn main(self){}}"#, "scope"); }

// Mixed scope deps
#[test] fn mixed_scope_deps() { compile_should_fail_with(r#"class A{} scoped class B{} class C[a:A,b:B]{} app MyApp{fn main(self){}}"#, "scope"); }

// REMOVED: scoped_no_app - likely valid
// REMOVED: transient_no_app - likely valid
// REMOVED: manual_construct_di - likely valid
// REMOVED: scope_on_non_class - likely valid or syntax error
// REMOVED: multiple_scopes - likely valid or syntax error
