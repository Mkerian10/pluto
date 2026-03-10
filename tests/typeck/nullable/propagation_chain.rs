//! Nullable propagation chain tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::{compile_and_run, compile_should_fail_with};

// Basic ? propagation errors
#[test]
fn propagate_in_non_nullable_fn() {
    compile_should_fail_with("fn f() int {\nlet x: int? = 42\nreturn x?\n}\nfn main() {}\n", "non-nullable");
}
#[test]
fn propagate_non_nullable_value() {
    compile_should_fail_with("fn f() int? {\nreturn 42?\n}\nfn main() {}\n", "'?' applied to non-nullable type int");
}

// Chained field access — ? in void main() is valid (guard clause pattern)
#[test]
fn chain_field_access_nullable() {
    compile_and_run("class A {\nx: int\n}\nclass B {\na: A?\n}\nfn main() {\nlet b = B { a: none }\nlet x = b.a?.x\n}\n");
}
#[test]
fn triple_chain_field_access() {
    compile_and_run("class A {\nx: int\n}\nclass B {\na: A?\n}\nclass C {\nb: B?\n}\nfn main() {\nlet c = C { b: none }\nlet x = c.b?.a?.x\n}\n");
}

// Method call chains — ? in void main() is valid
#[test]
fn nullable_method_chain() {
    compile_and_run("class C {\nfn foo(self) C? {\nreturn none\n}\n}\nfn main() {\nlet c = C {}\nlet x = c.foo()?.foo()\n}\n");
}
#[test]
fn propagate_method_result() {
    compile_and_run("class C {\nfn foo(self) int? {\nreturn none\n}\n}\nfn main() {\nlet c = C {}\nlet x = c.foo()?\n}\n");
}

// Propagation in expressions — ? in functions returning T? is valid
#[test]
fn propagate_in_binop() {
    compile_and_run("fn f() int? {\nreturn none\n}\nfn g() int? {\nreturn f()? + 1\n}\nfn main() {}\n");
}
#[test]
fn propagate_in_array() {
    compile_and_run("fn f() int? {\nreturn none\n}\nfn g() [int]? {\nreturn [f()?, 2, 3]\n}\nfn main() {}\n");
}
#[test]
fn propagate_in_struct() {
    compile_and_run("class C {\nx: int\n}\nfn f() int? {\nreturn none\n}\nfn g() C? {\nreturn C { x: f()? }\n}\nfn main() {}\n");
}

// Mixed error and nullable propagation
#[test]
fn nullable_and_error_propagate() {
    compile_should_fail_with("error E {}\nfn f() int! {\nraise E {}\n}\nfn g() int? {\nreturn f()!?\n}\nfn main() {}\n", "");
}
#[test]
fn error_and_nullable_propagate() {
    compile_should_fail_with("fn f() int? {\nreturn none\n}\nfn g() int! {\nreturn f()?!\n}\nfn main() {}\n", "");
}

// Propagate wrong type
#[test]
fn propagate_returns_wrong_type() {
    compile_should_fail_with("fn f() int? {\nreturn none\n}\nfn g() string? {\nreturn f()?\n}\nfn main() {}\n", "type mismatch");
}

// Deep propagation chains — all functions return int?, so ? is valid
#[test]
fn five_level_propagation() {
    compile_and_run("fn f1() int? {\nreturn none\n}\nfn f2() int? {\nreturn f1()?\n}\nfn f3() int? {\nreturn f2()?\n}\nfn f4() int? {\nreturn f3()?\n}\nfn f5() int? {\nreturn f4()?\n}\nfn main() {}\n");
}

// Propagate in control flow — ? in functions returning T? is valid
#[test]
fn propagate_in_if_early_return() {
    compile_and_run("fn f(x: int?) int? {\nif true {\nreturn x?\n}\nreturn 0\n}\nfn main() {}\n");
}
#[test]
fn propagate_in_match() {
    compile_should_fail_with("enum E {\nA\nB\n}\nfn f(e: E, x: int?) int? {\nmatch e {\nE.A => {\nreturn x?\n}\nE.B => {\nreturn 0\n}\n}\n}\nfn main() {}\n", "");
}
