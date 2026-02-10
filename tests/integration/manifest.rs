mod common;

use std::process::Command;

// ============================================================
// Test helpers
// ============================================================

/// Write project files + pluto.toml + dependency directories, compile and run.
/// `project_files`: files relative to project root (e.g., ("main.pluto", "..."))
/// `deps`: (dep_name, rel_path, files_in_dep) — auto-generates [dependencies] in pluto.toml
fn run_manifest_project(
    project_files: &[(&str, &str)],
    deps: &[(&str, &str, &[(&str, &str)])],
) -> String {
    let dir = tempfile::tempdir().unwrap();

    // Write pluto.toml
    let mut toml = String::from("[package]\nname = \"test-project\"\nversion = \"0.1.0\"\n\n[dependencies]\n");
    for (dep_name, rel_path, _) in deps {
        toml.push_str(&format!("{} = {{ path = \"{}\" }}\n", dep_name, rel_path));
    }
    std::fs::write(dir.path().join("pluto.toml"), &toml).unwrap();

    // Write project files
    for (name, content) in project_files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    // Write dependency files
    for (_, rel_path, dep_files) in deps {
        let dep_dir = dir.path().join(rel_path);
        std::fs::create_dir_all(&dep_dir).unwrap();
        for (name, content) in *dep_files {
            let path = dep_dir.join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, content).unwrap();
        }
    }

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    plutoc::compile_file(&entry, &bin_path)
        .unwrap_or_else(|e| panic!("Compilation failed: {e}"));

    let output = Command::new(&bin_path).output().unwrap();
    assert!(output.status.success(), "Binary exited with non-zero status. stderr: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Same but expect compilation failure. Returns the error message.
fn compile_manifest_should_fail(
    project_files: &[(&str, &str)],
    deps: &[(&str, &str, &[(&str, &str)])],
) -> String {
    let dir = tempfile::tempdir().unwrap();

    // Write pluto.toml
    let mut toml = String::from("[package]\nname = \"test-project\"\nversion = \"0.1.0\"\n\n[dependencies]\n");
    for (dep_name, rel_path, _) in deps {
        toml.push_str(&format!("{} = {{ path = \"{}\" }}\n", dep_name, rel_path));
    }
    std::fs::write(dir.path().join("pluto.toml"), &toml).unwrap();

    // Write project files
    for (name, content) in project_files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    // Write dependency files
    for (_, rel_path, dep_files) in deps {
        let dep_dir = dir.path().join(rel_path);
        std::fs::create_dir_all(&dep_dir).unwrap();
        for (name, content) in *dep_files {
            let path = dep_dir.join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, content).unwrap();
        }
    }

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    match plutoc::compile_file(&entry, &bin_path) {
        Err(e) => e.to_string(),
        Ok(()) => panic!("Compilation should have failed"),
    }
}

/// Compile with a raw pluto.toml string (for testing malformed manifests).
fn compile_with_raw_toml(
    toml_content: &str,
    project_files: &[(&str, &str)],
) -> String {
    let dir = tempfile::tempdir().unwrap();

    std::fs::write(dir.path().join("pluto.toml"), toml_content).unwrap();

    for (name, content) in project_files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    match plutoc::compile_file(&entry, &bin_path) {
        Err(e) => e.to_string(),
        Ok(()) => panic!("Compilation should have failed"),
    }
}

// ============================================================
// Happy path tests
// ============================================================

