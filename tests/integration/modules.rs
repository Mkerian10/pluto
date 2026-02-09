use std::process::Command;

fn plutoc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_plutoc"))
}

/// Write multiple files to a temp directory, compile the entry file, and return stdout.
fn run_project(files: &[(&str, &str)]) -> String {
    let dir = tempfile::tempdir().unwrap();

    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    let compile_output = plutoc()
        .arg("compile")
        .arg(&entry)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();

    assert!(
        compile_output.status.success(),
        "Compilation failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&compile_output.stdout),
        String::from_utf8_lossy(&compile_output.stderr)
    );

    assert!(bin_path.exists(), "Binary was not created");

    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(run_output.status.success(), "Binary exited with non-zero status");
    String::from_utf8_lossy(&run_output.stdout).to_string()
}

/// Write multiple files to a temp directory, compile entry file, assert compilation fails.
fn compile_project_should_fail(files: &[(&str, &str)]) {
    let dir = tempfile::tempdir().unwrap();

    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    let output = plutoc()
        .arg("compile")
        .arg(&entry)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();

    assert!(!output.status.success(), "Compilation should have failed");
}

// ============================================================
// Import single-file module
// ============================================================

#[test]
fn import_single_file_module() {
    let out = run_project(&[
        ("main.pluto", "import math\n\nfn main() {\n    print(math.add(1, 2))\n}"),
        ("math.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
    ]);
    assert_eq!(out, "3\n");
}

// ============================================================
// Import directory module
// ============================================================

#[test]
fn import_directory_module() {
    let out = run_project(&[
        ("main.pluto", "import math\n\nfn main() {\n    print(math.add(1, 2))\n    print(math.mul(3, 4))\n}"),
        ("math/add.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
        ("math/mul.pluto", "pub fn mul(a: int, b: int) int {\n    return a * b\n}"),
    ]);
    assert_eq!(out, "3\n12\n");
}

// ============================================================
// Import class and use qualified struct literal
// ============================================================

#[test]
fn import_class_struct_literal() {
    let out = run_project(&[
        ("main.pluto", "import geo\n\nfn main() {\n    let p = geo.Point { x: 10, y: 20 }\n    print(p.x)\n    print(p.y)\n}"),
        ("geo.pluto", "pub class Point {\n    x: int\n    y: int\n}"),
    ]);
    assert_eq!(out, "10\n20\n");
}

// ============================================================
// Call imported function
// ============================================================

#[test]
fn call_imported_function() {
    let out = run_project(&[
        ("main.pluto", "import utils\n\nfn main() {\n    print(utils.double(21))\n}"),
        ("utils.pluto", "pub fn double(x: int) int {\n    return x * 2\n}"),
    ]);
    assert_eq!(out, "42\n");
}

// ============================================================
// Qualified type in parameter
// ============================================================

#[test]
fn qualified_type_in_param() {
    let out = run_project(&[
        ("main.pluto", "import geo\n\nfn show(p: geo.Point) {\n    print(p.x)\n    print(p.y)\n}\n\nfn main() {\n    let p = geo.Point { x: 5, y: 6 }\n    show(p)\n}"),
        ("geo.pluto", "pub class Point {\n    x: int\n    y: int\n}"),
    ]);
    assert_eq!(out, "5\n6\n");
}

// ============================================================
// Private function not visible
// ============================================================

#[test]
fn private_function_not_visible() {
    compile_project_should_fail(&[
        ("main.pluto", "import math\n\nfn main() {\n    print(math.secret(1))\n}"),
        ("math.pluto", "fn secret(x: int) int {\n    return x\n}\n\npub fn add(a: int, b: int) int {\n    return a + b\n}"),
    ]);
}

// ============================================================
// Private class not visible
// ============================================================

#[test]
fn private_class_not_visible() {
    compile_project_should_fail(&[
        ("main.pluto", "import geo\n\nfn main() {\n    let p = geo.Internal { x: 1 }\n}"),
        ("geo.pluto", "class Internal {\n    x: int\n}\n\npub class Point {\n    x: int\n    y: int\n}"),
    ]);
}

// ============================================================
// Intra-module: two files in same directory see each other
// ============================================================

#[test]
fn intra_module_same_directory() {
    let out = run_project(&[
        ("main.pluto", "fn main() {\n    print(helper())\n}"),
        ("helper.pluto", "fn helper() int {\n    return 99\n}"),
    ]);
    assert_eq!(out, "99\n");
}

// ============================================================
// Files within imported directory module see each other
// ============================================================

#[test]
fn intra_module_directory() {
    let out = run_project(&[
        ("main.pluto", "import math\n\nfn main() {\n    print(math.add_double(2, 3))\n}"),
        ("math/core.pluto", "pub fn double(x: int) int {\n    return x * 2\n}"),
        ("math/ops.pluto", "pub fn add_double(a: int, b: int) int {\n    return double(a + b)\n}"),
    ]);
    assert_eq!(out, "10\n");
}

// ============================================================
// Missing module → error
// ============================================================

#[test]
fn missing_module_error() {
    compile_project_should_fail(&[
        ("main.pluto", "import nonexistent\n\nfn main() {\n}"),
    ]);
}

// ============================================================
// Single-file backward compat (no imports)
// ============================================================

#[test]
fn single_file_no_imports() {
    let out = run_project(&[
        ("main.pluto", "fn main() {\n    print(42)\n}"),
    ]);
    assert_eq!(out, "42\n");
}

// ============================================================
// Imported class with method
// ============================================================

#[test]
fn import_class_with_method() {
    let out = run_project(&[
        ("main.pluto", "import geo\n\nfn main() {\n    let p = geo.Point { x: 3, y: 4 }\n    print(p.sum())\n}"),
        ("geo.pluto", "pub class Point {\n    x: int\n    y: int\n\n    fn sum(self) int {\n        return self.x + self.y\n    }\n}"),
    ]);
    assert_eq!(out, "7\n");
}

// ============================================================
// Multiple imports
// ============================================================

#[test]
fn multiple_imports() {
    let out = run_project(&[
        ("main.pluto", "import math\nimport strings\n\nfn main() {\n    print(math.add(1, 2))\n    print(strings.greet())\n}"),
        ("math.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
        ("strings.pluto", "pub fn greet() string {\n    return \"hello\"\n}"),
    ]);
    assert_eq!(out, "3\nhello\n");
}

// ============================================================
// Imported function returning class
// ============================================================

#[test]
fn import_function_returning_class() {
    let out = run_project(&[
        ("main.pluto", "import geo\n\nfn main() {\n    let p = geo.make(10, 20)\n    print(p.x)\n    print(p.y)\n}"),
        ("geo.pluto", "pub class Point {\n    x: int\n    y: int\n}\n\npub fn make(x: int, y: int) Point {\n    return Point { x: x, y: y }\n}"),
    ]);
    assert_eq!(out, "10\n20\n");
}
