//! Nullable in containers tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Arrays with nullable elements
#[test] fn array_nullable_element_mismatch() { compile_should_fail_with(r#"fn main(){let a:[int?]=[42,\"hi\"]}"#, "type mismatch"); }
#[test] fn array_nullable_vs_non_nullable() { compile_should_fail_with(r#"fn main(){let a:[int]=[42,none]}"#, "type mismatch"); }
#[test] fn array_index_nullable() { compile_should_fail_with(r#"fn main(){let a:[int?]=[42,none] let x:int=a[0]}"#, "type mismatch"); }

// Maps with nullable keys/values
#[test] fn map_nullable_key() { compile_should_fail_with(r#"fn main(){let m=Map<int?,string>{} m[42]=\"hi\"}"#, ""); }
#[test] fn map_nullable_value_access() { compile_should_fail_with(r#"fn main(){let m=Map<string,int?>{} m[\"a\"]=42 let x:int=m[\"a\"]}"#, "type mismatch"); }
#[test] fn map_none_value() { compile_should_fail_with(r#"fn main(){let m=Map<string,int?>{} m[\"a\"]=none let x:int=m[\"a\"]}"#, "type mismatch"); }

// Sets with nullable elements
#[test] fn set_nullable_element() { compile_should_fail_with(r#"fn main(){let s=Set<int?>{} s.insert(42) s.insert(none)}"#, ""); }
#[test] fn set_nullable_contains() { compile_should_fail_with(r#"fn main(){let s=Set<int?>{42,none} let b=s.contains(none)}"#, ""); }

// Nested containers with nullable
#[test] fn array_of_nullable_arrays() { compile_should_fail_with(r#"fn main(){let a:[[int]?]=[[1,2],none]}"#, ""); }
#[test] fn map_of_nullable_maps() { compile_should_fail_with(r#"fn main(){let m:Map<string,Map<string,int>?>=Map<string,Map<string,int>?>{} m[\"a\"]=none}"#, ""); }

// Generic containers with nullable
#[test] fn generic_box_nullable() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b:Box<int?>=Box<int?>{value:none} let x:int=b.value}"#, "type mismatch"); }
#[test] fn generic_unwrap_nullable() { compile_should_fail_with(r#"class Box<T>{value:T fn get(self)T{return self.value}} fn main(){let b=Box<int?>{value:none} let x:int=b.get()}"#, "type mismatch"); }

// Operations on nullable containers
#[test] fn nullable_array_index() { compile_should_fail_with(r#"fn main(){let a:[int]?=[1,2,3] let x=a[0]}"#, "type mismatch"); }
#[test] fn nullable_map_access() { compile_should_fail_with(r#"fn main(){let m:Map<string,int>?=Map<string,int>{} m[\"a\"]=1}"#, "type mismatch"); }

// Container methods with nullable
#[test] fn array_len_on_nullable() { compile_should_fail_with(r#"fn main(){let a:[int]?=[1,2,3] let n=a.len()}"#, "type mismatch"); }