#[test]
fn basic_path_dep() {
    let out = run_manifest_project(
        &[("main.pluto", "import mylib\n\nfn main() {\n    print(mylib.add(1, 2))\n}")],
        &[("mylib", "deps/mylib", &[
            ("add.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
        ])],
    );
    assert_eq!(out, "3\n");
}

#[test]
fn multiple_deps() {
    let out = run_manifest_project(
        &[("main.pluto", "import mylib\nimport utils\n\nfn main() {\n    print(mylib.add(1, 2))\n    print(utils.greet())\n}")],
        &[
            ("mylib", "deps/mylib", &[
                ("add.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
            ]),
            ("utils", "deps/utils", &[
                ("greet.pluto", "pub fn greet() string {\n    return \"hello\"\n}"),
            ]),
        ],
    );
    assert_eq!(out, "3\nhello\n");
}

#[test]
fn dep_with_classes() {
    let out = run_manifest_project(
        &[("main.pluto", "import mylib\n\nfn main() {\n    let p = mylib.Point { x: 10, y: 20 }\n    print(p.x + p.y)\n}")],
        &[("mylib", "deps/mylib", &[
            ("point.pluto", "pub class Point {\n    x: int\n    y: int\n}"),
        ])],
    );
    assert_eq!(out, "30\n");
}

#[test]
fn dep_with_enums() {
    let out = run_manifest_project(
        &[("main.pluto", "import mylib\n\nfn main() {\n    let c = mylib.Color.Red\n    match c {\n        mylib.Color.Red {\n            print(\"red\")\n        }\n        mylib.Color.Green {\n            print(\"green\")\n        }\n        mylib.Color.Blue {\n            print(\"blue\")\n        }\n    }\n}")],
        &[("mylib", "deps/mylib", &[
            ("color.pluto", "pub enum Color {\n    Red\n    Green\n    Blue\n}"),
        ])],
    );
    assert_eq!(out, "red\n");
}

#[test]
fn dep_with_internal_imports() {
    // Dep has its own submodules that it uses internally
    let out = run_manifest_project(
        &[("main.pluto", "import mylib\n\nfn main() {\n    print(mylib.compute(3, 4))\n}")],
        &[("mylib", "deps/mylib", &[
            ("compute.pluto", "import helpers\n\npub fn compute(a: int, b: int) int {\n    return helpers.add(a, b)\n}"),
            ("helpers/add.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
        ])],
    );
    assert_eq!(out, "7\n");
}

#[test]
fn transitive_package_deps() {
    // Root depends on A, A depends on B (via its own pluto.toml)
    let dir = tempfile::tempdir().unwrap();

    // Root pluto.toml
    std::fs::write(dir.path().join("pluto.toml"),
        "[package]\nname = \"root\"\n\n[dependencies]\nliba = { path = \"deps/liba\" }\n").unwrap();
    std::fs::write(dir.path().join("main.pluto"),
        "import liba\n\nfn main() {\n    print(liba.compute(5))\n}").unwrap();

    // Dep A with its own pluto.toml
    let liba_dir = dir.path().join("deps/liba");
    std::fs::create_dir_all(&liba_dir).unwrap();
    std::fs::write(liba_dir.join("pluto.toml"),
        "[package]\nname = \"liba\"\n\n[dependencies]\nlibb = { path = \"../../deps/libb\" }\n").unwrap();
    std::fs::write(liba_dir.join("compute.pluto"),
        "import libb\n\npub fn compute(x: int) int {\n    return libb.double(x)\n}").unwrap();

    // Dep B (leaf, no pluto.toml needed)
    let libb_dir = dir.path().join("deps/libb");
    std::fs::create_dir_all(&libb_dir).unwrap();
    std::fs::write(libb_dir.join("double.pluto"),
        "pub fn double(x: int) int {\n    return x * 2\n}").unwrap();

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    plutoc::compile_file(&entry, &bin_path).unwrap_or_else(|e| panic!("Compilation failed: {e}"));

    let output = Command::new(&bin_path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "10\n");
}

#[test]
fn multi_segment_through_dep() {
    // `import mylib.sub` where mylib is a dep directory
    let out = run_manifest_project(
        &[("main.pluto", "import mylib.sub\n\nfn main() {\n    print(sub.greet())\n}")],
        &[("mylib", "deps/mylib", &[
            ("sub/greet.pluto", "pub fn greet() string {\n    return \"from sub\"\n}"),
        ])],
    );
    assert_eq!(out, "from sub\n");
}

#[test]
fn no_manifest_backward_compat() {
    // No pluto.toml — everything works as before
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.pluto"),
        "import math\n\nfn main() {\n    print(math.add(1, 2))\n}").unwrap();
    let math_dir = dir.path().join("math");
    std::fs::create_dir_all(&math_dir).unwrap();
    std::fs::write(math_dir.join("add.pluto"),
        "pub fn add(a: int, b: int) int {\n    return a + b\n}").unwrap();

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    plutoc::compile_file(&entry, &bin_path).unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    let output = Command::new(&bin_path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "3\n");
}

#[test]
fn empty_deps_section() {
    let out = run_manifest_project(
        &[("main.pluto", "fn main() {\n    print(42)\n}")],
        &[],
    );
    assert_eq!(out, "42\n");
}

#[test]
fn dep_with_closures_and_generics() {
    let out = run_manifest_project(
        &[("main.pluto", r#"import mylib

fn main() {
    let result = mylib.apply((x: int) => x * 2, 21)
    print(result)
}"#)],
        &[("mylib", "deps/mylib", &[
            ("apply.pluto", "pub fn apply(f: fn(int) int, x: int) int {\n    return f(x)\n}"),
        ])],
    );
    assert_eq!(out, "42\n");
}

#[test]
fn manifest_in_parent_dir() {
    // pluto.toml in parent, entry file in subdirectory
    let dir = tempfile::tempdir().unwrap();

    std::fs::write(dir.path().join("pluto.toml"),
        "[package]\nname = \"myapp\"\n\n[dependencies]\nmylib = { path = \"deps/mylib\" }\n").unwrap();
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("main.pluto"),
        "import mylib\n\nfn main() {\n    print(mylib.value())\n}").unwrap();

    let dep_dir = dir.path().join("deps/mylib");
    std::fs::create_dir_all(&dep_dir).unwrap();
    std::fs::write(dep_dir.join("value.pluto"),
        "pub fn value() int {\n    return 99\n}").unwrap();

    let entry = src_dir.join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    plutoc::compile_file(&entry, &bin_path).unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    let output = Command::new(&bin_path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "99\n");
}

#[test]
fn same_dep_imported_in_two_branches() {
    // Diamond: B and C both depend on shared D, no false cycle
    let dir = tempfile::tempdir().unwrap();

    // Root
    std::fs::write(dir.path().join("pluto.toml"),
        "[package]\nname = \"root\"\n\n[dependencies]\nlibb = { path = \"deps/libb\" }\nlibc = { path = \"deps/libc\" }\n").unwrap();
    std::fs::write(dir.path().join("main.pluto"),
        "import libb\nimport libc\n\nfn main() {\n    print(libb.get_b())\n    print(libc.get_c())\n}").unwrap();

    // Dep B depends on D
    let libb_dir = dir.path().join("deps/libb");
    std::fs::create_dir_all(&libb_dir).unwrap();
    std::fs::write(libb_dir.join("pluto.toml"),
        "[package]\nname = \"libb\"\n\n[dependencies]\nlibd = { path = \"../../deps/libd\" }\n").unwrap();
    std::fs::write(libb_dir.join("get_b.pluto"),
        "import libd\n\npub fn get_b() int {\n    return libd.shared_val() + 1\n}").unwrap();

    // Dep C depends on D
    let libc_dir = dir.path().join("deps/libc");
    std::fs::create_dir_all(&libc_dir).unwrap();
    std::fs::write(libc_dir.join("pluto.toml"),
        "[package]\nname = \"libc\"\n\n[dependencies]\nlibd = { path = \"../../deps/libd\" }\n").unwrap();
    std::fs::write(libc_dir.join("get_c.pluto"),
        "import libd\n\npub fn get_c() int {\n    return libd.shared_val() + 2\n}").unwrap();

    // Dep D (shared leaf)
    let libd_dir = dir.path().join("deps/libd");
    std::fs::create_dir_all(&libd_dir).unwrap();
    std::fs::write(libd_dir.join("shared_val.pluto"),
        "pub fn shared_val() int {\n    return 100\n}").unwrap();

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    plutoc::compile_file(&entry, &bin_path).unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    let output = Command::new(&bin_path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "101\n102\n");
}

// ============================================================
// Error cases
// ============================================================

#[test]
fn transitive_dep_not_visible_to_root() {
    // Root depends on A, A depends on B. Root tries to import B directly — should fail.
    let dir = tempfile::tempdir().unwrap();

    std::fs::write(dir.path().join("pluto.toml"),
        "[package]\nname = \"root\"\n\n[dependencies]\nliba = { path = \"deps/liba\" }\n").unwrap();
    std::fs::write(dir.path().join("main.pluto"),
        "import libb\n\nfn main() {\n    print(libb.val())\n}").unwrap();

    let liba_dir = dir.path().join("deps/liba");
    std::fs::create_dir_all(&liba_dir).unwrap();
    std::fs::write(liba_dir.join("pluto.toml"),
        "[package]\nname = \"liba\"\n\n[dependencies]\nlibb = { path = \"../../deps/libb\" }\n").unwrap();
    std::fs::write(liba_dir.join("mod.pluto"),
        "import libb\n\npub fn compute() int {\n    return libb.val()\n}").unwrap();

    let libb_dir = dir.path().join("deps/libb");
    std::fs::create_dir_all(&libb_dir).unwrap();
    std::fs::write(libb_dir.join("val.pluto"),
        "pub fn val() int {\n    return 42\n}").unwrap();

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    assert!(plutoc::compile_file(&entry, &bin_path).is_err(), "Should fail: libb not in root's deps");
}

#[test]
fn dep_local_collision_single_segment() {
    // Dep name matches local dir, should be hard error
    let err = compile_manifest_should_fail(
        &[
            ("main.pluto", "import mylib\n\nfn main() {\n    print(mylib.add(1, 2))\n}"),
            ("mylib/add.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
        ],
        &[("mylib", "deps/mylib", &[
            ("add.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
        ])],
    );
    assert!(err.contains("ambiguous"), "Expected ambiguity error, got: {}", err);
}

#[test]
fn dep_local_collision_multi_segment() {
    // `import foo.bar` where foo is both dep and local dir
    let err = compile_manifest_should_fail(
        &[
            ("main.pluto", "import mylib.sub\n\nfn main() {\n    print(sub.add(1, 2))\n}"),
            ("mylib/sub/add.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
        ],
        &[("mylib", "deps/mylib", &[
            ("sub/add.pluto", "pub fn add(a: int, b: int) int {\n    return a + b\n}"),
        ])],
    );
    assert!(err.contains("ambiguous"), "Expected ambiguity error, got: {}", err);
}

#[test]
fn dep_named_std_rejected() {
    let err = compile_with_raw_toml(
        "[package]\nname = \"test\"\n\n[dependencies]\nstd = { path = \"./stdlib\" }\n",
        &[("main.pluto", "fn main() {\n    print(1)\n}")],
    );
    assert!(err.contains("'std' is reserved"), "Expected 'std' reserved error, got: {}", err);
}

#[test]
fn invalid_dep_name_keyword() {
    let dir = tempfile::tempdir().unwrap();
    let dep_dir = dir.path().join("myclass");
    std::fs::create_dir_all(&dep_dir).unwrap();
    std::fs::write(dep_dir.join("mod.pluto"), "pub fn foo() int { return 1 }").unwrap();

    std::fs::write(dir.path().join("pluto.toml"),
        "[package]\nname = \"test\"\n\n[dependencies]\nclass = { path = \"myclass\" }\n").unwrap();
    std::fs::write(dir.path().join("main.pluto"), "fn main() {\n    print(1)\n}").unwrap();

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    let err = plutoc::compile_file(&entry, &bin_path).unwrap_err().to_string();
    assert!(err.contains("reserved keyword"), "Expected keyword error, got: {}", err);
}

#[test]
fn invalid_dep_name_format() {
    let err = compile_with_raw_toml(
        "[package]\nname = \"test\"\n\n[dependencies]\n\"123bad\" = { path = \"./dep\" }\n",
        &[("main.pluto", "fn main() {\n    print(1)\n}"), ("dep/mod.pluto", "pub fn x() int { return 1 }")],
    );
    assert!(err.contains("not a valid identifier"), "Expected invalid identifier error, got: {}", err);
}

#[test]
fn missing_dep_path() {
    let err = compile_with_raw_toml(
        "[package]\nname = \"test\"\n\n[dependencies]\nmylib = { path = \"./nonexistent\" }\n",
        &[("main.pluto", "fn main() {\n    print(1)\n}")],
    );
    assert!(err.contains("does not exist"), "Expected missing path error, got: {}", err);
}

#[test]
fn malformed_toml() {
    let err = compile_with_raw_toml(
        "this is not valid toml {{{",
        &[("main.pluto", "fn main() {\n    print(1)\n}")],
    );
    assert!(err.contains("invalid syntax"), "Expected TOML syntax error, got: {}", err);
}

#[test]
fn missing_package_section() {
    let err = compile_with_raw_toml(
        "[dependencies]\nmylib = { path = \"./dep\" }\n",
        &[("main.pluto", "fn main() {\n    print(1)\n}")],
    );
    assert!(err.contains("missing [package] section"), "Expected missing package error, got: {}", err);
}

#[test]
fn missing_package_name() {
    let err = compile_with_raw_toml(
        "[package]\nversion = \"0.1.0\"\n",
        &[("main.pluto", "fn main() {\n    print(1)\n}")],
    );
    assert!(err.contains("missing 'name' in [package]"), "Expected missing name error, got: {}", err);
}

#[test]
fn empty_package_name() {
    let err = compile_with_raw_toml(
        "[package]\nname = \"   \"\n",
        &[("main.pluto", "fn main() {\n    print(1)\n}")],
    );
    assert!(err.contains("package name must not be empty"), "Expected empty name error, got: {}", err);
}

#[test]
fn circular_dep_chain() {
    let dir = tempfile::tempdir().unwrap();

    // Root depends on A
    std::fs::write(dir.path().join("pluto.toml"),
        "[package]\nname = \"root\"\n\n[dependencies]\nliba = { path = \"deps/liba\" }\n").unwrap();
    std::fs::write(dir.path().join("main.pluto"), "fn main() {\n    print(1)\n}").unwrap();

    // A depends on B
    let liba_dir = dir.path().join("deps/liba");
    std::fs::create_dir_all(&liba_dir).unwrap();
    std::fs::write(liba_dir.join("pluto.toml"),
        "[package]\nname = \"liba\"\n\n[dependencies]\nlibb = { path = \"../../deps/libb\" }\n").unwrap();
    std::fs::write(liba_dir.join("mod.pluto"), "pub fn a() int { return 1 }").unwrap();

    // B depends on A (circular!)
    let libb_dir = dir.path().join("deps/libb");
    std::fs::create_dir_all(&libb_dir).unwrap();
    std::fs::write(libb_dir.join("pluto.toml"),
        "[package]\nname = \"libb\"\n\n[dependencies]\nliba = { path = \"../../deps/liba\" }\n").unwrap();
    std::fs::write(libb_dir.join("mod.pluto"), "pub fn b() int { return 2 }").unwrap();

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    let err = plutoc::compile_file(&entry, &bin_path).unwrap_err().to_string();
    assert!(err.contains("circular package dependency"), "Expected circular dep error, got: {}", err);
}

#[test]
fn dep_with_extern_rust_rejected() {
    let err = compile_manifest_should_fail(
        &[("main.pluto", "import mylib\n\nfn main() {\n    print(mylib.foo())\n}")],
        &[("mylib", "deps/mylib", &[
            ("mod.pluto", "extern rust \"fakelib\" as fakelib\n\npub fn foo() int {\n    return 1\n}"),
        ])],
    );
    assert!(err.contains("extern rust") && err.contains("package dependenc"),
        "Expected extern rust in dep error, got: {}", err);
}

#[test]
fn dep_without_manifest_no_parent_inheritance() {
    // Root has pluto.toml with dep A. Dep A has no pluto.toml.
    // A tries to import something from root's dep scope — should fail (no inheritance).
    let dir = tempfile::tempdir().unwrap();

    // Root with two deps
    std::fs::write(dir.path().join("pluto.toml"),
        "[package]\nname = \"root\"\n\n[dependencies]\nliba = { path = \"deps/liba\" }\nlibb = { path = \"deps/libb\" }\n").unwrap();
    std::fs::write(dir.path().join("main.pluto"),
        "import liba\nimport libb\n\nfn main() {\n    print(liba.get_a())\n    print(libb.get_b())\n}").unwrap();

    // Dep A (no pluto.toml) tries to use libb — should fail
    let liba_dir = dir.path().join("deps/liba");
    std::fs::create_dir_all(&liba_dir).unwrap();
    std::fs::write(liba_dir.join("get_a.pluto"),
        "import libb\n\npub fn get_a() int {\n    return libb.get_b()\n}").unwrap();

    // Dep B (leaf)
    let libb_dir = dir.path().join("deps/libb");
    std::fs::create_dir_all(&libb_dir).unwrap();
    std::fs::write(libb_dir.join("get_b.pluto"),
        "pub fn get_b() int {\n    return 42\n}").unwrap();

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    assert!(plutoc::compile_file(&entry, &bin_path).is_err(),
        "Should fail: liba can't import libb without its own pluto.toml declaring it");
}

// ============================================================
// Policy tests
// ============================================================

#[test]
fn exact_duplicate_import_allowed() {
    // Two sibling files in a dir module both `import` the same module — should be deduplicated silently
    let dir = tempfile::tempdir().unwrap();

    std::fs::write(dir.path().join("pluto.toml"),
        "[package]\nname = \"root\"\n\n[dependencies]\nmylib = { path = \"deps/mylib\" }\n").unwrap();
    std::fs::write(dir.path().join("main.pluto"),
        "import mymod\n\nfn main() {\n    print(mymod.a())\n    print(mymod.b())\n}").unwrap();

    let mymod_dir = dir.path().join("mymod");
    std::fs::create_dir_all(&mymod_dir).unwrap();
    std::fs::write(mymod_dir.join("a.pluto"),
        "import mylib\n\npub fn a() int {\n    return mylib.val()\n}").unwrap();
    std::fs::write(mymod_dir.join("b.pluto"),
        "import mylib\n\npub fn b() int {\n    return mylib.val() + 1\n}").unwrap();

    let dep_dir = dir.path().join("deps/mylib");
    std::fs::create_dir_all(&dep_dir).unwrap();
    std::fs::write(dep_dir.join("val.pluto"),
        "pub fn val() int {\n    return 10\n}").unwrap();

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    plutoc::compile_file(&entry, &bin_path).unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    let output = Command::new(&bin_path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "10\n11\n");
}

// ============================================================
// Resolver edge cases
// ============================================================

#[test]
fn multi_segment_dep_with_local_file_no_collision() {
    // `import foo.bar` with dep foo + local `foo.pluto` (NOT ambiguous — .pluto file isn't a valid multi-segment base)
    let out = run_manifest_project(
        &[
            ("main.pluto", "import mylib.sub\n\nfn main() {\n    print(sub.val())\n}"),
            ("mylib.pluto", "pub fn local_fn() int {\n    return 0\n}"),
        ],
        &[("mylib", "deps/mylib", &[
            ("sub/val.pluto", "pub fn val() int {\n    return 77\n}"),
        ])],
    );
    assert_eq!(out, "77\n");
}

#[test]
fn same_dep_via_different_relative_paths() {
    // Two deps point to same dir via different relative paths — should work (canonical dedup)
    let dir = tempfile::tempdir().unwrap();

    let shared_dir = dir.path().join("shared");
    std::fs::create_dir_all(&shared_dir).unwrap();
    std::fs::write(shared_dir.join("val.pluto"),
        "pub fn val() int {\n    return 42\n}").unwrap();

    std::fs::write(dir.path().join("pluto.toml"),
        "[package]\nname = \"root\"\n\n[dependencies]\nalias_a = { path = \"shared\" }\nalias_b = { path = \"./shared\" }\n").unwrap();
    std::fs::write(dir.path().join("main.pluto"),
        "import alias_a\nimport alias_b\n\nfn main() {\n    print(alias_a.val())\n    print(alias_b.val())\n}").unwrap();

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    plutoc::compile_file(&entry, &bin_path).unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    let output = Command::new(&bin_path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n42\n");
}

#[test]
fn parent_path_dep() {
    // Dep path using ../ resolves correctly
    let dir = tempfile::tempdir().unwrap();

    // Project in subdir
    let project_dir = dir.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();
    std::fs::write(project_dir.join("pluto.toml"),
        "[package]\nname = \"myapp\"\n\n[dependencies]\nmylib = { path = \"../mylib\" }\n").unwrap();
    std::fs::write(project_dir.join("main.pluto"),
        "import mylib\n\nfn main() {\n    print(mylib.val())\n}").unwrap();

    // Dep in sibling dir
    let dep_dir = dir.path().join("mylib");
    std::fs::create_dir_all(&dep_dir).unwrap();
    std::fs::write(dep_dir.join("val.pluto"),
        "pub fn val() int {\n    return 55\n}").unwrap();

    let entry = project_dir.join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    plutoc::compile_file(&entry, &bin_path).unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    let output = Command::new(&bin_path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "55\n");
}

#[test]
fn git_file_boundary_manifest_walk() {
    // .git as a file (worktree/submodule) should still stop manifest walk
    let dir = tempfile::tempdir().unwrap();

    // Create a .git file (not directory) in a subdirectory
    let sub = dir.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join(".git"), "gitdir: /some/path").unwrap();

    // Put pluto.toml above the .git file — should NOT be found by manifest walk from sub/
    std::fs::write(dir.path().join("pluto.toml"),
        "[package]\nname = \"root\"\n\n[dependencies]\nmylib = { path = \"mylib\" }\n").unwrap();
    let dep_dir = dir.path().join("mylib");
    std::fs::create_dir_all(&dep_dir).unwrap();
    std::fs::write(dep_dir.join("val.pluto"),
        "pub fn val() int { return 1 }").unwrap();

    std::fs::write(sub.join("main.pluto"),
        "fn main() {\n    print(99)\n}").unwrap();

    let entry = sub.join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    // Should compile fine (no manifest found, no deps) — backward compat
    plutoc::compile_file(&entry, &bin_path).unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    let output = Command::new(&bin_path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "99\n");
}

#[test]
fn two_aliases_same_canonical_path() {
    // Root declares math and m both pointing to same dir
    let dir = tempfile::tempdir().unwrap();

    let shared = dir.path().join("shared");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(shared.join("val.pluto"),
        "pub fn val() int {\n    return 7\n}").unwrap();

    std::fs::write(dir.path().join("pluto.toml"),
        "[package]\nname = \"root\"\n\n[dependencies]\nmath = { path = \"shared\" }\nm = { path = \"shared\" }\n").unwrap();
    std::fs::write(dir.path().join("main.pluto"),
        "import math\nimport m\n\nfn main() {\n    print(math.val())\n    print(m.val())\n}").unwrap();

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    plutoc::compile_file(&entry, &bin_path).unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    let output = Command::new(&bin_path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "7\n7\n");
}
