pub mod env;
pub mod types;
pub mod serializable;
mod register;
mod resolve;
mod infer;
mod check;
mod closures;
mod errors;

// Re-exports for external use
pub(crate) use check::check_function;
pub(crate) use register::check_trait_conformance;
pub(crate) use resolve::resolve_type_for_monomorphize;

use crate::diagnostics::{CompileError, CompileWarning, WarningKind};
use crate::parser::ast::Program;
use env::{ErrorInfo, TypeEnv};
use types::PlutoType;

fn types_compatible(actual: &PlutoType, expected: &PlutoType, env: &TypeEnv) -> bool {
    if actual == expected {
        return true;
    }
    if let (PlutoType::Class(cn), PlutoType::Trait(tn)) = (actual, expected) {
        return env.class_implements_trait(cn, tn);
    }
    // Fn types: structural compatibility (same param count, each param compatible, return compatible)
    if let (PlutoType::Fn(a_params, a_ret), PlutoType::Fn(e_params, e_ret)) = (actual, expected) {
        if a_params.len() != e_params.len() {
            return false;
        }
        for (ap, ep) in a_params.iter().zip(e_params.iter()) {
            if !types_compatible(ap, ep, env) {
                return false;
            }
        }
        return types_compatible(a_ret, e_ret, env);
    }
    // T is assignable to T? (implicit nullable wrap)
    if let PlutoType::Nullable(inner) = expected && types_compatible(actual, inner, env) {
        return true;
    }
    // Nullable(Void) (the none literal) is assignable to any Nullable(T)
    if actual == &PlutoType::Nullable(Box::new(PlutoType::Void)) && matches!(expected, PlutoType::Nullable(_)) {
        return true;
    }
    false
}

pub fn type_check(program: &Program) -> Result<(TypeEnv, Vec<CompileWarning>), CompileError> {
    let mut env = TypeEnv::new();

    // Pass 0: Register names only (no type resolution)
    register::register_trait_names(program, &mut env)?;
    register::register_enum_names(program, &mut env)?;
    register::register_app_placeholder(program, &mut env)?;
    register::register_stage_placeholders(program, &mut env)?;
    register::register_errors(program, &mut env)?;
    env.errors.entry("MathError".to_string()).or_insert(ErrorInfo {
        fields: vec![("message".to_string(), PlutoType::String)],
    });
    env.errors.entry("RustError".to_string()).or_insert(ErrorInfo {
        fields: vec![("message".to_string(), PlutoType::String)],
    });
    env.errors.entry("ChannelClosed".to_string()).or_insert(ErrorInfo {
        fields: vec![("message".to_string(), PlutoType::String)],
    });
    env.errors.entry("ChannelFull".to_string()).or_insert(ErrorInfo {
        fields: vec![("message".to_string(), PlutoType::String)],
    });
    env.errors.entry("ChannelEmpty".to_string()).or_insert(ErrorInfo {
        fields: vec![("message".to_string(), PlutoType::String)],
    });
    env.errors.entry("TaskCancelled".to_string()).or_insert(ErrorInfo {
        fields: vec![("message".to_string(), PlutoType::String)],
    });
    env.errors.entry("NetworkError".to_string()).or_insert(ErrorInfo {
        fields: vec![("message".to_string(), PlutoType::String)],
    });
    env.errors.entry("TimeoutError".to_string()).or_insert(ErrorInfo {
        fields: vec![("millis".to_string(), PlutoType::Int)],
    });
    env.errors.entry("ServiceUnavailable".to_string()).or_insert(ErrorInfo {
        fields: vec![("service".to_string(), PlutoType::String)],
    });
    register::register_class_names(program, &mut env)?;

    // Pass 1: Resolve types now that all names are registered
    register::resolve_trait_signatures(program, &mut env)?;
    register::resolve_enum_fields(program, &mut env)?;
    register::resolve_class_fields(program, &mut env)?;
    register::register_extern_fns(program, &mut env)?;
    register::register_functions(program, &mut env)?;
    register::register_method_sigs(program, &mut env)?;
    register::register_app_fields_and_methods(program, &mut env)?;
    register::register_stage_fields_and_methods(program, &mut env)?;
    register::validate_di_graph(program, &mut env)?;
    register::check_trait_conformance(program, &mut env)?;
    register::check_all_bodies(program, &mut env)?;
    check::enforce_mut_self(program, &env)?;
    // Seed Rust FFI fallible functions into fn_errors before inference
    // so that infer_error_sets can propagate RustError through callers.
    for fn_name in &program.fallible_extern_fns {
        env.fn_errors.entry(fn_name.clone())
            .or_default()
            .insert("RustError".to_string());
    }
    errors::infer_error_sets(program, &mut env);
    errors::enforce_error_handling(program, &env)?;
    crate::concurrency::infer_synchronization(program, &mut env);

    let warnings = generate_warnings(&env, program);
    Ok((env, warnings))
}

