//! Integration tests for `pluto analyze` command.

use tempfile::TempDir;

mod common;

#[test]
fn test_analyze_pt_file_creates_pluto() {
    // Test: pluto analyze creates .pluto file from .pt source
    //
    // Verifies that analyzing a .pt text file produces a .pluto binary
    // with fresh derived data.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("test.pt");
    let pluto_file = temp.path().join("test.pluto");

    std::fs::write(
        &pt_file,
        r#"
fn add(x: int, y: int) int {
    return x + y
}

fn main() {
    let result = add(10, 20)
}
"#,
    )
    .unwrap();

    // Run analyze
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .status()
        .unwrap();

    assert!(status.success(), "analyze command failed");
    assert!(pluto_file.exists(), ".pluto file not created");

    // Verify the .pluto file is valid binary format
    let data = std::fs::read(&pluto_file).unwrap();
    assert!(pluto::binary::is_binary_format(&data), ".pluto is not valid binary");

    // Verify we can deserialize it and it has derived data
    let (_program, _source, derived) = pluto::binary::deserialize_program(&data).unwrap();
    assert!(!derived.source_hash.is_empty(), "source_hash not computed");
}

#[test]
fn test_analyze_computes_function_metadata() {
    // Test: analyze computes function signatures and error sets
    //
    // Verifies that the derived data includes function metadata.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("funcs.pt");

    std::fs::write(
        &pt_file,
        r#"fn add(x: int, y: int) int {
    return x + y
}

fn main() {}
"#,
    )
    .unwrap();

    // Run analyze
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .status()
        .unwrap();

    assert!(status.success());

    // Load and check derived data
    let pluto_file = temp.path().join("funcs.pluto");
    let data = std::fs::read(&pluto_file).unwrap();
    let (program, _source, derived) = pluto::binary::deserialize_program(&data).unwrap();

    // Find the add function's UUID
    let add_fn = program
        .functions
        .iter()
        .find(|f| f.node.name.node == "add")
        .expect("add function not found");

    // Check function signature
    let sig = derived
        .fn_signatures
        .get(&add_fn.node.id)
        .expect("signature not found for add");

    assert_eq!(sig.param_types.len(), 2, "should have 2 parameters");
    assert!(!sig.is_fallible, "add should not be fallible");

    // Check error set (should be empty for non-fallible function)
    let error_set = derived
        .fn_error_sets
        .get(&add_fn.node.id)
        .expect("error set not found for add");

    assert!(error_set.is_empty(), "add should have no errors");
}

#[test]
fn test_analyze_computes_function_signatures() {
    // Test: analyze computes resolved function signatures
    //
    // Verifies that derived data includes param and return types.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("sigs.pt");

    std::fs::write(
        &pt_file,
        r#"
fn multiply(x: int, y: int) int {
    return x * y
}

fn main() {}
"#,
    )
    .unwrap();

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .status()
        .unwrap();

    assert!(status.success());

    let pluto_file = temp.path().join("sigs.pluto");
    let data = std::fs::read(&pluto_file).unwrap();
    let (program, _source, derived) = pluto::binary::deserialize_program(&data).unwrap();

    let multiply_fn = program
        .functions
        .iter()
        .find(|f| f.node.name.node == "multiply")
        .unwrap();

    let sig = derived
        .fn_signatures
        .get(&multiply_fn.node.id)
        .expect("signature not found");

    assert_eq!(sig.param_types.len(), 2);
    assert!(!sig.is_fallible);
}

