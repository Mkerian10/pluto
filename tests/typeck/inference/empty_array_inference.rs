//! Empty array type inference tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

#[test]
fn empty_array_no_annotation() {
    compile_should_fail_with(r#"fn main() { let x = [] }"#, "cannot infer");
}

#[test]
#[ignore]
fn empty_array_wrong_annotation() {
    compile_should_fail_with(r#"fn main() { let x: [string] = [] let y = x[0] + 5 }"#, "type mismatch");
}

#[test]
fn empty_array_in_binop() {
    compile_should_fail_with(r#"fn main() { let x = [] + [1,2,3] }"#, "cannot infer");
}

#[test]
fn empty_array_passed_to_generic() {
    compile_should_fail_with(r#"fn id<T>(x:[T])[T]{return x} fn main(){let x=id([])}"#, "cannot infer");
}

#[test]
fn nested_empty_array() {
    compile_should_fail_with(r#"fn main() { let x = [[]] }"#, "cannot infer");
}

#[test]
fn empty_array_in_map_value() {
    compile_should_fail_with(r#"fn main(){ let m=Map<string,[int]>{"a":[]} }"#, "");
}

#[test]
fn empty_array_index_access() {
    compile_should_fail_with(r#"fn main() { let x = [][0] }"#, "cannot infer");
}

#[test]
fn empty_array_method_call() {
    compile_should_fail_with(r#"fn main() { let x = [].len() }"#, "cannot infer");
}

#[test]
fn empty_array_in_field() {
    compile_should_fail_with(r#"class C{f:[int]} fn main(){let c=C{f:[]}}"#, "");
}

#[test]
fn empty_array_concat() {
    compile_should_fail_with(r#"fn main() { let x = [] + [] }"#, "cannot infer");
}

#[test]
fn return_empty_array_no_type() {
    compile_should_fail_with(r#"fn f() { return [] } fn main() {}"#, "cannot infer");
}

#[test]
fn empty_array_in_closure() {
    compile_should_fail_with(r#"fn main() { let f = () => [] }"#, "cannot infer");
}

#[test]
#[ignore]
fn assign_empty_to_typed_var() {
    compile_should_fail_with(r#"fn main() { let x: [string] = [] x[0] = 5 }"#, "index assignment");
}

#[test]
fn empty_array_spread_attempt() {
    compile_should_fail_with(r#"fn main() { let x = [1, ...[]]} }"#, "");
}

#[test]
fn compare_empty_arrays() {
    compile_should_fail_with(r#"fn main() { let x = [] == [] }"#, "cannot infer");
}
