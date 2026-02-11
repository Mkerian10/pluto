use std::path::Path;
use std::process::Command;

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

fn run_with_stdlib(source: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let source_file = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&source_file, source).unwrap();

    // Copy the real stdlib directory
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib_src = manifest_dir.join("stdlib");
    let stdlib_dst = dir.path().join("stdlib");
    copy_dir_recursive(&stdlib_src, &stdlib_dst);

    plutoc::compile_file_with_stdlib(&source_file, &bin_path, Some(&stdlib_dst))
        .unwrap_or_else(|e| panic!("Compilation failed: {e}"));

    let output = Command::new(&bin_path).output().unwrap();
    assert!(output.status.success(), "Binary exited with non-zero status");
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn rpc_module_import() {
    let out = run_with_stdlib(
        "import std.rpc\n\nfn main() {\n    let result = rpc.http_post(\"http://localhost:8000/test\", \"body\")\n    print(result)\n}",
    );
    // Should get dummy response from __pluto_http_post stub
    assert_eq!(out, "{\"status\":\"ok\",\"result\":42}\n");
}

#[test]
fn rpc_http_post_with_timeout() {
    let out = run_with_stdlib(
        "import std.rpc\n\nfn main() {\n    let result = rpc.http_post_with_timeout(\"http://localhost:8000/test\", \"body\", 10000)\n    print(result)\n}",
    );
    assert_eq!(out, "{\"status\":\"ok\",\"result\":42}\n");
}