#[test]
fn test_analyze_staleness_detection() {
    // Test: derived data includes source hash for staleness detection
    //
    // Verifies that the source_hash field is populated and changes
    // when the source changes.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("stale.pt");

    std::fs::write(
        &pt_file,
        r#"
fn version_one() int {
    return 1
}

fn main() {}
"#,
    )
    .unwrap();

    // First analyze
    std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .status()
        .unwrap();

    let pluto_file = temp.path().join("stale.pluto");
    let data1 = std::fs::read(&pluto_file).unwrap();
    let (_prog1, source1, derived1) = pluto::binary::deserialize_program(&data1).unwrap();

    let hash1 = derived1.source_hash.clone();
    assert!(!hash1.is_empty(), "hash should be set");

    // Verify not stale
    assert!(!derived1.is_stale(&source1), "should not be stale");

    // Modify source
    std::fs::write(
        &pt_file,
        r#"
fn version_two() int {
    return 2
}

fn main() {}
"#,
    )
    .unwrap();

    // Re-analyze
    std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .status()
        .unwrap();

    let data2 = std::fs::read(&pluto_file).unwrap();
    let (_prog2, source2, derived2) = pluto::binary::deserialize_program(&data2).unwrap();

    let hash2 = derived2.source_hash.clone();

    // Hash should be different
    assert_ne!(hash1, hash2, "hash should change when source changes");

    // Old derived data should be stale against new source
    assert!(derived1.is_stale(&source2), "old data should be stale");

    // New derived data should not be stale
    assert!(!derived2.is_stale(&source2), "new data should not be stale");
}

#[test]
fn test_analyze_preserves_ast_and_source() {
    // Test: analyze preserves the original AST and source text
    //
    // Only the derived layer should change, not the authored content.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("preserve.pt");

    let source_text = r#"
fn original_name(param: int) int {
    return param * 2
}

fn main() {}
"#;

    std::fs::write(&pt_file, source_text).unwrap();

    std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .status()
        .unwrap();

    let pluto_file = temp.path().join("preserve.pluto");
    let data = std::fs::read(&pluto_file).unwrap();
    let (program, source, _derived) = pluto::binary::deserialize_program(&data).unwrap();

    // Source should be exactly what we wrote (modulo leading/trailing whitespace)
    assert!(source.contains("original_name"));
    assert!(source.contains("param * 2"));

    // AST function name should match
    let func = program.functions.iter().find(|f| f.node.name.node == "original_name");
    assert!(func.is_some(), "function name preserved in AST");
}

#[test]
fn test_analyze_invalid_syntax() {
    // Test: analyze reports parse errors gracefully
    //
    // Should exit with error, not crash or create invalid .pluto file.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("invalid.pt");

    std::fs::write(
        &pt_file,
        r#"
fn broken(x: int  // Missing closing paren and brace
"#,
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();

    assert!(!output.status.success(), "should fail on invalid syntax");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error:"), "should report an error");

    // .pluto file should not be created
    let pluto_file = temp.path().join("invalid.pluto");
    assert!(!pluto_file.exists(), "should not create .pluto on parse error");
}

#[test]
fn test_analyze_output_message() {
    // Test: analyze command prints success message
    //
    // Should show input → output paths.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("msg.pt");

    std::fs::write(
        &pt_file,
        r#"
fn main() {}
"#,
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("analyzed"));
    assert!(stderr.contains("msg.pt"));
    assert!(stderr.contains("msg.pluto"));
}

#[test]
fn test_analyze_multi_file_modules() {
    // Test: analyze handles multi-file projects with imports
    //
    // Verifies that module resolution works correctly.

    let temp = TempDir::new().unwrap();

    // Create a math module (must be .pluto extension for module system)
    let math_file = temp.path().join("math.pluto");
    std::fs::write(
        &math_file,
        r#"pub fn add(x: int, y: int) int {
    return x + y
}

pub fn multiply(x: int, y: int) int {
    return x * y
}
"#,
    )
    .unwrap();

    // Create entry file that imports math
    let main_file = temp.path().join("main.pt");
    std::fs::write(
        &main_file,
        r#"
import math

fn main() {
    let sum = math.add(10, 20)
    let product = math.multiply(sum, 2)
}
"#,
    )
    .unwrap();

    // Run analyze on entry file
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&main_file)
        .arg("--stdlib")
        .arg("stdlib")
        .status()
        .unwrap();

    assert!(status.success(), "analyze should succeed on multi-file project");

    // Load and check derived data
    let pluto_file = temp.path().join("main.pluto");
    assert!(pluto_file.exists(), ".pluto file should be created");

    let data = std::fs::read(&pluto_file).unwrap();
    let (program, _source, derived) = pluto::binary::deserialize_program(&data).unwrap();

    // Verify both local and imported functions are in the flattened AST
    let has_main = program.functions.iter().any(|f| f.node.name.node == "main");
    let has_math_add = program.functions.iter().any(|f| f.node.name.node == "math.add");
    let has_math_multiply = program.functions.iter().any(|f| f.node.name.node == "math.multiply");

    assert!(has_main, "main function should be in AST");
    assert!(has_math_add, "math.add function should be in flattened AST");
    assert!(has_math_multiply, "math.multiply function should be in flattened AST");

    // Verify derived data includes all functions
    assert_eq!(derived.fn_signatures.len(), 3, "should have signatures for all 3 functions");
}

