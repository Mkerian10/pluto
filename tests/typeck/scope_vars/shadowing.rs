//! Variable shadowing tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Local shadows parameter
#[test]
fn local_shadows_param() { compile_should_fail_with("fn f(x:int){\nlet x=2\n}\nfn main(){}", "already declared"); }

// Nested scope shadows
#[test]
fn nested_shadows() { compile_should_fail_with("fn main(){\nlet x=1\nif true{\nlet x=2\n}\n}", "shadows"); }

// Loop variable shadows outer
#[test]
fn loop_shadows_outer() { compile_should_fail_with("fn main(){\nlet i=1\nfor i in 0..10{}\n}", "shadows"); }

// Match binding shadows
#[test]
fn match_shadows() { compile_should_fail_with("enum E{\nA{x:int}\n}\nfn main(){\nlet x=1\nlet e = E.A { x: 2 }\nmatch e{\nE.A{x}{}\n}\n}", "shadows"); }

// Closure parameter shadows capture
#[test]
fn closure_param_shadows_capture() { compile_should_fail_with("fn main(){\nlet x=1\nlet f=(x:int)=>x+1\n}", "shadows"); }

// Function shadows global
#[test]
fn function_shadows_global() { compile_should_fail_with("fn x() int{\nreturn 1\n}\nfn main(){\nlet x=2\n}", "shadows a function"); }

// Class shadows function (declaration-vs-declaration, #174)
#[test]
fn class_shadows_function() { compile_should_fail_with("fn C(){}\nclass C{\nv:int\n}\nfn main(){}", ""); }

// Type param shadows class (declaration-vs-declaration, #174)
#[test]
fn type_param_shadows_class() { compile_should_fail_with("class T{\nv:int\n}\nfn f<T>(x:T){}\nfn main(){}", ""); }

// Multiple shadow levels
#[test]
fn multiple_shadow_levels() { compile_should_fail_with("fn main(){\nlet x=1\nif true{\nlet x=2\nif true{\nlet x=3\n}\n}\n}", "shadows"); }

// Shadow after scope ends — this is ALLOWED (post-scope reuse)
#[test]
#[ignore] // Post-scope reuse is allowed, this should NOT fail
fn shadow_after_scope() { compile_should_fail_with("fn main(){\nif true{\nlet x=1\n}\nlet x=2\n}", ""); }

// Shadow in different branches
#[test]
fn shadow_diff_branches() { compile_should_fail_with("fn main(){\nlet x=1\nif true{\nlet x=2\n}else{\nlet x=3\n}\n}", "shadows"); }

// Shadow in match arms
#[test]
fn shadow_match_arms() { compile_should_fail_with("enum E{\nA\nB\n}\nfn main(){\nlet x=1\nmatch E.A{\nE.A{\nlet x=2\n}\nE.B{\nlet x=3\n}\n}\n}", "shadows"); }

// Field name shadows parameter — fields require self., not in variable scope
#[test]
#[ignore] // Fields are not in variable scope — no collision possible
fn field_shadows_param() { compile_should_fail_with("class C{\nx:int\nfn foo(self,x:int){}\n}\nfn main(){}", "shadows"); }

// Method name shadows field (declaration-vs-declaration, #174)
#[test]
fn method_shadows_field() { compile_should_fail_with("class C{\nfoo:int\n}\nfn foo(self){}\nfn main(){}", ""); }

// Import shadows local
#[test]
#[ignore] // Module names not tracked as variables yet
fn import_shadows_local() { compile_should_fail_with("import math\nfn main(){\nlet math=1\n}", ""); }

// Enum variant shadows variable
#[test]
#[ignore] // Enum variant names aren't tracked as variables
fn variant_shadows_var() { compile_should_fail_with("enum E{\nA\n}\nfn main(){\nlet A=1\n}", ""); }

// Error type shadows class (declaration-vs-declaration, #174)
#[test]
fn error_shadows_class() { compile_should_fail_with("class E{\nv:int\n}\nerror E{}\nfn main(){}", ""); }

// Trait shadows enum (declaration-vs-declaration, #174)
#[test]
fn trait_shadows_enum() { compile_should_fail_with("enum T{\nA\n}\ntrait T{\nfn foo(self)\n}\nfn main(){}", ""); }

// Generic shadow in nested function (declaration-vs-declaration, #174)
#[test]
fn generic_shadow_nested() { compile_should_fail_with("fn f<T>(x:T){\nfn g<T>(y:T){}\n}\nfn main(){}", ""); }

// Shadow builtin
#[test]
fn shadow_builtin() { compile_should_fail_with("fn main(){\nlet print=1\n}", "shadows a function"); }