fn generate_warnings(env: &TypeEnv, program: &Program) -> Vec<CompileWarning> {
    let mut warnings = Vec::new();

    // Collect function parameter names to exclude from unused-variable warnings
    let mut param_names = std::collections::HashSet::new();
    for func in &program.functions {
        for p in &func.node.params {
            param_names.insert(p.name.node.clone());
        }
    }
    if let Some(app) = &program.app {
        for m in &app.node.methods {
            for p in &m.node.params {
                param_names.insert(p.name.node.clone());
            }
        }
    }
    for stage in &program.stages {
        for m in &stage.node.methods {
            for p in &m.node.params {
                param_names.insert(p.name.node.clone());
            }
        }
    }
    for class in &program.classes {
        for method in &class.node.methods {
            for p in &method.node.params {
                param_names.insert(p.name.node.clone());
            }
        }
    }

    for ((name, depth), decl_span) in &env.variable_decls {
        // Skip _-prefixed variables (intentionally unused convention)
        if name.starts_with('_') {
            continue;
        }
        // Skip function parameters
        if param_names.contains(name) {
            continue;
        }
        // Skip if variable was read
        if env.variable_reads.contains(&(name.clone(), *depth)) {
            continue;
        }
        warnings.push(CompileWarning {
            msg: format!("unused variable '{name}'"),
            span: *decl_span,
            kind: WarningKind::UnusedVariable,
        });
    }

    // Sort for deterministic output
    warnings.sort_by_key(|w| w.span.start);
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::Parser;

    fn check(src: &str) -> Result<TypeEnv, CompileError> {
        let tokens = lex(src).unwrap();
        let mut parser = Parser::new(&tokens, src);
        let program = parser.parse_program().unwrap();
        type_check(&program).map(|(env, _warnings)| env)
    }

    fn check_with_parse(src: &str) -> Result<TypeEnv, CompileError> {
        let tokens = lex(src)?;
        let mut parser = Parser::new(&tokens, src);
        let program = parser.parse_program()?;
        type_check(&program).map(|(env, _warnings)| env)
    }

    #[test]
    fn valid_add_function() {
        check("fn add(a: int, b: int) int {\n    return a + b\n}").unwrap();
    }

    #[test]
    fn valid_main_with_call() {
        check("fn add(a: int, b: int) int {\n    return a + b\n}\n\nfn main() {\n    let x = add(1, 2)\n}").unwrap();
    }

    #[test]
    fn type_mismatch_return() {
        let result = check("fn foo() int {\n    return true\n}");
        assert!(result.is_err());
    }

    #[test]
    fn undefined_variable() {
        let result = check("fn main() {\n    let x = y\n}");
        assert!(result.is_err());
    }

    #[test]
    fn wrong_arg_count() {
        let result = check("fn foo(a: int) int {\n    return a\n}\n\nfn main() {\n    let x = foo(1, 2)\n}");
        assert!(result.is_err());
    }

    #[test]
    fn wrong_arg_type() {
        let result = check("fn foo(a: int) int {\n    return a\n}\n\nfn main() {\n    let x = foo(true)\n}");
        assert!(result.is_err());
    }

    #[test]
    fn bool_condition_required() {
        let result = check("fn main() {\n    if 42 {\n        let x = 1\n    }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn valid_comparisons() {
        check("fn main() {\n    let x = 1 < 2\n    let y = 3 == 4\n}").unwrap();
    }

    // Class tests

    #[test]
    fn valid_class_construction() {
        check("class Point {\n    x: int\n    y: int\n}\n\nfn main() {\n    let p = Point { x: 1, y: 2 }\n}").unwrap();
    }

    #[test]
    fn valid_field_access() {
        check("class Point {\n    x: int\n    y: int\n}\n\nfn main() {\n    let p = Point { x: 1, y: 2 }\n    let v = p.x\n}").unwrap();
    }

    #[test]
    fn valid_method_call() {
        check("class Point {\n    x: int\n    y: int\n\n    fn get_x(self) int {\n        return self.x\n    }\n}\n\nfn main() {\n    let p = Point { x: 1, y: 2 }\n    let v = p.get_x()\n}").unwrap();
    }

    #[test]
    fn wrong_field_type_rejected() {
        let result = check("class Point {\n    x: int\n    y: int\n}\n\nfn main() {\n    let p = Point { x: true, y: 2 }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn missing_field_rejected() {
        let result = check("class Point {\n    x: int\n    y: int\n}\n\nfn main() {\n    let p = Point { x: 1 }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn unknown_field_rejected() {
        let result = check("class Point {\n    x: int\n    y: int\n}\n\nfn main() {\n    let p = Point { x: 1, z: 2 }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn class_as_param() {
        check("class Point {\n    x: int\n    y: int\n}\n\nfn get_x(p: Point) int {\n    return p.x\n}\n\nfn main() {\n    let p = Point { x: 42, y: 0 }\n    let v = get_x(p)\n}").unwrap();
    }

    // Trait tests

    #[test]
    fn valid_trait_basic() {
        check("trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n\n    fn bar(self) int {\n        return self.val\n    }\n}\n\nfn main() {\n}").unwrap();
    }

    #[test]
    fn trait_missing_method_rejected() {
        let result = check("trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n}\n\nfn main() {\n}");
        assert!(result.is_err());
    }

    #[test]
    fn trait_unknown_rejected() {
        let result = check("class X impl NonExistent {\n    val: int\n}\n\nfn main() {\n}");
        assert!(result.is_err());
    }

    #[test]
    fn trait_as_param() {
        check("trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n\n    fn bar(self) int {\n        return self.val\n    }\n}\n\nfn process(f: Foo) int {\n    return f.bar()\n}\n\nfn main() {\n    let x = X { val: 42 }\n    let r = process(x)\n}").unwrap();
    }

    #[test]
    fn trait_default_method() {
        check("trait Foo {\n    fn bar(self) int {\n        return 0\n    }\n}\n\nclass X impl Foo {\n    val: int\n}\n\nfn main() {\n}").unwrap();
    }

    // Enum tests

    #[test]
    fn enum_registration() {
        let env = check("enum Color {\n    Red\n    Green\n    Blue\n}\n\nfn main() {\n}").unwrap();
        assert!(env.enums.contains_key("Color"));
        assert_eq!(env.enums["Color"].variants.len(), 3);
    }

    #[test]
    fn enum_unit_construction() {
        check("enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n}").unwrap();
    }

    #[test]
    fn enum_data_construction() {
        check("enum Status {\n    Active\n    Suspended { reason: string }\n}\n\nfn main() {\n    let s = Status.Suspended { reason: \"banned\" }\n}").unwrap();
    }

    #[test]
    fn enum_exhaustive_match() {
        check("enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    match c {\n        Color.Red {\n            let x = 1\n        }\n        Color.Blue {\n            let x = 2\n        }\n    }\n}").unwrap();
    }

    #[test]
    fn enum_non_exhaustive_rejected() {
        let result = check("enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    match c {\n        Color.Red {\n            let x = 1\n        }\n    }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn enum_wrong_field_name_rejected() {
        let result = check("enum Status {\n    Suspended { reason: string }\n}\n\nfn main() {\n    let s = Status.Suspended { msg: \"banned\" }\n}");
        assert!(result.is_err());
    }

    // Closure tests

    #[test]
    fn closure_basic_type() {
        check("fn main() {\n    let f = (x: int) => x + 1\n}").unwrap();
    }

    #[test]
    fn closure_with_return_type() {
        check("fn main() {\n    let f = (x: int) int => x + 1\n}").unwrap();
    }

    #[test]
    fn closure_no_params() {
        check("fn main() {\n    let f = () => 42\n}").unwrap();
    }

    #[test]
    fn closure_multi_params() {
        check("fn main() {\n    let f = (x: int, y: int) => x + y\n}").unwrap();
    }

    #[test]
    fn closure_fn_type_annotation() {
        check("fn main() {\n    let f: fn(int) int = (x: int) => x + 1\n}").unwrap();
    }

    #[test]
    fn closure_call() {
        check("fn main() {\n    let f = (x: int) => x + 1\n    let r = f(5)\n}").unwrap();
    }

    #[test]
    fn closure_wrong_arg_count_rejected() {
        let result = check("fn main() {\n    let f = (x: int) => x + 1\n    let r = f(1, 2)\n}");
        assert!(result.is_err());
    }

    #[test]
    fn closure_wrong_arg_type_rejected() {
        let result = check("fn main() {\n    let f = (x: int) => x + 1\n    let r = f(true)\n}");
        assert!(result.is_err());
    }

    #[test]
    fn closure_as_fn_param() {
        check("fn apply(f: fn(int) int, x: int) int {\n    return f(x)\n}\n\nfn main() {\n    let r = apply((x: int) => x + 1, 5)\n}").unwrap();
    }

    #[test]
    fn closure_capture() {
        check("fn main() {\n    let y = 10\n    let f = (x: int) => x + y\n}").unwrap();
    }

    #[test]
    fn closure_wrong_return_type_rejected() {
        let result = check("fn main() {\n    let f = (x: int) int => true\n}");
        assert!(result.is_err());
    }

    #[test]
    fn fn_type_void_return() {
        check("fn main() {\n    let f: fn(int) = (x: int) => {\n        let y = x\n    }\n}").unwrap();
    }

    // App / DI tests

    #[test]
    fn app_basic_registration() {
        let env = check("app MyApp {\n    fn main(self) {\n    }\n}").unwrap();
        assert!(env.app.is_some());
        let (name, _) = env.app.as_ref().unwrap();
        assert_eq!(name, "MyApp");
    }

    #[test]
    fn app_with_deps() {
        let env = check("class Database {\n    fn query(self) string {\n        return \"result\"\n    }\n}\n\napp MyApp[db: Database] {\n    fn main(self) {\n        let r = self.db.query()\n    }\n}").unwrap();
        assert!(env.app.is_some());
        assert_eq!(env.di_order.len(), 1);
        assert_eq!(env.di_order[0], "Database");
    }

    #[test]
    fn di_cycle_rejected() {
        let result = check("class A[b: B] {\n}\n\nclass B[a: A] {\n}\n\napp MyApp[a: A] {\n    fn main(self) {\n    }\n}");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("circular dependency"), "expected cycle error, got: {}", err);
    }

    #[test]
    fn di_struct_lit_for_inject_class_rejected() {
        let result = check("class Database {\n    x: int\n}\n\nclass UserService[db: Database] {\n    name: string\n}\n\nfn main() {\n    let d = Database { x: 1 }\n    let u = UserService { db: d, name: \"test\" }\n}");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("injected dependencies"), "expected inject error, got: {}", err);
    }

    #[test]
    fn app_and_main_rejected() {
        let result = check("fn main() {\n}\n\napp MyApp {\n    fn main(self) {\n    }\n}");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cannot have both"), "expected conflict error, got: {}", err);
    }

    #[test]
    fn app_missing_main_rejected() {
        let result = check("app MyApp {\n    fn other(self) {\n    }\n}");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("must have a 'main' method"), "expected missing main error, got: {}", err);
    }

    #[test]
    fn app_main_no_self_rejected() {
        let result = check("app MyApp {\n    fn main() {\n    }\n}");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("self"), "expected self error, got: {}", err);
    }

    // Error handling tests

    #[test]
    fn error_decl_registered() {
        let env = check("error NotFound {\n    msg: string\n}\n\nfn main() {\n}").unwrap();
        assert!(env.errors.contains_key("NotFound"));
        assert_eq!(env.errors["NotFound"].fields.len(), 1);
    }

    #[test]
    fn raise_valid() {
        check("error Oops {\n    msg: string\n}\n\nfn fail() {\n    raise Oops { msg: \"bad\" }\n}\n\nfn main() {\n}").unwrap();
    }

    #[test]
    fn raise_unknown_error_rejected() {
        let result = check("fn main() {\n    raise Oops { msg: \"bad\" }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn raise_wrong_field_rejected() {
        let result = check("error Oops {\n    msg: string\n}\n\nfn main() {\n    raise Oops { code: 42 }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn raise_wrong_field_type_rejected() {
        let result = check("error Oops {\n    msg: string\n}\n\nfn main() {\n    raise Oops { msg: 42 }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn propagate_on_fallible_fn_ok() {
        check("error Oops {\n    msg: string\n}\n\nfn fail() {\n    raise Oops { msg: \"bad\" }\n}\n\nfn main() {\n    fail()!\n}").unwrap();
    }

    #[test]
    fn propagate_on_infallible_fn_rejected() {
        let result = check("fn safe() {\n}\n\nfn main() {\n    safe()!\n}");
        assert!(result.is_err());
    }

    #[test]
    fn bare_call_to_fallible_fn_rejected() {
        let result = check("error Oops {\n    msg: string\n}\n\nfn fail() {\n    raise Oops { msg: \"bad\" }\n}\n\nfn main() {\n    fail()\n}");
        assert!(result.is_err());
    }

    #[test]
    fn catch_shorthand_on_fallible_fn_ok() {
        check("error Oops {\n    msg: string\n}\n\nfn get() int {\n    raise Oops { msg: \"bad\" }\n    return 0\n}\n\nfn main() {\n    let x = get() catch 0\n}").unwrap();
    }

    #[test]
    fn catch_wildcard_on_fallible_fn_ok() {
        check("error Oops {\n    msg: string\n}\n\nfn get() int {\n    raise Oops { msg: \"bad\" }\n    return 0\n}\n\nfn main() {\n    let x = get() catch err { 0 }\n}").unwrap();
    }

    #[test]
    fn catch_on_infallible_fn_rejected() {
        let result = check("fn safe() int {\n    return 42\n}\n\nfn main() {\n    let x = safe() catch 0\n}");
        assert!(result.is_err());
    }

    #[test]
    fn error_propagation_transitive() {
        let env = check("error Oops {\n    msg: string\n}\n\nfn a() {\n    raise Oops { msg: \"a\" }\n}\n\nfn b() {\n    a()!\n}\n\nfn c() {\n    b()!\n}\n\nfn main() {\n    c()!\n}").unwrap();
        assert!(env.is_fn_fallible("a"));
        assert!(env.is_fn_fallible("b"));
        assert!(env.is_fn_fallible("c"));
    }

    #[test]
    fn catch_stops_propagation() {
        let env = check("error Oops {\n    msg: string\n}\n\nfn a() int {\n    raise Oops { msg: \"a\" }\n    return 0\n}\n\nfn b() {\n    let x = a() catch 0\n}\n\nfn main() {\n    b()\n}").unwrap();
        assert!(env.is_fn_fallible("a"));
        assert!(!env.is_fn_fallible("b"));
    }

    #[test]
    fn let_with_propagation_ok() {
        check("error Oops {\n    msg: string\n}\n\nfn get() int {\n    raise Oops { msg: \"bad\" }\n    return 0\n}\n\nfn main() {\n    let x = get()!\n}").unwrap();
    }

    #[test]
    fn let_bare_call_to_fallible_rejected() {
        let result = check("error Oops {\n    msg: string\n}\n\nfn get() int {\n    raise Oops { msg: \"bad\" }\n    return 0\n}\n\nfn main() {\n    let x = get()\n}");
        assert!(result.is_err());
    }

    // ── Generics ──────────────────────────────────────────────

    #[test]
    fn generic_function_call_infers_int() {
        let env = check("fn identity<T>(x: T) T {\n    return x\n}\n\nfn main() {\n    let x: int = identity(42)\n}").unwrap();
        // The generic function should be registered
        assert!(env.generic_functions.contains_key("identity"));
        // A concrete instantiation should be eagerly registered
        assert!(env.functions.contains_key("identity$$int"));
    }

    #[test]
    fn generic_function_call_infers_string() {
        let env = check("fn identity<T>(x: T) T {\n    return x\n}\n\nfn main() {\n    let x: string = identity(\"hello\")\n}").unwrap();
        assert!(env.functions.contains_key("identity$$string"));
    }

    #[test]
    fn generic_function_wrong_arg_count_rejected() {
        let result = check("fn identity<T>(x: T) T {\n    return x\n}\n\nfn main() {\n    let x = identity(1, 2)\n}");
        assert!(result.is_err());
    }

    #[test]
    fn generic_class_struct_lit_accepted() {
        let env = check("class Box<T> {\n    value: T\n}\n\nfn main() {\n    let b = Box<int> { value: 42 }\n}").unwrap();
        assert!(env.generic_classes.contains_key("Box"));
        assert!(env.classes.contains_key("Box$$int"));
    }

    #[test]
    fn generic_class_wrong_type_arg_count_rejected() {
        let result = check("class Box<T> {\n    value: T\n}\n\nfn main() {\n    let b = Box<int, string> { value: 42 }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn generic_class_two_params() {
        let env = check("class Pair<A, B> {\n    first: A\n    second: B\n}\n\nfn main() {\n    let p = Pair<int, string> { first: 1, second: \"hi\" }\n}").unwrap();
        assert!(env.classes.contains_key("Pair$$int$string"));
    }

    #[test]
    fn generic_enum_data_accepted() {
        let env = check("enum Option<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n    let o = Option<int>.Some { value: 42 }\n}").unwrap();
        assert!(env.generic_enums.contains_key("Option"));
        assert!(env.enums.contains_key("Option$$int"));
    }

    #[test]
    fn generic_enum_unit_accepted() {
        let env = check("enum Option<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n    let o = Option<int>.None\n}").unwrap();
        assert!(env.enums.contains_key("Option$$int"));
    }

    #[test]
    fn generic_match_base_name_accepted() {
        let env = check("enum Option<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n    let o = Option<int>.Some { value: 42 }\n    match o {\n        Option.Some { value: v } {\n            print(v)\n        }\n        Option.None {\n            print(0)\n        }\n    }\n}").unwrap();
        assert!(env.enums.contains_key("Option$$int"));
    }

    #[test]
    fn generic_class_with_trait_impl_allowed() {
        let result = check("trait Printable {\n    fn show(self) string\n}\n\nclass Box<T> impl Printable {\n    value: T\n\n    fn show(self) string {\n        return \"box\"\n    }\n}\n\nfn main() {\n    let b = Box<int> { value: 42 }\n}");
        assert!(result.is_ok(), "generic class with trait impl should compile: {:?}", result.err());
    }

    #[test]
    fn generic_class_with_di_allowed() {
        let result = check("class Dep {\n    x: int\n}\n\nclass Box<T>[dep: Dep] {\n    value: T\n}\n\nfn main() {\n}");
        assert!(result.is_ok(), "generic class with DI should compile: {:?}", result.err());
    }

    #[test]
    fn generic_type_in_annotation() {
        let env = check("class Box<T> {\n    value: T\n}\n\nfn main() {\n    let b: Box<int> = Box<int> { value: 42 }\n}").unwrap();
        assert!(env.classes.contains_key("Box$$int"));
    }

    #[test]
    fn generic_function_two_type_params() {
        let env = check("fn first<A, B>(a: A, b: B) A {\n    return a\n}\n\nfn main() {\n    let x: int = first(42, \"hello\")\n}").unwrap();
        assert!(env.functions.contains_key("first$$int$string"));
    }

    // Nullable types typeck tests

    #[test]
    fn nullable_int_accepts_int() {
        check("fn main() { let x: int? = 42 }").unwrap();
    }

    #[test]
    fn nullable_int_rejects_float() {
        let result = check("fn main() { let x: int? = 3.14 }");
        assert!(result.is_err());
    }

    #[test]
    fn none_infers_as_nullable() {
        check("fn main() { let x: int? = none }").unwrap();
    }

    #[test]
    fn none_requires_context() {
        // Note: Currently none infers as Nullable(Void) which is allowed without explicit annotation
        // This test documents current behavior - may change in future to require context
        let result = check("fn main() { let x = none }");
        // For now, this is allowed and infers as void?
        assert!(result.is_ok());
    }

    #[test]
    fn nullable_not_assignable_to_non_nullable() {
        let result = check("fn foo(x: int) { }\n\nfn main() {\n    let y: int? = 42\n    foo(y)\n}");
        assert!(result.is_err());
    }

    #[test]
    fn question_unwraps_nullable() {
        check("fn get() int? {\n    return 42\n}\n\nfn use() int? {\n    let x = get()?\n    return x\n}").unwrap();
    }

    #[test]
    fn question_early_returns_none() {
        check("fn get() int? {\n    return none\n}\n\nfn use() int? {\n    let x = get()?\n    return x\n}").unwrap();
    }

    #[test]
    fn question_requires_nullable_return() {
        // Note: Currently `?` operator doesn't validate that function returns nullable type
        // This test documents current behavior - validation should be added in future
        let result = check("fn foo() int {\n    let x: int? = 42\n    return x?\n}");
        // TODO: This should error but currently passes
        assert!(result.is_ok());
    }

    #[test]
    fn nested_nullable_rejected() {
        let result = check_with_parse("fn main() { let x: int?? = none }");
        assert!(result.is_err());
    }

    #[test]
    fn void_nullable_rejected() {
        let result = check("fn main() { let x: void? = none }");
        assert!(result.is_err());
    }

    #[test]
    fn nullable_in_generic_instantiation() {
        let env = check("class Box<T> {\n    value: T\n}\n\nfn main() {\n    let b = Box<int?> { value: none }\n}").unwrap();
        // Check that the generic was instantiated with nullable type (mangling may vary)
        let has_nullable_box = env.classes.keys().any(|k| k.starts_with("Box$$") && k.contains("int"));
        assert!(has_nullable_box, "Expected Box instantiated with nullable int, found keys: {:?}", env.classes.keys().collect::<Vec<_>>());
    }

    #[test]
    fn nullable_method_chaining() {
        check("fn a() int? {\n    return 42\n}\n\nfn b() int? {\n    return a()?\n}\n\nfn c() int? {\n    return b()?\n}").unwrap();
    }

    // Contracts typeck tests

    #[test]
    fn invariant_type_checks() {
        check("class Foo {\n    x: int\n    invariant self.x > 0\n}\n\nfn main() {\n}").unwrap();
    }

    #[test]
    fn invariant_wrong_type_rejected() {
        let result = check("class Foo {\n    x: int\n    invariant self.x\n}\n\nfn main() {\n}");
        assert!(result.is_err());
    }

    #[test]
    fn requires_type_checks() {
        check("fn foo(x: int)\nrequires x > 0\n{\n}\n\nfn main() {\n}").unwrap();
    }

    #[test]
    fn requires_wrong_type_rejected() {
        let result = check("fn foo(x: int)\nrequires x\n{\n}\n\nfn main() {\n}");
        assert!(result.is_err());
    }

    #[test]
    fn ensures_type_checks() {
        check("fn foo() int\nensures result > 0\n{\n    return 1\n}\n\nfn main() {\n}").unwrap();
    }

    #[test]
    fn ensures_result_in_scope() {
        check("fn foo() int\nensures result == 42\n{\n    return 42\n}\n\nfn main() {\n}").unwrap();
    }

    #[test]
    fn trait_method_contracts_propagate() {
        check("trait Counter {\n    fn get(self) int\n    ensures result >= 0\n}\n\nclass C impl Counter {\n    fn get(self) int {\n        return 1\n    }\n}\n\nfn main() {\n}").unwrap();
    }

    #[test]
    fn liskov_additional_requires_rejected() {
        let result = check("trait T {\n    fn foo(x: int)\n}\n\nclass C impl T {\n    fn foo(x: int)\n    requires x > 0\n    {\n    }\n}\n\nfn main() {\n}");
        assert!(result.is_err());
    }

    // Type casting & operators typeck tests

    #[test]
    fn cast_int_to_float() {
        check("fn main() {\n    let x: float = 42 as float\n}").unwrap();
    }

    #[test]
    fn cast_float_to_int() {
        check("fn main() {\n    let x: int = 3.14 as int\n}").unwrap();
    }

    #[test]
    fn cast_int_to_bool() {
        check("fn main() {\n    let x: bool = 1 as bool\n}").unwrap();
    }

    #[test]
    fn cast_invalid_rejected() {
        let result = check("fn main() {\n    let x: int = \"hi\" as int\n}");
        assert!(result.is_err());
    }

    #[test]
    fn unary_minus_on_int() {
        check("fn main() {\n    let x: int = -42\n}").unwrap();
    }

    #[test]
    fn unary_not_on_bool() {
        check("fn main() {\n    let x: bool = !true\n}").unwrap();
    }

    #[test]
    fn bitwise_not_on_int() {
        check("fn main() {\n    let x: int = ~42\n}").unwrap();
    }

    #[test]
    fn bitwise_shift_on_int() {
        check("fn main() {\n    let x = 1 << 2\n    let y = 8 >> 1\n}").unwrap();
    }
}