#[test]
fn test_analyze_prints_assumption_surface() {
    // `pluto analyze` reports the assumption surface (rfc-properties.md
    // phase 5 / epistemics.md): every claim discharged by ASSUMPTION, with
    // owner, property, instantiation, and line. Pinned output shape.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("assume.pt");
    std::fs::write(
        &pt_file,
        "property idempotent(key: expr) {\n}\n\nextern fn ext_put(k: string, v: string) int assume idempotent(key = k)\n\nfn main() {}\n",
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();
    assert!(output.status.success(), "analyze command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("assumption surface: 1 assumed claim"),
        "stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("assume idempotent(key = k) — extern fn ext_put (line 4)"),
        "stdout:\n{stdout}"
    );
}

#[test]
fn test_analyze_prints_empty_assumption_surface() {
    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("clean.pt");
    std::fs::write(&pt_file, "fn main() {\n    print(1)\n}\n").unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();
    assert!(output.status.success(), "analyze command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("assumption surface: empty (no assumed claims)"),
        "stdout:\n{stdout}"
    );
}

#[test]
fn test_analyze_prints_checked_claims_and_requires_counter() {
    // `pluto analyze` reports CHECKED claims (rfc-properties.md phase 5.5:
    // proven guard placement, runtime guard data) alongside the assumption
    // surface, and the count of call sites whose `requires` clauses were
    // proven statically. Pinned output shape.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("checked.pt");
    std::fs::write(
        &pt_file,
        "property idempotent(key: expr) {\n    dedup key\n}\n\nobject P {\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, k: string) int provides idempotent(key = k) {\n        if self.seen.contains(k) { return self.total }\n        self.seen.insert(k)\n        self.total = self.total + 1\n        return self.total\n    }\n}\n\nfn bump(n: int) int\n    requires n > 0\n{\n    return n + 1\n}\n\nfn main() {\n    print(bump(3))\n}\n",
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();
    assert!(output.status.success(), "analyze command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("checked claims: 1 checked claim"),
        "stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("checked idempotent(key = k) — method P.apply (line 9)"),
        "stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("proven requires sites: 1"),
        "stdout:\n{stdout}"
    );
}

#[test]
fn test_analyze_reports_arith_check_residue() {
    // #416 phase 2: `pluto analyze` reports the runtime residue of trapping
    // arithmetic — how many int +/-/* overflow checks the interval proofs
    // elided and how many remain. The guard-bounded loop counter elides;
    // the unbounded accumulation stays checked; the wrapping_mul call is
    // deliberate modular arithmetic and is neither counted nor checked.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("residue.pt");
    std::fs::write(
        &pt_file,
        "fn main() {\n    let mut i = 0\n    let mut sum = 0\n    while i < 256 {\n        i = i + 1\n    }\n    let big = 9223372036854775807\n    sum = sum + big\n    let h = wrapping_mul(big, 1099511628211)\n    print(i)\n    print(sum)\n    print(h)\n}\n",
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();
    assert!(output.status.success(), "analyze command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("arithmetic overflow checks: 1 elided, 1 checked"),
        "stdout:\n{stdout}"
    );
}

#[test]
fn test_analyze_arith_residue_kill_respects_facts() {
    // Soundness pin: elision happens on a proof only. The first increment
    // is proven by the loop guard; the reassignment kills the guard fact,
    // so the following multiply is unprovable and stays checked.
    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("killres.pt");
    std::fs::write(
        &pt_file,
        "fn main() {\n    let mut i = 0\n    while i < 256 {\n        i = i + 1\n        i = i * 2\n    }\n    print(i)\n}\n",
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();
    assert!(output.status.success(), "analyze command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("arithmetic overflow checks: 1 elided, 1 checked"),
        "stdout:\n{stdout}"
    );
}

#[test]
fn test_analyze_arith_residue_variable_bound_counter_elides() {
    // The strict relation `i < n` alone bounds i <= int.max - 1 (n is an
    // i64 value itself), so the canonical variable-bounded counter elides
    // even though n has no interval.
    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("varbound.pt");
    std::fs::write(
        &pt_file,
        "fn work(n: int) int {\n    let mut i = 0\n    while i < n {\n        i = i + 1\n    }\n    return i\n}\n\nfn main() {\n    print(work(1000))\n}\n",
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();
    assert!(output.status.success(), "analyze command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("arithmetic overflow checks: 1 elided, 0 checked"),
        "stdout:\n{stdout}"
    );
}

#[test]
fn test_analyze_reports_shift_check_residue() {
    // #441: shift amounts outside 0..63 trap. A guard proving the amount in
    // range elides the check; an unbounded amount stays checked; a constant
    // amount is typeck-decided and never counted.
    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("shiftres.pt");
    std::fs::write(
        &pt_file,
        "fn amount() int {\n    return 5\n}\n\nfn main() {\n    let k = amount()\n    let mut v = 0\n    if k >= 0 && k < 64 {\n        v = 1 << k\n    }\n    let w = 1 << k\n    let c = 1 << 63\n    print(v)\n    print(w)\n    print(c)\n}\n",
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();
    assert!(output.status.success(), "analyze command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("shift range checks: 1 elided, 1 checked"),
        "stdout:\n{stdout}"
    );
}

#[test]
fn test_analyze_assert_elides_shift_and_overflow_checks() {
    // `assert` is a flow-fact source: the asserted range bounds `k`, so
    // the shift-amount check AND the multiply's overflow check elide. The
    // facts are exactly a terminating guard's — no new surface, no
    // heuristics.
    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("assertres.pt");
    std::fs::write(
        &pt_file,
        "fn amount() int {\n    return 70\n}\n\nfn main() {\n    let k = amount()\n    assert k >= 0 && k < 64\n    let v = 1 << k\n    let s = k * k\n    print(v)\n    print(s)\n}\n",
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();
    assert!(output.status.success(), "analyze command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("shift range checks: 1 elided, 0 checked"),
        "stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("arithmetic overflow checks: 1 elided, 0 checked"),
        "stdout:\n{stdout}"
    );
}

#[test]
fn test_analyze_assert_with_call_elides_nothing() {
    // Soundness pin: an assert whose condition contains a call contributes
    // NO facts (the call could mutate state between evaluation and use),
    // so the shift check stays — even though one conjunct is pure.
    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("assertcall.pt");
    std::fs::write(
        &pt_file,
        "fn amount() int {\n    return 70\n}\n\nfn limit() int {\n    return 64\n}\n\nfn main() {\n    let k = amount()\n    assert k >= 0 && k < limit()\n    let v = 1 << k\n    print(v)\n}\n",
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();
    assert!(output.status.success(), "analyze command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("shift range checks: 0 elided, 1 checked"),
        "stdout:\n{stdout}"
    );
}

#[test]
fn test_analyze_mask_arithmetic_elides() {
    // Interval rule `x & c ∈ [0, c]` (constant c >= 0): the two-byte pack
    // is provably in range at both the multiply and the add.
    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("maskres.pt");
    std::fs::write(
        &pt_file,
        "fn pack(x: int, y: int) int {\n    return (x & 255) * 256 + (y & 255)\n}\n\nfn main() {\n    print(pack(300, 7))\n}\n",
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();
    assert!(output.status.success(), "analyze command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("arithmetic overflow checks: 2 elided, 0 checked"),
        "stdout:\n{stdout}"
    );
}

#[test]
fn test_analyze_mask_and_mod_rules_are_exact() {
    // No false facts: a negative mask (`x & -1 == x`) and a non-constant
    // divisor contribute nothing and stay checked; `x % 10 ∈ [-9, 9]`
    // (truncated remainder — the sign follows the dividend) proves the
    // scaled multiply. Residue: 1 elided (the `(x % 10) * 100000000`),
    // 4 checked (both `*` on unknown values and both `+`).
    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("maskmod.pt");
    std::fs::write(
        &pt_file,
        "fn f(x: int, y: int) int {\n    let a = (x & -1) * 2\n    let b = (x % 10) * 100000000\n    let c = (x % y) * 2\n    return a + b + c\n}\n\nfn main() {\n    print(f(300, 7))\n}\n",
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();
    assert!(output.status.success(), "analyze command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("arithmetic overflow checks: 1 elided, 4 checked"),
        "stdout:\n{stdout}"
    );
}

#[test]
fn test_analyze_bytes_read_loop_elides() {
    // Byte-typed values carry [0, 255] by construction — a type fact, not
    // a flow fact, so it survives loop havoc. All three sites elide: the
    // widened byte read times 256, the add of the masked accumulator, and
    // the counter increment (strict relation to `data.len()`).
    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("bytesloop.pt");
    std::fs::write(
        &pt_file,
        "fn decode(data: bytes) int {\n    let mut acc = 0\n    let mut i = 0\n    while i < data.len() {\n        acc = (data[i].to_int()) * 256 + (acc & 255)\n        i = i + 1\n    }\n    return acc\n}\n\nfn main() {\n    let mut buf = bytes_new()\n    buf.push((7).to_byte())\n    buf.push((9).to_byte())\n    print(decode(buf))\n}\n",
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();
    assert!(output.status.success(), "analyze command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("arithmetic overflow checks: 3 elided, 0 checked"),
        "stdout:\n{stdout}"
    );
}

#[test]
fn test_analyze_prints_perception_surface() {
    // `pluto analyze` reports the perception surface (rfc-module-semantics.md
    // section 4): every boundary-crossing type whose invariants are
    // re-checked at decode. Pinned output shape, including the empty form.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("percep.pt");
    std::fs::write(
        &pt_file,
        "import std.wire\n\nclass Account {\n    balance: int\n    invariant self.balance >= 0\n}\n\nstage Api {\n    pub fn get(self) Account {\n        return Account { balance: 1 }\n    }\n\n    fn main(self) {\n    }\n}\n",
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();
    assert!(output.status.success(), "analyze command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("perception surface: 1 boundary type validated at decode"),
        "expected perception surface line, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Account — self.balance >= 0"),
        "expected the Account entry, got:\n{stdout}"
    );

    // A program with no boundary types reports the empty form.
    let plain = temp.path().join("plain.pt");
    std::fs::write(&plain, "fn main() {\n    print(1)\n}\n").unwrap();
    let output2 = std::process::Command::new(env!("CARGO_BIN_EXE_pluto"))
        .arg("analyze")
        .arg(&plain)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(
        stdout2.contains("perception surface: empty"),
        "expected empty perception surface, got:\n{stdout2}"
    );
}
