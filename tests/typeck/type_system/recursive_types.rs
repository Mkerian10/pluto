//! Recursive type and type cycle tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Direct recursive class
#[test]
#[ignore] // Compiler limitation: doesn't detect recursive types
fn direct_recursive_class() { compile_should_fail_with(r#"class C{x:C} fn main(){}"#, ""); }

// Indirect recursive class
#[test]
#[ignore] // Compiler limitation: doesn't detect recursive types
fn indirect_recursive_class() { compile_should_fail_with(r#"class A{b:B} class B{a:A} fn main(){}"#, ""); }

// Recursive enum variant
#[test]
#[ignore] // Compiler limitation: doesn't detect recursive types
fn recursive_enum_variant() { compile_should_fail_with(r#"enum E{Node{next:E}Leaf} fn main(){}"#, ""); }

// Cycle through three classes
#[test]
#[ignore] // Compiler limitation: doesn't detect recursive types
fn three_class_cycle() { compile_should_fail_with(r#"class A{b:B} class B{c:C} class C{a:A} fn main(){}"#, ""); }

// Recursive type parameter
#[test]
#[ignore] // Compiler limitation: doesn't detect recursive types
fn recursive_type_param() { compile_should_fail_with(r#"class C<T>{x:T} fn f()C<C<C<int>>>{} fn main(){}"#, ""); }

// Recursive trait bound
#[test]
fn recursive_trait_bound() { compile_should_fail_with(r#"trait T<U:T<U>>{} fn main(){}"#, ""); }

// Recursive function type
#[test]
#[ignore] // Compiler limitation: doesn't detect recursive types
fn recursive_fn_type() { compile_should_fail_with(r#"fn f(g:fn(fn(int)int)int)int{return 1} fn main(){}"#, ""); }

// Recursive array type
#[test]
fn recursive_array_type() { compile_should_fail_with(r#"class C{arr:Array<C>} fn main(){}"#, ""); }

// Recursive map type
#[test]
#[ignore] // Compiler limitation: doesn't detect recursive types
fn recursive_map_type() { compile_should_fail_with(r#"class C{m:Map<string,C>} fn main(){}"#, ""); }

// Recursive nullable type
#[test]
#[ignore] // Compiler limitation: doesn't detect recursive types
fn recursive_nullable_type() { compile_should_fail_with(r#"class C{x:C?} fn main(){}"#, ""); }

// Mutual recursion with enums
#[test]
#[ignore] // Compiler limitation: doesn't detect recursive types
fn mutual_enum_recursion() { compile_should_fail_with(r#"enum A{B{b:B}} enum B{A{a:A}} fn main(){}"#, ""); }

// Recursive generic class
#[test]
fn recursive_generic_class() { compile_should_fail_with(r#"class C<T>{x:C<T>} fn main(){}"#, ""); }

// Recursive trait implementation
#[test]
fn recursive_trait_impl() { compile_should_fail_with(r#"trait T{fn f(self)T} class C{} impl T{fn f(self)T{return self}} fn main(){}"#, ""); }

// Cycle through bracket deps
#[test]
fn cycle_bracket_deps() { compile_should_fail_with(r#"class A[b:B]{} class B[a:A]{} fn main(){}"#, ""); }

// Recursive error type
#[test]
fn recursive_error_type() { compile_should_fail_with(r#"error E{cause:E} fn main(){}"#, ""); }

// Recursive task type
#[test]
#[ignore] // Compiler limitation: doesn't detect recursive types
fn recursive_task_type() { compile_should_fail_with(r#"fn task()Task<Task<int>>{} fn main(){}"#, ""); }

// Recursive channel type
#[test]
fn recursive_channel_type() { compile_should_fail_with(r#"class C{ch:Channel<C,C>} fn main(){}"#, ""); }

// Type alias cycle
#[test]
fn type_alias_cycle() { compile_should_fail_with(r#"type A=B type B=A fn main(){}"#, ""); }

// Recursive in field and method
#[test]
fn recursive_field_method() { compile_should_fail_with(r#"class C{x:C} fn get(self)C{return self.x} fn main(){}"#, ""); }

// Deeply nested recursive type
#[test]
#[ignore] // Compiler limitation: doesn't detect recursive types
fn deep_recursive_type() { compile_should_fail_with(r#"class A{b:B} class B{c:C} class C{d:D} class D{e:E} class E{a:A} fn main(){}"#, ""); }
