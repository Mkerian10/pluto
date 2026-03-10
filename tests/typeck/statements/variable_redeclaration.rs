//! Variable redeclaration and shadowing enforcement tests
#[path = "../common.rs"]
mod common;
use common::{compile_and_run, compile_should_fail_with};

// Same scope redeclaration
#[test]
fn redeclare_same_scope() { compile_should_fail_with("fn main(){\n  let x=1\n  let x=2\n}", "already declared"); }
#[test]
fn redeclare_different_types() { compile_should_fail_with("fn main(){\n  let x=1\n  let x=true\n}", "already declared"); }

// Function parameter redeclaration — param and let body are in the same scope
#[test]
fn param_redeclare() { compile_should_fail_with("fn f(x:int){\nlet x=2\n}\nfn main(){}", "already declared"); }
#[test]
fn two_params_same_name() { compile_should_fail_with("fn f(x:int,x:string){}\nfn main(){}", "already declared"); }

// For loop variable shadowing outer var
#[test]
fn for_var_redeclare() { compile_should_fail_with("fn main(){\n  let i=1\n  for i in 0..10{}\n}", "shadows"); }
#[test]
fn nested_for_same_var() { compile_should_fail_with("fn main(){\nfor i in 0..10{\nfor i in 0..5{}\n}\n}", "shadows"); }

// Match binding shadows outer variable
#[test]
fn match_binding_redeclare() { compile_should_fail_with("enum E{\nA{x:int}\n}\nfn main(){\nlet x=1\nlet e = E.A { x: 2 }\nmatch e{\nE.A{x}{}\n}\n}", "shadows"); }

// Closure parameter shadows outer variable
#[test]
fn closure_param_redeclare() { compile_should_fail_with("fn main(){\nlet x=1\nlet f=(x:int)=>{\nx+1\n}\n}", "shadows"); }

// Class field vs method param — fields are accessed via self.x, not as variables.
// A method param named `x` when the class has field `x` is fine since fields require `self.`
#[test]
#[ignore] // Fields are not in variable scope — no collision possible
fn field_vs_method_param() { compile_should_fail_with("class C{\nx:int\nfn foo(self,x:int){}\n}\nfn main(){}", "shadows"); }

// Redeclare in nested scope is shadowing (now rejected)
#[test]
fn shadow_in_nested_scope() { compile_should_fail_with("fn main(){\nlet x=1\nif true{\nlet x=2\n}\n}", "shadows"); }

// Redeclare after nested scope — same scope, so "already declared"
#[test]
fn redeclare_after_scope() { compile_should_fail_with("fn main(){\n  let x=1\n  if true{}\n  let x=2\n}", "already declared"); }

// Function name vs variable
#[test]
fn function_name_vs_var() { compile_should_fail_with("fn x(){}\nfn main(){\nlet x=1\n}", "shadows a function"); }

// Class name vs variable
#[test]
fn class_name_vs_var() { compile_should_fail_with("class C{\nv:int\n}\nfn main(){\nlet C=1\n}", "shadows a class"); }

// Enum name vs variable
#[test]
fn enum_name_vs_var() { compile_should_fail_with("enum E{\nA\n}\nfn main(){\nlet E=1\n}", "shadows an enum"); }

// Match arms have independent scopes — same binding name in different arms is allowed when no outer shadow
#[test]
fn match_arms_same_binding() { compile_should_fail_with("enum E{\nA{x:int}\nB{x:int}\n}\nfn main(){\nlet x=99\nlet e = E.A { x: 1 }\nmatch e{\nE.A{x}{}\nE.B{x}{}\n}\n}", "shadows"); }

// Generic type param vs variable
#[test]
#[ignore] // Type params are not tracked as variables — separate issue
fn type_param_vs_var() { compile_should_fail_with("fn f<T>(x:T){\nlet T=1\n}\nfn main(){}", "shadows"); }

// Trait name vs variable
#[test]
fn trait_name_vs_var() { compile_should_fail_with("trait T{\nfn foo(self)\n}\nfn main(){\nlet T=1\n}", "shadows a trait"); }

// Error name vs variable
#[test]
fn error_name_vs_var() { compile_should_fail_with("error E{}\nfn main(){\nlet E=1\n}", "shadows an error"); }

// App name vs variable — app gets registered as a class, so hits "shadows a class"
#[test]
fn app_name_vs_var() { compile_should_fail_with("app MyApp{\nfn main(self){\nlet MyApp=1\n}\n}", "shadows"); }

// Imported module name vs variable
#[test]
#[ignore] // #160: TypeEnv doesn't track module names yet
fn module_name_vs_var() { compile_should_fail_with("import math\nfn main(){\nlet math=1\n}", "shadows"); }

// ═══════════════════════════════════════════════════════════════════════════════
// EDGE CASES: Allowed post-scope reuse
// ═══════════════════════════════════════════════════════════════════════════════

// Post-scope reuse is allowed — variable is gone after scope exits
#[test]
fn post_scope_reuse_if() {
    compile_and_run("fn main(){\nif true{\nlet x=1\n}\nlet x=2\n}");
}

// Sequential for loops can reuse the same loop variable name
#[test]
fn post_scope_reuse_for() {
    compile_and_run("fn main(){\nfor x in [1,2]{}\nfor x in [3,4]{}\n}");
}

// Same closure param name in independent closures is fine
#[test]
fn independent_closures_same_param() {
    compile_and_run("fn main(){\nlet f=(x:int)=>x+1\nlet g=(x:int)=>x+2\n}");
}

// ═══════════════════════════════════════════════════════════════════════════════
// EDGE CASES: Channel and catch handler shadowing
// ═══════════════════════════════════════════════════════════════════════════════

// Channel sender shadows outer variable
#[test]
fn channel_sender_shadows() {
    compile_should_fail_with("fn main(){\nlet tx=1\nlet (tx, rx) = chan<int>()\n}", "already declared");
}

// Catch handler variable shadows outer variable
#[test]
fn catch_handler_shadows() {
    compile_should_fail_with(
        "error MyErr{}\nfn fail() int{\nraise MyErr{}\n}\nfn main(){\nlet e=1\nlet x = fail() catch e {\nreturn\n}\n}",
        "shadows",
    );
}

// Builtin name shadowing
#[test]
fn shadow_builtin_abs() {
    compile_should_fail_with("fn main(){\nlet abs=1\n}", "shadows a function");
}
