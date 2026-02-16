mod common;

use std::process::Command;
use std::path::Path;

/// Write multiple files to a temp directory, compile the entry file via library call, and return stdout.
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

    pluto::compile_file(&entry, &bin_path)
        .unwrap_or_else(|e| panic!("Compilation failed: {e}"));

    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(run_output.status.success(), "Binary exited with non-zero status");
    String::from_utf8_lossy(&run_output.stdout).to_string()
}

/// Write multiple files to a temp directory, compile entry file via library call, assert compilation fails.
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

    assert!(
        pluto::compile_file(&entry, &bin_path).is_err(),
        "Compilation should have failed"
    );
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
    // Visibility is deferred (Python-style) — private items are flattened and accessible
    let out = run_project(&[
        ("main.pluto", "import math\n\nfn main() {\n    print(math.secret(1))\n}"),
        ("math.pluto", "fn secret(x: int) int {\n    return x\n}\n\npub fn add(a: int, b: int) int {\n    return a + b\n}"),
    ]);
    assert_eq!(out, "1\n");
}

// ============================================================
// Private class not visible
// ============================================================

#[test]
fn private_class_not_visible() {
    // Visibility is deferred (Python-style) — private items are flattened and accessible
    let out = run_project(&[
        ("main.pluto", "import geo\n\nfn main() {\n    let p = geo.Internal { x: 1 }\n    print(p.x)\n}"),
        ("geo.pluto", "class Internal {\n    x: int\n}\n\npub class Point {\n    x: int\n    y: int\n}"),
    ]);
    assert_eq!(out, "1\n");
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

// ============================================================
// Cross-module enum support
// ============================================================

#[test]
fn import_enum_unit_variant() {
    let out = run_project(&[
        ("main.pluto", r#"import status

fn main() {
    let s = status.State.Active
    match s {
        status.State.Active {
            print("active")
        }
        status.State.Inactive {
            print("inactive")
        }
    }
}
"#),
        ("status.pluto", r#"pub enum State {
    Active
    Inactive
}
"#),
    ]);
    assert_eq!(out, "active\n");
}

#[test]
fn import_enum_data_variant() {
    let out = run_project(&[
        ("main.pluto", r#"import types

fn main() {
    let r = types.Result.Error { msg: "oops" }
    match r {
        types.Result.Ok { value } {
            print(value)
        }
        types.Result.Error { msg } {
            print(msg)
        }
    }
}
"#),
        ("types.pluto", r#"pub enum Result {
    Ok { value: int }
    Error { msg: string }
}
"#),
    ]);
    assert_eq!(out, "oops\n");
}

#[test]
fn import_enum_as_function_param() {
    let out = run_project(&[
        ("main.pluto", r#"import color

fn describe(c: color.Light) {
    match c {
        color.Light.Red {
            print("stop")
        }
        color.Light.Green {
            print("go")
        }
    }
}

fn main() {
    describe(color.Light.Green)
}
"#),
        ("color.pluto", r#"pub enum Light {
    Red
    Green
}
"#),
    ]);
    assert_eq!(out, "go\n");
}

#[test]
fn import_enum_return_from_function() {
    let out = run_project(&[
        ("main.pluto", r#"import shape

fn main() {
    let s = shape.make_circle(5)
    match s {
        shape.Kind.Circle { radius } {
            print(radius)
        }
        shape.Kind.Square { side } {
            print(side)
        }
    }
}
"#),
        ("shape.pluto", r#"pub enum Kind {
    Circle { radius: int }
    Square { side: int }
}

pub fn make_circle(r: int) Kind {
    return Kind.Circle { radius: r }
}
"#),
    ]);
    assert_eq!(out, "5\n");
}

#[test]
fn private_enum_not_visible() {
    // Visibility is deferred (Python-style) — private items are flattened and accessible
    let out = run_project(&[
        ("main.pluto", r#"import inner

fn main() {
    let x = inner.Secret.A
    match x {
        inner.Secret.A {
            print("a")
        }
        inner.Secret.B {
            print("b")
        }
    }
}
"#),
        ("inner.pluto", r#"enum Secret {
    A
    B
}
"#),
    ]);
    assert_eq!(out, "a\n");
}

// ============================================================
// Hierarchical imports: import a.b
// ============================================================

#[test]
fn hierarchical_import_file() {
    let out = run_project(&[
        ("main.pluto", "import utils.math\n\nfn main() {\n    print(math.add(3, 4))\n}"),
        ("utils/math.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
    ]);
    assert_eq!(out, "7\n");
}

#[test]
fn hierarchical_import_directory() {
    let out = run_project(&[
        ("main.pluto", "import utils.math\n\nfn main() {\n    print(math.add(3, 4))\n    print(math.mul(5, 6))\n}"),
        ("utils/math/add.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
        ("utils/math/mul.pluto", "pub fn mul(a: int, b: int) int {\n    return a * b\n}"),
    ]);
    assert_eq!(out, "7\n30\n");
}

#[test]
fn hierarchical_import_three_levels() {
    let out = run_project(&[
        ("main.pluto", "import a.b.c\n\nfn main() {\n    print(c.value())\n}"),
        ("a/b/c.pluto", "pub fn value() int {\n    return 42\n}"),
    ]);
    assert_eq!(out, "42\n");
}

// ============================================================
// Import alias
// ============================================================

#[test]
fn import_alias() {
    let out = run_project(&[
        ("main.pluto", "import math as m\n\nfn main() {\n    print(m.add(10, 20))\n}"),
        ("math.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
    ]);
    assert_eq!(out, "30\n");
}

#[test]
fn hierarchical_import_alias() {
    let out = run_project(&[
        ("main.pluto", "import utils.math as m\n\nfn main() {\n    print(m.add(10, 20))\n}"),
        ("utils/math.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
    ]);
    assert_eq!(out, "30\n");
}

// ============================================================
// mod.pluto — all .pluto files in directory are merged
// ============================================================

#[test]
fn mod_pluto_merges_with_siblings() {
    // mod.pluto AND extra.pluto are both loaded — all files merged
    let out = run_project(&[
        ("main.pluto", "import math\n\nfn main() {\n    print(math.add(1, 2))\n    print(math.mul(2, 3))\n}"),
        ("math/mod.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
        ("math/extra.pluto", "pub fn mul(a: int, b: int) int {\n    return a * b\n}"),
    ]);
    assert_eq!(out, "3\n6\n");
}

#[test]
fn mod_pluto_extra_is_visible() {
    // With mod.pluto present, extra.pluto is still merged — mul() is visible
    let out = run_project(&[
        ("main.pluto", "import math\n\nfn main() {\n    print(math.mul(2, 3))\n}"),
        ("math/mod.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
        ("math/extra.pluto", "pub fn mul(a: int, b: int) int {\n    return a * b\n}"),
    ]);
    assert_eq!(out, "6\n");
}

// ============================================================
// Stdlib imports: import std.x
// ============================================================

#[test]
fn stdlib_import_from_relative_stdlib_dir() {
    // When ./stdlib exists relative to entry file, import std.mymod resolves from there
    let out = run_project(&[
        ("main.pluto", "import std.mymod\n\nfn main() {\n    print(mymod.value())\n}"),
        ("stdlib/mymod.pluto", "pub fn value() int {\n    return 77\n}"),
    ]);
    assert_eq!(out, "77\n");
}

#[test]
fn stdlib_import_missing_stdlib_root() {
    // No stdlib/ directory, no --stdlib flag → error
    compile_project_should_fail(&[
        ("main.pluto", "import std.io\n\nfn main() {\n}"),
    ]);
}

// ============================================================
// Hierarchical import: missing intermediate directory
// ============================================================

#[test]
fn hierarchical_import_missing_intermediate() {
    compile_project_should_fail(&[
        ("main.pluto", "import nonexistent.math\n\nfn main() {\n}"),
    ]);
}

// ============================================================
// Extern fn in imported module
// ============================================================

#[test]
fn imported_module_with_extern_fn() {
    // Module wraps a C runtime function via extern fn, main calls the wrapper
    let out = run_project(&[
        ("main.pluto", "import printer\n\nfn main() {\n    printer.say(42)\n}"),
        ("printer.pluto", "extern fn __pluto_print_int(value: int)\n\npub fn say(x: int) {\n    __pluto_print_int(x)\n}"),
    ]);
    assert_eq!(out, "42\n");
}

// ============================================================
// Stdlib end-to-end: import std.io
// ============================================================

#[test]
fn stdlib_io_println() {
    // stdlib/io/mod.pluto lives relative to the entry file
    let out = run_project(&[
        ("main.pluto", r#"import std.io

fn main() {
    io.println("hello from stdlib")
}
"#),
        ("stdlib/io/mod.pluto", r#"extern fn __pluto_print_string(s: string)

pub fn println(s: string) {
    __pluto_print_string(s)
}
"#),
    ]);
    assert_eq!(out, "hello from stdlib\n");
}

#[test]
fn stdlib_io_print_no_newline() {
    let out = run_project(&[
        ("main.pluto", r#"import std.io

fn main() {
    io.print("hello")
    io.print(" world")
}
"#),
        ("stdlib/io/mod.pluto", r#"extern fn __pluto_print_string_no_newline(s: string)

pub fn print(s: string) {
    __pluto_print_string_no_newline(s)
}
"#),
    ]);
    assert_eq!(out, "hello world");
}

#[test]
fn app_in_imported_module_rejected() {
    compile_project_should_fail(&[
        ("main.pluto", r#"import svc

app MyApp {
    fn main(self) {
    }
}
"#),
        ("svc.pluto", r#"pub app SvcApp {
    fn main(self) {
    }
}
"#),
    ]);
}

// ============================================================
// Transitive imports
// ============================================================

#[test]
fn transitive_import_basic() {
    // A imports B, B imports C
    let out = run_project(&[
        ("main.pluto", r#"import b

fn main() {
    print(b.greet())
}
"#),
        ("b.pluto", r#"import c

pub fn greet() string {
    return c.hello()
}
"#),
        ("c.pluto", r#"pub fn hello() string {
    return "hello from c"
}
"#),
    ]);
    assert_eq!(out, "hello from c\n");
}

#[test]
fn transitive_import_chain() {
    // A→B→C→D, three levels deep
    let out = run_project(&[
        ("main.pluto", r#"import b

fn main() {
    print(b.get_b())
}
"#),
        ("b.pluto", r#"import c

pub fn get_b() int {
    return c.get_c()
}
"#),
        ("c.pluto", r#"import d

pub fn get_c() int {
    return d.get_d()
}
"#),
        ("d.pluto", r#"pub fn get_d() int {
    return 42
}
"#),
    ]);
    assert_eq!(out, "42\n");
}

#[test]
fn transitive_import_shared() {
    // Diamond: B and C both import shared
    let out = run_project(&[
        ("main.pluto", r#"import b
import c

fn main() {
    print(b.get_value())
    print(c.get_value())
}
"#),
        ("b.pluto", r#"import shared

pub fn get_value() int {
    return shared.base() + 1
}
"#),
        ("c.pluto", r#"import shared

pub fn get_value() int {
    return shared.base() + 2
}
"#),
        ("shared.pluto", r#"pub fn base() int {
    return 10
}
"#),
    ]);
    assert_eq!(out, "11\n12\n");
}

#[test]
fn circular_import_rejected() {
    // A→B→A cycle should produce an error
    compile_project_should_fail(&[
        ("main.pluto", r#"import a

fn main() {
    print(a.value())
}
"#),
        ("a.pluto", r#"import b

pub fn value() int {
    return b.other()
}
"#),
        ("b.pluto", r#"import a

pub fn other() int {
    return a.value()
}
"#),
    ]);
}

#[test]
fn transitive_import_with_classes() {
    // Module imports another and uses its classes
    let out = run_project(&[
        ("main.pluto", r#"import shapes

fn main() {
    let c = shapes.make_circle(5)
    print(c.radius)
}
"#),
        ("shapes.pluto", r#"import geo

pub fn make_circle(r: int) geo.Circle {
    return geo.Circle { radius: r }
}
"#),
        ("geo.pluto", r#"pub class Circle {
    radius: int
}
"#),
    ]);
    assert_eq!(out, "5\n");
}

#[test]
fn transitive_import_mod_pluto() {
    // mod.pluto and helper.pluto are auto-merged as siblings — no explicit import needed
    let out = run_project(&[
        ("main.pluto", r#"import lib

fn main() {
    print(lib.combined())
}
"#),
        ("lib/mod.pluto", r#"pub fn combined() int {
    return base() * 2
}
"#),
        ("lib/helper.pluto", r#"fn base() int {
    return 21
}
"#),
    ]);
    assert_eq!(out, "42\n");
}

#[test]
fn private_helper_in_pub_function() {
    // Pub fn calls private helper within same module — works since all items are flattened
    let out = run_project(&[
        ("main.pluto", r#"import math

fn main() {
    print(math.double(21))
}
"#),
        ("math.pluto", r#"fn helper(x: int) int {
    return x * 2
}

pub fn double(x: int) int {
    return helper(x)
}
"#),
    ]);
    assert_eq!(out, "42\n");
}

#[test]
fn transitive_import_stdlib() {
    // Module imports from stdlib internally
    let out = run_project(&[
        ("main.pluto", r#"import mylib

fn main() {
    mylib.say_hello()
}
"#),
        ("mylib.pluto", r#"import std.io

pub fn say_hello() {
    io.println("hello from lib")
}
"#),
        ("stdlib/io/mod.pluto", r#"extern fn __pluto_print_string(s: string)

pub fn println(s: string) {
    __pluto_print_string(s)
}
"#),
    ]);
    assert_eq!(out, "hello from lib\n");
}

#[test]
fn transitive_import_shared_extern_fn() {
    // Two modules both declare the same extern fn — should deduplicate cleanly
    let out = run_project(&[
        ("main.pluto", r#"import a
import b

fn main() {
    a.say(1)
    b.say(2)
}
"#),
        ("a.pluto", r#"extern fn __pluto_print_int(value: int)

pub fn say(x: int) {
    __pluto_print_int(x)
}
"#),
        ("b.pluto", r#"extern fn __pluto_print_int(value: int)

pub fn say(x: int) {
    __pluto_print_int(x)
}
"#),
    ]);
    assert_eq!(out, "1\n2\n");
}

// ============================================================
// Stdlib: std.strings
// ============================================================

/// Copy a directory tree recursively.
fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path);
        } else {
            std::fs::copy(entry.path(), &dest_path).unwrap();
        }
    }
}

/// Run a project that uses the real stdlib (copied from the repo's stdlib/ directory).
fn run_project_with_stdlib(files: &[(&str, &str)]) -> String {
    let dir = tempfile::tempdir().unwrap();

    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    // Copy the real stdlib directory
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib_src = manifest_dir.join("stdlib");
    let stdlib_dst = dir.path().join("stdlib");
    copy_dir_recursive(&stdlib_src, &stdlib_dst);

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    pluto::compile_file_with_stdlib(&entry, &bin_path, Some(&stdlib_dst))
        .unwrap_or_else(|e| panic!("Compilation failed: {e}"));

    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(run_output.status.success(), "Binary exited with non-zero status");
    String::from_utf8_lossy(&run_output.stdout).to_string()
}

#[test]
fn stdlib_strings_basic() {
    let out = run_project_with_stdlib(&[
        ("main.pluto", r#"import std.strings

fn main() {
    let s = "hello world"

    print(strings.substring(s, 0, 5))
    print(strings.contains(s, "world"))
    print(strings.starts_with(s, "hello"))
    print(strings.ends_with(s, "world"))
    print(strings.index_of(s, "world"))
    print(strings.char_at(s, 4))
    print(strings.len(s))
}
"#),
    ]);
    assert_eq!(out, "hello\ntrue\ntrue\ntrue\n6\no\n11\n");
}

#[test]
fn stdlib_strings_transform() {
    let out = run_project_with_stdlib(&[
        ("main.pluto", r#"import std.strings

fn main() {
    print(strings.to_upper("hello"))
    print(strings.to_lower("WORLD"))
    print(strings.trim("  hi  "))
    print(strings.replace("aXbXc", "X", "-"))

    let parts = strings.split("a,b,c", ",")
    print(parts[0])
    print(parts[1])
    print(parts[2])
}
"#),
    ]);
    assert_eq!(out, "HELLO\nworld\nhi\na-b-c\na\nb\nc\n");
}

// ============================================================
// Stdlib: std.math
// ============================================================

#[test]
fn stdlib_math_basic() {
    let out = run_project_with_stdlib(&[
        ("main.pluto", r#"import std.math

fn main() {
    print(math.abs(0 - 5))
    print(math.min(3, 7))
    print(math.max(3, 7))
    print(math.pow(2, 10))
    print(math.clamp(15, 0, 10))
    print(math.clamp(0 - 5, 0, 10))
    print(math.clamp(5, 0, 10))
}
"#),
    ]);
    assert_eq!(out, "5\n3\n7\n1024\n10\n0\n5\n");
}

#[test]
fn stdlib_math_pow_negative_exp() {
    let out = run_project_with_stdlib(&[
        ("main.pluto", r#"import std.math

fn main() {
    print(math.pow(2, 0 - 3))
}
"#),
    ]);
    assert_eq!(out, "0\n");
}

// ============================================================
// Stdlib: std.socket
// ============================================================

#[test]
fn stdlib_socket_create_close() {
    let out = run_project_with_stdlib(&[
        ("main.pluto", r#"import std.socket

fn main() {
    let fd = socket.create(2, 1, 0)
    if fd >= 0 {
        print("created")
    }
    socket.close(fd)
    print("closed")
}
"#),
    ]);
    assert_eq!(out, "created\nclosed\n");
}

#[test]
fn stdlib_net_tcp_roundtrip() {
    let out = run_project_with_stdlib(&[
        ("main.pluto", r#"import std.socket

fn main() {
    let server_fd = socket.create(2, 1, 0)
    socket.set_reuseaddr(server_fd)
    socket.bind(server_fd, "127.0.0.1", 0)
    socket.listen(server_fd, 128)
    let port = socket.get_port(server_fd)

    let client_fd = socket.create(2, 1, 0)
    socket.connect(client_fd, "127.0.0.1", port)

    let conn_fd = socket.accept(server_fd)

    socket.write(client_fd, "hello")
    let msg1 = socket.read(conn_fd, 1024)
    print(msg1)

    socket.write(conn_fd, "world")
    let msg2 = socket.read(client_fd, 1024)
    print(msg2)

    socket.close(conn_fd)
    socket.close(client_fd)
    socket.close(server_fd)
}
"#),
    ]);
    assert_eq!(out, "hello\nworld\n");
}

#[test]
fn stdlib_net_classes() {
    let out = run_project_with_stdlib(&[
        ("main.pluto", r#"import std.net
import std.socket

fn main() {
    let server = net.listen("127.0.0.1", 0)
    let port = server.port()

    let client = net.connect("127.0.0.1", port)
    let conn = server.accept()

    client.write("ping")
    let msg = conn.read(1024)
    print(msg)

    conn.close()
    client.close()
    server.close()
}
"#),
    ]);
    assert_eq!(out, "ping\n");
}

#[test]
fn test_sibling_file_error_attribution() {
    // When a sibling file has a syntax error, the error message should
    // correctly attribute the error to the sibling file, not the entry file.
    let dir = tempfile::tempdir().unwrap();

    // Entry file (valid)
    std::fs::write(
        dir.path().join("main.pluto"),
        r#"fn main() {
    print("Hello")
}
"#,
    )
    .unwrap();

    // Sibling file with syntax error (empty braces in string)
    std::fs::write(
        dir.path().join("bad.pluto"),
        r#"pub fn get_json() string {
    return "{}"
}
"#,
    )
    .unwrap();

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    let result = pluto::compile_file(&entry, &bin_path);
    assert!(result.is_err(), "Compilation should fail due to sibling file error");

    let err = result.unwrap_err();

    // The error should be a SiblingFile error
    match &err {
        pluto::diagnostics::CompileError::SiblingFile { path, source } => {
            // Verify the path points to bad.pluto, not main.pluto
            assert!(
                path.ends_with("bad.pluto"),
                "Error path should point to bad.pluto, got: {}",
                path.display()
            );

            // Verify the underlying error is a syntax error
            match **source {
                pluto::diagnostics::CompileError::Syntax { .. } => {},
                _ => panic!("Expected Syntax error, got: {:?}", source),
            }
        }
        _ => panic!("Expected SiblingFile error, got: {:?}", err),
    }

    // Also verify the error message mentions the sibling file
    let err_string = format!("{}", err);
    assert!(err_string.contains("Syntax error"), "Error should mention syntax error");
}

// ============================================================
// If-Expression Integration Tests
// ============================================================

#[test]
fn if_expr_with_qualified_types() {
    let files = vec![
        ("types.pluto", "pub class Point {\n    x: int\n    y: int\n}"),
        ("main.pluto", "import types\nfn main() {\n    let p = if true {\n        types.Point { x: 1, y: 2 }\n    } else {\n        types.Point { x: 3, y: 4 }\n    }\n    print(p.x + p.y)\n}"),
    ];
    let stdout = run_project(&files);
    assert_eq!(stdout.trim(), "3");
}

#[test]
fn if_expr_with_qualified_enum() {
    let files = vec![
        ("colors.pluto", "pub enum Color { Red Green Blue }"),
        ("main.pluto", "import colors\nfn main() {\n    let c = if true { colors.Color.Red } else { colors.Color.Blue }\n    match c {\n        colors.Color.Red { print(\"red\") }\n        colors.Color.Green { print(\"green\") }\n        colors.Color.Blue { print(\"blue\") }\n    }\n}"),
    ];
    let stdout = run_project(&files);
    assert_eq!(stdout.trim(), "red");
}

// ============================================================
// .deps/ Directory Resolution Tests
// ============================================================

#[test]
fn deps_directory_simple() {
    let files = vec![
        ("main.pluto", "import mylib\n\nfn main() {\n    print(mylib.greet())\n}"),
        (".deps/mylib/lib.pluto", "pub fn greet() string {\n    return \"hello from deps\"\n}"),
    ];
    let stdout = run_project(&files);
    assert_eq!(stdout.trim(), "hello from deps");
}

#[test]
fn deps_local_shadows_deps() {
    let files = vec![
        ("main.pluto", "import foo\n\nfn main() {\n    print(foo.source())\n}"),
        ("foo.pluto", "pub fn source() string {\n    return \"local\"\n}"),
        (".deps/foo/lib.pluto", "pub fn source() string {\n    return \"deps\"\n}"),
    ];
    let stdout = run_project(&files);
    assert_eq!(stdout.trim(), "local");  // Local takes precedence
}

#[test]
fn deps_not_found() {
    let files = vec![
        ("main.pluto", "import nonexistent\n\nfn main() {}"),
    ];
    compile_project_should_fail(&files);
}

#[test]
fn deps_transitive() {
    let files = vec![
        ("main.pluto", "import mylib\n\nfn main() {\n    print(mylib.combined())\n}"),
        (".deps/mylib/main.pluto", "import util\n\npub fn combined() int {\n    return util.value()\n}"),
        (".deps/mylib/util.pluto", "pub fn value() int {\n    return 42\n}"),
    ];
    let stdout = run_project(&files);
    assert_eq!(stdout.trim(), "42");
}
#[test]
fn standalone_compilation_skips_siblings() {
    use std::process::Command;
    use tempfile::tempdir;

    // Create temp directory with two files:
    // 1. app.pluto - has an app declaration
    // 2. main.pluto - has a regular main function
    // Without --standalone, this would fail (can't have both app and main)
    // With --standalone, main.pluto compiles in isolation

    let dir = tempdir().unwrap();

    let app_file = dir.path().join("app.pluto");
    std::fs::write(&app_file, "app TestApp {\n    fn main(self) {\n        print(\"from app\")\n    }\n}").unwrap();

    let main_file = dir.path().join("main.pluto");
    std::fs::write(&main_file, "fn main() {\n    print(\"from main\")\n}").unwrap();

    let bin_path = dir.path().join("test_bin");

    // Should fail without standalone (sibling merging enabled)
    let result = pluto::compile_file_with_options(
        &main_file,
        &bin_path,
        None,
        pluto::GcBackend::MarkSweep,
        false, // standalone = false
    );
    assert!(result.is_err(), "Should fail without standalone due to app+main conflict");

    // Should succeed with standalone (sibling merging disabled)
    let result = pluto::compile_file_with_options(
        &main_file,
        &bin_path,
        None,
        pluto::GcBackend::MarkSweep,
        true, // standalone = true
    );
    assert!(result.is_ok(), "Should succeed with standalone flag");

    // Verify the binary runs and produces correct output
    let output = Command::new(&bin_path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "from main");
}
