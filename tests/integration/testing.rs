mod common;
use common::{compile_test_and_run, compile_test_should_fail_with, compile_should_fail_with, compile_and_run_stdout};

// ── Basic test execution ──────────────────────────────────────────────────────

#[test]
fn test_basic_passing() {
    let (stdout, _, code) = compile_test_and_run(r#"
test "one equals one" {
    expect(1).to_equal(1)
}
"#);
    assert_eq!(code, 0);
    assert!(stdout.contains("test one equals one ... ok"));
    assert!(stdout.contains("1 tests passed"));
}

#[test]
fn test_multiple_tests() {
    let (stdout, _, code) = compile_test_and_run(r#"
test "first" {
    expect(1).to_equal(1)
}

test "second" {
    expect(true).to_be_true()
}

test "third" {
    expect(false).to_be_false()
}
"#);
    assert_eq!(code, 0);
    assert!(stdout.contains("test first ... ok"));
    assert!(stdout.contains("test second ... ok"));
    assert!(stdout.contains("test third ... ok"));
    assert!(stdout.contains("3 tests passed"));
}

#[test]
fn test_with_helper_functions() {
    let (stdout, _, code) = compile_test_and_run(r#"
fn add(a: int, b: int) int {
    return a + b
}

test "addition works" {
    expect(add(1, 2)).to_equal(3)
    expect(add(-1, 1)).to_equal(0)
}
"#);
    assert_eq!(code, 0);
    assert!(stdout.contains("test addition works ... ok"));
    assert!(stdout.contains("1 tests passed"));
}

// ── Assertion methods ─────────────────────────────────────────────────────────

#[test]
fn test_to_equal_int() {
    let (_, _, code) = compile_test_and_run(r#"
test "int equality" {
    expect(42).to_equal(42)
    expect(-1).to_equal(-1)
    expect(0).to_equal(0)
}
"#);
    assert_eq!(code, 0);
}

#[test]
fn test_to_equal_float() {
    let (_, _, code) = compile_test_and_run(r#"
test "float equality" {
    expect(3.14).to_equal(3.14)
    expect(0.0).to_equal(0.0)
}
"#);
    assert_eq!(code, 0);
}

#[test]
fn test_to_equal_bool() {
    let (_, _, code) = compile_test_and_run(r#"
test "bool equality" {
    expect(true).to_equal(true)
    expect(false).to_equal(false)
}
"#);
    assert_eq!(code, 0);
}

#[test]
fn test_to_equal_string() {
    let (_, _, code) = compile_test_and_run(r#"
test "string equality" {
    expect("hello").to_equal("hello")
    expect("").to_equal("")
}
"#);
    assert_eq!(code, 0);
}

#[test]
fn test_to_be_true() {
    let (_, _, code) = compile_test_and_run(r#"
test "true check" {
    expect(true).to_be_true()
    expect(1 > 0).to_be_true()
    expect(1 == 1).to_be_true()
}
"#);
    assert_eq!(code, 0);
}

#[test]
fn test_to_be_false() {
    let (_, _, code) = compile_test_and_run(r#"
test "false check" {
    expect(false).to_be_false()
    expect(1 > 2).to_be_false()
    expect(1 == 2).to_be_false()
}
"#);
    assert_eq!(code, 0);
}

// ── Failing assertions ────────────────────────────────────────────────────────

#[test]
fn test_failing_int_equality() {
    let (_, stderr, code) = compile_test_and_run(r#"
test "will fail" {
    expect(1).to_equal(2)
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("FAIL"));
    assert!(stderr.contains("expected 1 to equal 2"));
}

#[test]
fn test_failing_to_be_true() {
    let (_, stderr, code) = compile_test_and_run(r#"
test "will fail" {
    expect(false).to_be_true()
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("FAIL"));
    assert!(stderr.contains("expected true but got false"));
}

#[test]
fn test_failing_to_be_false() {
    let (_, stderr, code) = compile_test_and_run(r#"
test "will fail" {
    expect(true).to_be_false()
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("FAIL"));
    assert!(stderr.contains("expected false but got true"));
}

#[test]
fn test_failing_string_equality() {
    let (_, stderr, code) = compile_test_and_run(r#"
test "will fail" {
    expect("hello").to_equal("world")
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("FAIL"));
    assert!(stderr.contains("hello"));
    assert!(stderr.contains("world"));
}

// ── Compile errors ────────────────────────────────────────────────────────────

#[test]
fn test_type_mismatch_to_equal() {
    compile_test_should_fail_with(r#"
test "bad" {
    expect(1).to_equal("hello")
}
"#, "to_equal");
}

#[test]
fn test_to_be_true_non_bool() {
    compile_test_should_fail_with(r#"
test "bad" {
    expect(1).to_be_true()
}
"#, "requires bool");
}

#[test]
fn test_to_be_false_non_bool() {
    compile_test_should_fail_with(r#"
test "bad" {
    expect(1).to_be_false()
}
"#, "requires bool");
}

#[test]
fn test_unknown_assertion_method() {
    compile_test_should_fail_with(r#"
test "bad" {
    expect(1).to_be_awesome()
}
"#, "unknown assertion method");
}

#[test]
fn test_duplicate_test_names() {
    compile_test_should_fail_with(r#"
test "same name" {
    expect(1).to_equal(1)
}

test "same name" {
    expect(2).to_equal(2)
}
"#, "duplicate test name");
}

#[test]
fn test_pub_test_rejected() {
    compile_test_should_fail_with(r#"
pub test "bad" {
    expect(1).to_equal(1)
}
"#, "tests cannot be pub");
}

#[test]
fn test_bare_expect_rejected() {
    compile_test_should_fail_with(r#"
test "bad" {
    expect(1)
}
"#, "expect() must be followed by an assertion method");
}

#[test]
fn test_expect_builtin_shadowing_rejected() {
    compile_should_fail_with(r#"
fn expect(x: int) int {
    return x
}

fn main() int {
    return expect(5)
}
"#, "expect");
}

// ── Non-test mode stripping ───────────────────────────────────────────────────

#[test]
fn test_strip_tests_in_non_test_mode() {
    // Tests should be stripped in normal compilation mode.
    // This program has a test block but also a valid main function.
    let stdout = compile_and_run_stdout(r#"
fn main() {
    print(42)
}

test "this should be stripped" {
    expect(1).to_equal(2)
}
"#);
    assert_eq!(stdout.trim(), "42");
}

// ── Empty test body ───────────────────────────────────────────────────────────

#[test]
fn test_empty_body_passes() {
    let (stdout, _, code) = compile_test_and_run(r#"
test "empty" {
}
"#);
    assert_eq!(code, 0);
    assert!(stdout.contains("test empty ... ok"));
    assert!(stdout.contains("1 tests passed"));
}

// ── Line numbers in failure messages ──────────────────────────────────────────

#[test]
fn test_line_numbers_in_failure() {
    let (_, stderr, code) = compile_test_and_run(r#"
test "line check" {
    expect(1).to_equal(1)
    expect(2).to_equal(3)
}
"#);
    assert_ne!(code, 0);
    // The failing assertion is on line 4
    assert!(stderr.contains("line 4"), "Expected 'line 4' in stderr: {}", stderr);
}

// ── Declaration order ─────────────────────────────────────────────────────────

#[test]
fn test_declaration_order() {
    let (stdout, _, code) = compile_test_and_run(r#"
test "alpha" {
    expect(true).to_be_true()
}

test "beta" {
    expect(true).to_be_true()
}

test "gamma" {
    expect(true).to_be_true()
}
"#);
    assert_eq!(code, 0);
    // Tests run in declaration order
    let alpha_pos = stdout.find("test alpha").unwrap();
    let beta_pos = stdout.find("test beta").unwrap();
    let gamma_pos = stdout.find("test gamma").unwrap();
    assert!(alpha_pos < beta_pos);
    assert!(beta_pos < gamma_pos);
}

// ── Multiple files with tests ─────────────────────────────────────────────────

#[test]
fn test_multiple_files_unique_test_ids() {
    // Regression test for P1 #3: Test Runner Generates Duplicate IDs for Multiple Files
    // When multiple .pluto files in the same directory have test blocks, each test should
    // get a unique ID based on the file path hash to avoid duplicate symbol errors at link time.
    use std::process::Command;

    let dir = tempfile::tempdir().unwrap();

    // Create two sibling files, each with test blocks
    std::fs::write(
        dir.path().join("file_a.pluto"),
        r#"
test "test in file a" {
    expect(1).to_equal(1)
}

test "another test in file a" {
    expect(true).to_be_true()
}
"#,
    ).unwrap();

    std::fs::write(
        dir.path().join("file_b.pluto"),
        r#"
test "test in file b" {
    expect(2).to_equal(2)
}

test "another test in file b" {
    expect(false).to_be_false()
}
"#,
    ).unwrap();

    let entry_a = dir.path().join("file_a.pluto");
    let entry_b = dir.path().join("file_b.pluto");
    let bin_path_a = dir.path().join("test_bin_a");
    let bin_path_b = dir.path().join("test_bin_b");

    // Compile file_a in test mode - siblings should NOT be auto-merged to prevent test ID collisions
    pluto::compile_file_for_tests(&entry_a, &bin_path_a, None, false)
        .unwrap_or_else(|e| panic!("Test compilation of file_a failed: {e}"));

    // Compile file_b separately
    pluto::compile_file_for_tests(&entry_b, &bin_path_b, None, false)
        .unwrap_or_else(|e| panic!("Test compilation of file_b failed: {e}"));

    // Run file_a tests - should only see file_a's 2 tests
    let output_a = Command::new(&bin_path_a).output().unwrap();
    let stdout_a = String::from_utf8_lossy(&output_a.stdout);
    let stderr_a = String::from_utf8_lossy(&output_a.stderr);

    assert!(output_a.status.success(), "file_a tests should pass. stderr: {}", stderr_a);
    assert!(stdout_a.contains("test in file a ... ok"), "stdout: {}", stdout_a);
    assert!(stdout_a.contains("another test in file a ... ok"), "stdout: {}", stdout_a);
    assert!(stdout_a.contains("2 tests passed"), "stdout: {}", stdout_a);
    // Should NOT contain file_b tests
    assert!(!stdout_a.contains("test in file b"), "file_a should not include file_b tests");

    // Run file_b tests - should only see file_b's 2 tests
    let output_b = Command::new(&bin_path_b).output().unwrap();
    let stdout_b = String::from_utf8_lossy(&output_b.stdout);
    let stderr_b = String::from_utf8_lossy(&output_b.stderr);

    assert!(output_b.status.success(), "file_b tests should pass. stderr: {}", stderr_b);
    assert!(stdout_b.contains("test in file b ... ok"), "stdout: {}", stdout_b);
    assert!(stdout_b.contains("another test in file b ... ok"), "stdout: {}", stdout_b);
    assert!(stdout_b.contains("2 tests passed"), "stdout: {}", stdout_b);
    // Should NOT contain file_a tests
    assert!(!stdout_b.contains("test in file a"), "file_b should not include file_a tests");
}

// ── Test-local DI containers (scope blocks in test bodies) ───────────────

#[test]
fn test_scope_singleton_override() {
    // Seeding a singleton inside a test overrides what the graph wires
    let (stdout, _, code) = compile_test_and_run(r#"
class Database {
    tag: string

    fn query(self) string {
        return self.tag
    }
}

class Service[db: Database] {
    fn run(self) string {
        return self.db.query()
    }
}

test "service with fake db" {
    scope(Database { tag: "fake" }) |svc: Service| {
        expect(svc.run()).to_equal("fake")
    }
}
"#);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("1 tests passed"));
}

#[test]
fn test_scope_deep_singleton_chain() {
    // Unseeded auto-constructible singletons are created scope-locally
    let (stdout, _, code) = compile_test_and_run(r#"
class Level3 {
    v: int
}

class Level2[l3: Level3] {
    fn get(self) int {
        return self.l3.v
    }
}

class Level1[l2: Level2] {
    fn get(self) int {
        return self.l2.get()
    }
}

test "deep chain" {
    scope(Level3 { v: 42 }) |top: Level1| {
        expect(top.get()).to_equal(42)
    }
}
"#);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("1 tests passed"));
}

#[test]
fn test_scope_containers_isolated() {
    // Two scope blocks in one test are independent containers
    let (stdout, _, code) = compile_test_and_run(r#"
class Db {
    tag: string

    fn t(self) string {
        return self.tag
    }
}

class Svc[db: Db] {
    fn run(self) string {
        return self.db.t()
    }
}

test "two containers isolated" {
    scope(Db { tag: "first" }) |s: Svc| {
        expect(s.run()).to_equal("first")
    }
    scope(Db { tag: "second" }) |s: Svc| {
        expect(s.run()).to_equal("second")
    }
}
"#);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("1 tests passed"));
}

#[test]
fn test_scope_mixed_lifecycles() {
    // Scoped and singleton seeds coexist in one test container
    let (stdout, _, code) = compile_test_and_run(r#"
class Db {
    tag: string
}

scoped class Ctx {
    id: int
}

scoped class Handler[ctx: Ctx, db: Db] {
    fn dbtag(self) string {
        return self.db.tag
    }

    fn cid(self) int {
        return self.ctx.id
    }
}

test "mixed lifecycles" {
    scope(Ctx { id: 9 }, Db { tag: "test-db" }) |h: Handler| {
        expect(h.dbtag()).to_equal("test-db")
        expect(h.cid()).to_equal(9)
    }
}
"#);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("1 tests passed"));
}

#[test]
fn test_scope_stateful_singleton_needs_seed() {
    // A stateful singleton reached by a test container must be seeded
    compile_test_should_fail_with(r#"
class Config {
    port: int
}

class Service[cfg: Config] {
    fn p(self) int {
        return self.cfg.port
    }
}

scoped class W {
    z: int
}

test "missing seed" {
    scope(W { z: 1 }) |svc: Service| {
        expect(svc.p()).to_equal(1)
    }
}
"#, "singleton class 'Config' has non-injected fields and must be provided as a seed");
}

#[test]
fn test_scope_singleton_seed_rejected_outside_tests() {
    // The relaxation is test-only: run-mode scope blocks still require
    // scoped classes, with a hint pointing at test bodies
    compile_should_fail_with(r#"
class Database {
    tag: string
}

fn main(){
    scope(Database { tag: "x" }) |d: Database| {
    }
}
"#, "seed it inside a test body for a test-local override");
}

// ── expect_raises ─────────────────────────────────────────────────────────────

#[test]
fn test_expect_raises_typed_passing() {
    let (stdout, _, code) = compile_test_and_run(r#"
error ParseError {
    input: string
}

fn parse(s: string) int {
    if s == "bad" {
        raise ParseError { input: s }
    }
    return 42
}

test "bad input raises ParseError" {
    expect_raises(ParseError) {
        parse("bad")!
    }
    expect(1).to_equal(1)
}
"#);
    assert_eq!(code, 0);
    assert!(stdout.contains("test bad input raises ParseError ... ok"));
    assert!(stdout.contains("1 tests passed"));
}

#[test]
fn test_expect_raises_fails_when_no_raise() {
    let (_, stderr, code) = compile_test_and_run(r#"
error ParseError {
    input: string
}

fn parse(s: string) int {
    if s == "bad" {
        raise ParseError { input: s }
    }
    return 42
}

test "no raise fails" {
    expect_raises(ParseError) {
        let v = parse("good")!
        expect(v).to_equal(42)
    }
}
"#);
    assert_ne!(code, 0);
    assert!(
        stderr.contains("expected ParseError to be raised, but no error was raised"),
        "stderr: {stderr}"
    );
}

#[test]
fn test_expect_raises_fails_on_wrong_error_type() {
    let (_, stderr, code) = compile_test_and_run(r#"
error ParseError {
    input: string
}

error SegmentError {
    offset: int
}

fn fickle(which: bool) int {
    if which {
        raise ParseError { input: "x" }
    }
    raise SegmentError { offset: 1 }
}

test "wrong type fails" {
    expect_raises(ParseError) {
        fickle(false)!
    }
}
"#);
    assert_ne!(code, 0);
    assert!(
        stderr.contains("expected ParseError to be raised, got SegmentError"),
        "stderr: {stderr}"
    );
    // The wrong error is consumed by the construct, not re-propagated.
    assert!(
        !stderr.contains("unhandled error escaped main"),
        "stderr: {stderr}"
    );
}

#[test]
fn test_expect_raises_wildcard_passing() {
    let (stdout, _, code) = compile_test_and_run(r#"
error SegmentError {
    offset: int
}

fn explode() int {
    raise SegmentError { offset: 7 }
}

test "wildcard catches any raise" {
    expect_raises {
        explode()!
    }
}
"#);
    assert_eq!(code, 0);
    assert!(stdout.contains("test wildcard catches any raise ... ok"));
}

#[test]
fn test_expect_raises_wildcard_fails_when_no_raise() {
    let (_, stderr, code) = compile_test_and_run(r#"
error SegmentError {
    offset: int
}

fn explode(go: bool) int {
    if go {
        raise SegmentError { offset: 7 }
    }
    return 0
}

test "wildcard no raise" {
    expect_raises {
        explode(false)!
    }
}
"#);
    assert_ne!(code, 0);
    assert!(
        stderr.contains("expected an error to be raised, but no error was raised"),
        "stderr: {stderr}"
    );
}

#[test]
fn test_expect_raises_propagation_in_infallible_test() {
    // `!` inside the block is the propagation route to the construct: the
    // enclosing test stays infallible, and statements after the construct
    // still run.
    let (stdout, _, code) = compile_test_and_run(r#"
error ParseError {
    input: string
}

fn parse(s: string) int {
    if s == "bad" {
        raise ParseError { input: s }
    }
    return 42
}

test "bang propagates to the construct" {
    expect_raises(ParseError) {
        let v = parse("good")!
        expect(v).to_equal(42)
        parse("bad")!
    }
    expect(parse("good") catch 0).to_equal(42)
}
"#);
    assert_eq!(code, 0);
    assert!(stdout.contains("test bang propagates to the construct ... ok"));
}

#[test]
fn test_expect_raises_direct_raise_in_block() {
    let (stdout, _, code) = compile_test_and_run(r#"
error ParseError {
    input: string
}

test "direct raise" {
    expect_raises(ParseError) {
        raise ParseError { input: "direct" }
    }
}
"#);
    assert_eq!(code, 0);
    assert!(stdout.contains("test direct raise ... ok"));
}

#[test]
fn test_expect_raises_nested_in_loop() {
    let (stdout, _, code) = compile_test_and_run(r#"
error SegmentError {
    offset: int
}

fn check(off: int) int {
    if off > 10 {
        raise SegmentError { offset: off }
    }
    return off
}

test "raises assertion inside a loop" {
    for i in 0..3 {
        expect_raises(SegmentError) {
            check(11 + i)!
        }
    }
    expect(check(5) catch 0).to_equal(5)
}
"#);
    assert_eq!(code, 0);
    assert!(stdout.contains("test raises assertion inside a loop ... ok"));
}

#[test]
fn test_expect_raises_block_cannot_raise_is_compile_error() {
    compile_test_should_fail_with(r#"
error ParseError {
    input: string
}

test "cannot raise" {
    expect_raises(ParseError) {
        let x = 1 + 1
        expect(x).to_equal(2)
    }
}
"#, "expect_raises block cannot raise");
}

#[test]
fn test_expect_raises_type_not_in_error_set_is_compile_error() {
    compile_test_should_fail_with(r#"
error ParseError {
    input: string
}

error SegmentError {
    offset: int
}

error IoError {
    path: string
}

fn fickle(which: bool) int {
    if which {
        raise ParseError { input: "x" }
    }
    raise SegmentError { offset: 1 }
}

test "not in set" {
    expect_raises(IoError) {
        fickle(true)!
    }
}
"#, "the block can raise 'ParseError', 'SegmentError' — not 'IoError'");
}

#[test]
fn test_expect_raises_unknown_error_type_is_compile_error() {
    compile_test_should_fail_with(r#"
error ParseError {
    input: string
}

fn explode() int {
    raise ParseError { input: "x" }
}

test "unknown type" {
    expect_raises(NoSuchError) {
        explode()!
    }
}
"#, "unknown error type 'NoSuchError'");
}

#[test]
fn test_expect_raises_fully_caught_block_cannot_raise() {
    // A `catch` inside the block consumes the error locally, so nothing can
    // reach the construct — rejected like any other can't-raise block.
    compile_test_should_fail_with(r#"
error ParseError {
    input: string
}

fn parse(s: string) int {
    if s == "bad" {
        raise ParseError { input: s }
    }
    return 42
}

test "caught inside" {
    expect_raises(ParseError) {
        let v = parse("bad") catch 0
        expect(v).to_equal(0)
    }
}
"#, "expect_raises block cannot raise");
}

#[test]
fn test_expect_raises_bare_fallible_call_still_needs_handling() {
    // The construct is the handler for `!`, not a license for bare fallible
    // calls — the usual handling rule still applies inside the block.
    compile_test_should_fail_with(r#"
error ParseError {
    input: string
}

fn explode() int {
    raise ParseError { input: "x" }
}

test "bare call" {
    expect_raises(ParseError) {
        explode()
    }
}
"#, "must be handled with ! or catch");
}
