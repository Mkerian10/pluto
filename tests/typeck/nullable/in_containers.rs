//! Nullable in containers tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Arrays with nullable elements
#[test]
#[ignore] // PR #46 - outdated assertions
fn array_nullable_element_mismatch() { compile_should_fail_with(r#"fn main(){let a:[int?]=[42,\"hi\"]}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn array_nullable_vs_non_nullable() { compile_should_fail_with(r#"fn main(){let a:[int]=[42,none]}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn array_index_nullable() { compile_should_fail_with(r#"fn main(){let a:[int?]=[42,none] let x:int=a[0]}"#, "type mismatch"); }

// Maps with nullable keys/values
#[test]
#[ignore] // PR #46 - outdated assertions
fn map_nullable_key() { compile_should_fail_with(r#"fn main(){let m=Map<int?,string>{} m[42]=\"hi\"}"#, ""); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn map_nullable_value_access() { compile_should_fail_with(r#"fn main(){let m=Map<string,int?>{} m[\"a\"]=42 let x:int=m[\"a\"]}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn map_none_value() { compile_should_fail_with(r#"fn main(){let m=Map<string,int?>{} m[\"a\"]=none let x:int=m[\"a\"]}"#, "type mismatch"); }

// Sets with nullable elements
#[test]
#[ignore] // PR #46 - outdated assertions
fn set_nullable_element() { compile_should_fail_with(r#"fn main(){let s=Set<int?>{} s.insert(42) s.insert(none)}"#, ""); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn set_nullable_contains() { compile_should_fail_with(r#"fn main(){let s=Set<int?>{42,none} let b=s.contains(none)}"#, ""); }

// Nested containers with nullable
#[test]
#[ignore] // PR #46 - outdated assertions
fn array_of_nullable_arrays() { compile_should_fail_with(r#"fn main(){let a:[[int]?]=[[1,2],none]}"#, ""); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn map_of_nullable_maps() { compile_should_fail_with(r#"fn main(){let m:Map<string,Map<string,int>?>=Map<string,Map<string,int>?>{} m[\"a\"]=none}"#, ""); }

// Generic containers with nullable
#[test]
#[ignore] // PR #46 - outdated assertions
fn generic_box_nullable() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b:Box<int?>=Box<int?>{value:none} let x:int=b.value}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn generic_unwrap_nullable() { compile_should_fail_with(r#"class Box<T>{value:T fn get(self)T{return self.value}} fn main(){let b=Box<int?>{value:none} let x:int=b.get()}"#, "type mismatch"); }

// Operations on nullable containers
#[test]
#[ignore] // PR #46 - outdated assertions
fn nullable_array_index() { compile_should_fail_with(r#"fn main(){let a:[int]?=[1,2,3] let x=a[0]}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn nullable_map_access() { compile_should_fail_with(r#"fn main(){let m:Map<string,int>?=Map<string,int>{} m[\"a\"]=1}"#, "type mismatch"); }

// Container methods with nullable
#[test]
#[ignore] // PR #46 - outdated assertions
fn array_len_on_nullable() { compile_should_fail_with(r#"fn main(){let a:[int]?=[1,2,3] let n=a.len()}"#, "type mismatch"); }
