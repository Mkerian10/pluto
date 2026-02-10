mod common;

/// Helper: compile source and return warning messages.
fn compile_and_get_warnings(source: &str) -> Vec<String> {
    match plutoc::compile_to_object_with_warnings(source) {
        Ok((_obj, warnings)) => warnings.iter().map(|w| w.msg.clone()).collect(),
        Err(e) => panic!("Compilation failed unexpectedly: {e}"),
    }
}

#[test]
fn unused_let_variable_warns() {
    let warnings = compile_and_get_warnings(
        "fn main() {\n    let x = 42\n}",
    );
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("unused variable 'x'"));
}

#[test]
fn used_variable_no_warning() {
    let warnings = compile_and_get_warnings(
        "fn main() {\n    let x = 42\n    print(x)\n}",
    );
    assert!(warnings.is_empty(), "expected no warnings, got: {:?}", warnings);
}

#[test]
fn underscore_prefix_suppresses() {
    let warnings = compile_and_get_warnings(
        "fn main() {\n    let _x = 42\n}",
    );
    assert!(warnings.is_empty(), "expected no warnings for _-prefixed var, got: {:?}", warnings);
}

#[test]
fn for_loop_variable_not_warned() {
    let warnings = compile_and_get_warnings(
        "fn main() {\n    let a = [1, 2, 3]\n    for i in a {\n        print(i)\n    }\n}",
    );
    // 'a' is used by the for-loop, 'i' is a loop variable — no warnings expected
    assert!(warnings.is_empty(), "expected no warnings, got: {:?}", warnings);
}

#[test]
fn function_param_not_warned() {
    let warnings = compile_and_get_warnings(
        "fn foo(x: int) {\n    let y = 1\n    print(y)\n}\n\nfn main() {\n    foo(42)\n}",
    );
    // 'x' is a function parameter — should not be warned even if unused
    assert!(warnings.is_empty(), "expected no warnings, got: {:?}", warnings);
}

#[test]
fn multiple_unused_variables() {
    let warnings = compile_and_get_warnings(
        "fn main() {\n    let a = 1\n    let b = 2\n    let c = 3\n}",
    );
    assert_eq!(warnings.len(), 3);
    assert!(warnings[0].contains("unused variable 'a'"));
    assert!(warnings[1].contains("unused variable 'b'"));
    assert!(warnings[2].contains("unused variable 'c'"));
}

#[test]
fn variable_written_but_never_read() {
    let warnings = compile_and_get_warnings(
        "fn main() {\n    let x = 1\n    let y = x + 1\n}",
    );
    // x is read (in x + 1), y is never read
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("unused variable 'y'"));
}

#[test]
fn variables_in_different_scopes() {
    let warnings = compile_and_get_warnings(
        "fn main() {\n    let x = 1\n    if true {\n        let y = 2\n        print(y)\n    }\n}",
    );
    // x is unused, y is used in inner scope
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("unused variable 'x'"));
}

#[test]
fn warning_does_not_prevent_compilation() {
    // Compile with unused variable — should still produce valid object bytes
    let result = plutoc::compile_to_object_with_warnings(
        "fn main() {\n    let x = 42\n}",
    );
    assert!(result.is_ok(), "compilation should succeed despite warnings");
    let (obj, warnings) = result.unwrap();
    assert!(!obj.is_empty(), "object bytes should not be empty");
    assert!(!warnings.is_empty(), "should have at least one warning");
}

#[test]
fn no_warnings_for_clean_code() {
    let warnings = compile_and_get_warnings(
        "fn add(a: int, b: int) int {\n    return a + b\n}\n\nfn main() {\n    let result = add(1, 2)\n    print(result)\n}",
    );
    assert!(warnings.is_empty(), "expected no warnings for clean code, got: {:?}", warnings);
}

#[test]
fn method_param_not_warned() {
    let warnings = compile_and_get_warnings(
        "class Foo {\n    value: int\n    fn bar(self, x: int) {\n        print(self.value)\n    }\n}\n\nfn main() {\n    let f = Foo { value: 1 }\n    f.bar(42)\n}",
    );
    // 'x' is a method param, 'f' is used — no warnings
    assert!(warnings.is_empty(), "expected no warnings, got: {:?}", warnings);
}
