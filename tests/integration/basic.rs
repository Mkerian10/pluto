use std::process::Command;

fn plutoc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_plutoc"))
}

fn compile_and_run(source: &str) -> i32 {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("test.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&src_path, source).unwrap();

    let compile_output = plutoc()
        .arg("compile")
        .arg(&src_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();

    assert!(
        compile_output.status.success(),
        "Compilation failed: {}",
        String::from_utf8_lossy(&compile_output.stderr)
    );

    assert!(bin_path.exists(), "Binary was not created");

    let run_output = Command::new(&bin_path).output().unwrap();
    run_output.status.code().unwrap_or(-1)
}

fn compile_should_fail(source: &str) {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("test.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&src_path, source).unwrap();

    let output = plutoc()
        .arg("compile")
        .arg(&src_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();

    assert!(!output.status.success(), "Compilation should have failed");
}

#[test]
fn empty_main() {
    let code = compile_and_run("fn main() { }");
    assert_eq!(code, 0);
}

#[test]
fn main_with_let() {
    let code = compile_and_run("fn main() {\n    let x = 42\n}");
    assert_eq!(code, 0);
}

#[test]
fn function_call() {
    let code = compile_and_run(
        "fn add(a: int, b: int) int {\n    return a + b\n}\n\nfn main() {\n    let x = add(1, 2)\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn arithmetic_operations() {
    let code = compile_and_run(
        "fn main() {\n    let a = 10\n    let b = 3\n    let sum = a + b\n    let diff = a - b\n    let prod = a * b\n    let quot = a / b\n    let rem = a % b\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn boolean_operations() {
    let code = compile_and_run(
        "fn main() {\n    let a = true\n    let b = false\n    let c = 1 < 2\n    let d = 3 == 3\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn if_else() {
    let code = compile_and_run(
        "fn main() {\n    if true {\n        let x = 1\n    } else {\n        let x = 2\n    }\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn while_loop() {
    let code = compile_and_run(
        "fn main() {\n    let x = 0\n    while x < 10 {\n        x = x + 1\n    }\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn nested_function_calls() {
    let code = compile_and_run(
        "fn double(x: int) int {\n    return x * 2\n}\n\nfn add_one(x: int) int {\n    return x + 1\n}\n\nfn main() {\n    let result = add_one(double(5))\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn type_error_rejected() {
    compile_should_fail("fn main() {\n    let x: int = true\n}");
}

#[test]
fn undefined_variable_rejected() {
    compile_should_fail("fn main() {\n    let x = y\n}");
}

#[test]
fn undefined_function_rejected() {
    compile_should_fail("fn main() {\n    let x = foo(1)\n}");
}
