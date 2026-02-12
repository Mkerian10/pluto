//! Collection type error tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Array element type mismatch
#[test] fn array_elem_type_mismatch() { compile_should_fail_with(r#"fn main(){let arr:Array<int>=[1,2,"hi"]}"#, ""); }

// Map key type mismatch
#[test] fn map_key_type_mismatch() { compile_should_fail_with(r#"fn main(){let m:Map<int,string>={1:"a","b":"c"}}"#, ""); }

// Map value type mismatch
#[test] fn map_value_type_mismatch() { compile_should_fail_with(r#"fn main(){let m:Map<string,int>={"a":1,"b":"c"}}"#, ""); }

// Set element type mismatch
#[test] fn set_elem_type_mismatch() { compile_should_fail_with(r#"fn main(){let s:Set<int>={1,2,"hi"}}"#, ""); }

// Array generic wrong type arg
#[test] fn array_generic_wrong() { compile_should_fail_with(r#"fn main(){let arr:Array<string>=[1,2,3]}"#, ""); }

// Map with non-hashable key
#[test] fn map_non_hashable_key() { compile_should_fail_with(r#"class C{x:int} fn main(){let m=Map<C,int>{}}"#, ""); }

// Set with non-hashable element
#[test] fn set_non_hashable_elem() { compile_should_fail_with(r#"class C{x:int} fn main(){let s=Set<C>{}}"#, ""); }

// Nested array type mismatch
#[test] fn nested_array_mismatch() { compile_should_fail_with(r#"fn main(){let arr:Array<Array<int>>=[[1,2],[3,"hi"]]}"#, ""); }

// Map with array values wrong type
#[test] fn map_array_value_mismatch() { compile_should_fail_with(r#"fn main(){let m:Map<string,Array<int>>={"a":[1,2],"b":["hi"]}}"#, ""); }

// Set of maps wrong type
#[test] fn set_map_mismatch() { compile_should_fail_with(r#"fn main(){let s:Set<Map<string,int>>={{"a":1},{"b":"hi"}}}"#, ""); }

// Array method return type mismatch
#[test] fn array_method_return_mismatch() { compile_should_fail_with(r#"fn main(){let arr=[1,2,3] let x:string=arr.len()}"#, ""); }

// Map insert wrong value type
#[test] fn map_insert_wrong_value() { compile_should_fail_with(r#"fn main(){let m=Map<string,int>{} m.insert("a","b")}"#, ""); }

// Set insert wrong type
#[test] fn set_insert_wrong_type() { compile_should_fail_with(r#"fn main(){let s=Set<int>{} s.insert("hi")}"#, ""); }

// Array concatenation type mismatch
#[test] fn array_concat_mismatch() { compile_should_fail_with(r#"fn main(){let a1=[1,2] let a2=["a","b"] let a3=a1+a2}"#, ""); }

// Collection in generic wrong type
#[test] fn collection_generic_mismatch() { compile_should_fail_with(r#"class Box<T>{val:T} fn main(){let b:Box<Array<int>>=Box{val:["hi"]}}"#, ""); }
