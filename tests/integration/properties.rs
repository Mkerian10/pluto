//! The property form (docs/design/rfc-properties.md slice 2, phase 4):
//! `property` declarations, `satisfies` instantiation by substitution,
//! discharge through the standard invariant/dominance machinery, and
//! two-sided blame diagnostics.

mod common;

use common::{compile_and_run_stdout, compile_should_fail_with};
use std::process::Command;

/// Assert compilation fails and the error message contains EVERY expected
/// substring — used for two-sided blame, which must name both the property
/// body and the failing site in one diagnostic.
fn compile_should_fail_with_all(source: &str, expected: &[&str]) {
    match pluto::compile_to_object(source) {
        Ok(_) => panic!("Compilation should have failed"),
        Err(e) => {
            let msg = e.to_string();
            for part in expected {
                assert!(
                    msg.contains(part),
                    "error message missing '{part}'.\nFull message:\n{msg}"
                );
            }
        }
    }
}

/// Write multiple files to a temp directory, compile the entry via library
/// call, and return stdout (mirrors tests/integration/modules.rs).
fn run_project(files: &[(&str, &str)]) -> String {
    let dir = tempfile::tempdir().unwrap();
    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }
    let entry = dir.path().join("main.pt");
    let bin_path = dir.path().join("test_bin");
    pluto::compile_file(&entry, &bin_path)
        .unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(run_output.status.success(), "Binary exited with non-zero status");
    String::from_utf8_lossy(&run_output.stdout).to_string()
}

/// Like run_project but asserts compilation fails with every substring.
fn compile_project_should_fail_with_all(files: &[(&str, &str)], expected: &[&str]) {
    let dir = tempfile::tempdir().unwrap();
    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }
    let entry = dir.path().join("main.pt");
    let bin_path = dir.path().join("test_bin");
    match pluto::compile_file(&entry, &bin_path) {
        Ok(_) => panic!("Compilation should have failed"),
        Err(e) => {
            let msg = e.to_string();
            for part in expected {
                assert!(
                    msg.contains(part),
                    "error message missing '{part}'.\nFull message:\n{msg}"
                );
            }
        }
    }
}

// ============================================================
// Declaration: parse and pretty round-trip
// ============================================================

#[test]
fn property_declaration_parses() {
    let src = "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nfn main() {\n    print(1)\n}\n";
    let program = pluto::parse_source(src).expect("parses");
    assert_eq!(program.properties.len(), 1);
    let prop = &program.properties[0].node;
    assert_eq!(prop.name.node, "monotonic");
    assert_eq!(prop.params.len(), 1);
    assert_eq!(prop.atoms.len(), 1);
    assert_eq!(prop.atoms[0].node.line, 2);
}

#[test]
fn property_declaration_pretty_round_trip() {
    let src = "pub property fenced(f: field, authority: field<int>, grant: type) {\n    f guarded_by (g: grant) g.token == authority\n}\n";
    let program = pluto::parse_source(src).expect("parses");
    let printed = pluto::pretty::pretty_print(&program, false);
    assert!(
        printed.contains("pub property fenced(f: field, authority: field<int>, grant: type) {"),
        "printed:\n{printed}"
    );
    assert!(
        printed.contains("f guarded_by (g: grant) g.token == authority"),
        "printed:\n{printed}"
    );
    // The printed form parses back to the same declaration shape.
    let reparsed = pluto::parse_source(&printed).expect("round-trips");
    assert_eq!(reparsed.properties.len(), 1);
    assert_eq!(reparsed.properties[0].node.params.len(), 3);
}

#[test]
fn satisfies_clause_pretty_round_trip() {
    let src = "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nclass C satisfies monotonic(self.n) {\n    n: int\n}\n";
    let program = pluto::parse_source(src).expect("parses");
    let printed = pluto::pretty::pretty_print(&program, false);
    assert!(
        printed.contains("class C satisfies monotonic(self.n) {"),
        "printed:\n{printed}"
    );
    let reparsed = pluto::parse_source(&printed).expect("round-trips");
    let class = reparsed.classes.iter().find(|c| c.node.name.node == "C").unwrap();
    assert_eq!(class.node.satisfies.len(), 1);
}

// ============================================================
// Declaration and instantiation errors
// ============================================================

#[test]
fn unknown_property_rejected() {
    compile_should_fail_with(
        "class C satisfies nope(self.n) {\n    n: int\n}\n\nfn main() {}\n",
        "unknown property 'nope'",
    );
}

#[test]
fn arity_mismatch_rejected() {
    compile_should_fail_with(
        "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nclass C satisfies monotonic(self.a, self.b) {\n    a: int\n    b: int\n}\n\nfn main() {}\n",
        "expects 1 argument (f: field<int>), found 2",
    );
}

#[test]
fn field_kind_mismatch_rejected() {
    compile_should_fail_with(
        "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nclass C satisfies monotonic(self.name) {\n    name: string\n}\n\nfn main() {}\n",
        "requires a field of type int, but field 'name' of 'C' has type string",
    );
}

#[test]
fn missing_field_rejected() {
    compile_should_fail_with(
        "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nclass C satisfies monotonic(self.ghost) {\n    n: int\n}\n\nfn main() {}\n",
        "'C' has no field 'ghost'",
    );
}

#[test]
fn const_arg_must_be_literal() {
    compile_should_fail_with(
        "property bounded(f: field<int>, lo: const int) {\n    invariant f >= lo\n}\n\nclass C satisfies bounded(self.n, self.n) {\n    n: int\n}\n\nfn main() {}\n",
        "must be an integer literal",
    );
}

#[test]
fn unknown_name_in_body_rejected() {
    compile_should_fail_with(
        "property bad(f: field<int>) {\n    invariant g >= 0\n}\n\nfn main() {}\n",
        "unknown name 'g'",
    );
}

#[test]
fn self_reference_in_body_rejected() {
    compile_should_fail_with(
        "property bad(f: field<int>) {\n    invariant self.x >= 0\n}\n\nfn main() {}\n",
        "property bodies are parametric",
    );
}

#[test]
fn non_int_field_param_in_invariant_atom_rejected() {
    compile_should_fail_with(
        "property bad(f: field) {\n    invariant f >= 0\n}\n\nfn main() {}\n",
        "only 'field<int>' and 'const int' parameters are terms",
    );
}

#[test]
fn empty_property_body_rejected() {
    compile_should_fail_with(
        "property empty(f: field<int>) {\n}\n\nfn main() {}\n",
        "empty body",
    );
}

#[test]
fn property_on_generic_class_param_independent_accepted() {
    // `satisfies` on a generic class works when the substituted atoms pass
    // the param-independence validation: the injected clause is an ordinary
    // (template-proven) generic invariant.
    let out = compile_and_run_stdout(
        r#"
property monotonic(f: field<int>) {
    invariant f >= old(f)
}

object Topic<T> satisfies monotonic(self.published) {
    latest: T?
    published: int

    fn publish(mut self, msg: T) {
        self.latest = msg
        self.published = self.published + 1
    }
}

fn main() {
    let mut t = Topic<int> { latest: none, published: 0 }
    t.publish(5)
    print(t.published)
}
"#,
    );
    assert_eq!(out.trim(), "1");
}

#[test]
fn property_on_generic_class_violating_template_blamed() {
    // Two-sided blame survives the generic path: the failure names both the
    // failing template method and the property instantiation.
    compile_should_fail_with_all(
        "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nobject Topic<T> satisfies monotonic(self.published) {\n    published: int\n\n    fn rewind(mut self) {\n        self.published = self.published - 1\n    }\n}\n\nfn main() {}\n",
        &[
            "invariant 'self.published >= old(self.published)' of class 'Topic<T>' is violated",
            "in method 'rewind'",
            "required by property 'monotonic'",
            "instantiated with f = self.published",
        ],
    );
}

#[test]
fn property_on_generic_class_param_typed_field_rejected() {
    // Param-dependence carried by the kind check: `field<int>` requires the
    // field's declared type to be literally `int`, which a param-typed field
    // is not.
    compile_should_fail_with(
        "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nclass Box<T> satisfies monotonic(self.v) {\n    n: int\n    v: T\n}\n\nfn main() {}\n",
        "requires a field of type int",
    );
}

#[test]
fn duplicate_guard_on_field_rejected() {
    compile_should_fail_with(
        "property fenced(f: field, authority: field<int>, grant: type) {\n    f guarded_by (g: grant) g.token == authority\n}\n\nclass Grant {\n    token: int\n}\n\nclass C satisfies fenced(self.data, self.epoch, Grant), fenced(self.data, self.epoch, Grant) {\n    data: int\n    epoch: int\n}\n\nfn main() {}\n",
        "already guarded by property 'fenced'",
    );
}

// ============================================================
// Substitution proving: the happy paths
// ============================================================

#[test]
fn monotonic_happy_path() {
    let out = compile_and_run_stdout(
        "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nobject Counter satisfies monotonic(self.n) {\n    n: int\n\n    fn bump(mut self) {\n        self.n = self.n + 1\n    }\n\n    fn get(self) int {\n        return self.n\n    }\n}\n\nfn main() {\n    let mut c = Counter { n: 0 }\n    c.bump()\n    c.bump()\n    print(c.get())\n}\n",
    );
    assert_eq!(out, "2\n");
}

#[test]
fn fenced_happy_path() {
    let out = compile_and_run_stdout(
        "property fenced(f: field, authority: field<int>, grant: type) {\n    f guarded_by (g: grant) g.token == authority\n}\n\nclass Grant {\n    token: int\n}\n\nobject Store satisfies fenced(self.data, self.epoch, Grant) {\n    data: string\n    epoch: int\n\n    fn grant(mut self) Grant {\n        self.epoch = self.epoch + 1\n        return Grant { token: self.epoch }\n    }\n\n    fn apply(mut self, g: Grant, d: string) {\n        let tok = g.token\n        if tok != self.epoch {\n            return\n        }\n        self.data = d\n    }\n\n    fn read(self) string {\n        return self.data\n    }\n}\n\nfn main() {\n    let mut s = Store { data: \"x\", epoch: 0 }\n    let g = s.grant()\n    s.apply(g, \"hello\")\n    print(s.read())\n}\n",
    );
    assert_eq!(out, "hello\n");
}

#[test]
fn multi_property_satisfies() {
    let out = compile_and_run_stdout(
        "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nproperty bounded(f: field<int>, lo: const int) {\n    invariant f >= lo\n}\n\nclass Score satisfies monotonic(self.n), bounded(self.n, 0) {\n    n: int\n\n    fn add(mut self, k: int) {\n        if k > 0 {\n            self.n = self.n + k\n        }\n    }\n}\n\nfn main() {\n    let mut s = Score { n: 0 }\n    s.add(7)\n    s.add(-3)\n    print(s.n)\n}\n",
    );
    assert_eq!(out, "7\n");
}

#[test]
fn const_int_param_substitutes() {
    // bounded(self.n, 10): constructing below the bound must fail.
    compile_should_fail_with_all(
        "property bounded(f: field<int>, lo: const int) {\n    invariant f >= lo\n}\n\nclass C satisfies bounded(self.n, 10) {\n    n: int\n}\n\nfn main() {\n    let c = C { n: 3 }\n}\n",
        &[
            "violates its invariant 'self.n >= 10'",
            "required by property 'bounded'",
            "instantiated with f = self.n, lo = 10",
        ],
    );
}

// ============================================================
// Two-sided blame
// ============================================================

#[test]
fn violating_monotonic_blames_both_sides() {
    // The failing-site half: class, method, symbolic state. The
    // property-body half: name, definition line, instantiation bindings.
    compile_should_fail_with_all(
        "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nobject Counter satisfies monotonic(self.n) {\n    n: int\n\n    fn reset(mut self) {\n        self.n = 0\n    }\n}\n\nfn main() {\n    let mut c = Counter { n: 0 }\n    c.reset()\n}\n",
        &[
            // the failing site in user code (standard symbolic-state diagnostic)
            "cannot prove invariant 'self.n >= old(self.n)' of class 'Counter'",
            "in method 'reset'",
            "at this point self.n = 0",
            // the property-body side
            "required by property 'monotonic' (defined at line 2)",
            "instantiated with f = self.n",
        ],
    );
}

#[test]
fn violating_fenced_blames_both_sides() {
    compile_should_fail_with_all(
        "property fenced(f: field, authority: field<int>, grant: type) {\n    f guarded_by (g: grant) g.token == authority\n}\n\nclass Grant {\n    token: int\n}\n\nobject Store satisfies fenced(self.data, self.epoch, Grant) {\n    data: string\n    epoch: int\n\n    fn sneak(mut self, d: string) {\n        self.data = d\n    }\n}\n\nfn main() {\n    let mut s = Store { data: \"x\", epoch: 0 }\n    s.sneak(\"oops\")\n}\n",
        &[
            // the failing site (standard dominance diagnostic)
            "cannot prove guard 'data' guarded_by (g: Grant) g.token == self.epoch of class 'Store'",
            // the property-body side
            "required by property 'fenced' (defined at line 2)",
            "instantiated with f = self.data, authority = self.epoch, grant = Grant",
        ],
    );
}

#[test]
fn foreign_write_to_property_guarded_field_blames_both_sides() {
    compile_should_fail_with_all(
        "property fenced(f: field, authority: field<int>, grant: type) {\n    f guarded_by (g: grant) g.token == authority\n}\n\nclass Grant {\n    token: int\n}\n\nclass Store satisfies fenced(self.data, self.epoch, Grant) {\n    data: string\n    epoch: int\n}\n\nfn main() {\n    let mut s = Store { data: \"x\", epoch: 0 }\n    s.data = \"outside\"\n}\n",
        &[
            "may only be written through 'self'",
            "required by property 'fenced'",
        ],
    );
}

#[test]
fn unprovable_instantiated_fragment_blames_property() {
    // A float field passes the declaration-time kind check for `field`
    // targets only via field<int> — here the property author wrote a body
    // whose atom lands outside the provable fragment after substitution is
    // impossible through kinds, so instead: instantiation-time fragment
    // errors still carry blame when the binder class lacks the compared
    // field. The guard predicate names g.token, but Evidence has no such
    // field — the standard registration error must carry the property side.
    compile_should_fail_with_all(
        "property fenced(f: field, authority: field<int>, grant: type) {\n    f guarded_by (g: grant) g.token == authority\n}\n\nclass Evidence {\n    stamp: int\n}\n\nclass Store satisfies fenced(self.data, self.epoch, Evidence) {\n    data: string\n    epoch: int\n}\n\nfn main() {}\n",
        &["required by property 'fenced'"],
    );
}

// ============================================================
// Cross-module properties
// ============================================================

#[test]
fn cross_module_property() {
    let out = run_project(&[
        (
            "main.pt",
            "import verify\n\nobject Counter satisfies verify.monotonic(self.n) {\n    n: int\n\n    fn bump(mut self) {\n        self.n = self.n + 1\n    }\n}\n\nfn main() {\n    let mut c = Counter { n: 0 }\n    c.bump()\n    print(c.n)\n}\n",
        ),
        (
            "verify/verify.pt",
            "pub property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n",
        ),
    ]);
    assert_eq!(out, "1\n");
}

#[test]
fn cross_module_violation_blames_module_qualified_property() {
    compile_project_should_fail_with_all(
        &[
            (
                "main.pt",
                "import verify\n\nobject Counter satisfies verify.monotonic(self.n) {\n    n: int\n\n    fn reset(mut self) {\n        self.n = 0\n    }\n}\n\nfn main() {\n    let mut c = Counter { n: 0 }\n    c.reset()\n}\n",
            ),
            (
                "verify/verify.pt",
                "pub property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n",
            ),
        ],
        &[
            "cannot prove invariant 'self.n >= old(self.n)' of class 'Counter'",
            "required by property 'monotonic' (defined at verify, line 2)",
            "instantiated with f = self.n",
        ],
    );
}

#[test]
fn private_property_not_importable() {
    compile_project_should_fail_with_all(
        &[
            (
                "main.pt",
                "import verify\n\nclass C satisfies verify.monotonic(self.n) {\n    n: int\n}\n\nfn main() {}\n",
            ),
            (
                "verify/verify.pt",
                "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n",
            ),
        ],
        &["'monotonic' is private to module 'verify'"],
    );
}

// ============================================================
// Name retention (the analyze surface)
// ============================================================

#[test]
fn provided_property_names_retained_in_derived_info() {
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("main.pt");
    std::fs::write(
        &entry,
        "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nproperty bounded(f: field<int>, lo: const int) {\n    invariant f >= lo\n}\n\nclass Score satisfies monotonic(self.n), bounded(self.n, 0) {\n    n: int\n}\n\nfn main() {}\n",
    )
    .unwrap();
    let (program, _source, derived) =
        pluto::analyze_file(&entry, None).expect("analyze succeeds");
    let class = program
        .classes
        .iter()
        .find(|c| c.node.name.node == "Score")
        .expect("class present");
    let info = derived
        .class_infos
        .get(&class.node.id)
        .expect("class info present");
    assert_eq!(
        info.provided_properties,
        vec!["monotonic(self.n)".to_string(), "bounded(self.n, 0)".to_string()],
        "the property names are the type's exported facts"
    );
}

// ============================================================
// Blob acceptance (RFC acceptance test 2)
// ============================================================

#[test]
fn blob_example_compiles_with_std_verify() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let entry = manifest_dir.join("examples/blob/main.pt");
    let stdlib = manifest_dir.join("stdlib");
    let out_dir = tempfile::tempdir().unwrap();
    let bin_path = out_dir.path().join("blob_bin");
    pluto::compile_file_with_stdlib(&entry, &bin_path, Some(&stdlib))
        .unwrap_or_else(|e| panic!("blob example failed to compile: {e}"));
    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(run_output.status.success());
    let stdout = String::from_utf8_lossy(&run_output.stdout);
    assert!(stdout.contains("B wrote -> blob is 'B: the good copy' (epoch 2)"), "stdout:\n{stdout}");
}

#[test]
fn blob_violating_variant_fails_with_two_sided_blame() {
    // Inline restatement of the blob authority (local property stands in
    // for std.verify — compile_to_object has no stdlib root) with a method
    // that decrements the epoch: the monotonic instantiation must reject
    // it, naming both the property and the site.
    compile_should_fail_with_all(
        "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nproperty fenced(f: field, authority: field<int>, grant: type) {\n    f guarded_by (g: grant) g.token == authority\n}\n\nclass WriteGrant {\n    token: int\n}\n\nobject BlobAuthority satisfies monotonic(self.epoch), fenced(self.data, self.epoch, WriteGrant) {\n    data: string\n    epoch: int\n\n    fn grant_write(mut self) WriteGrant {\n        self.epoch = self.epoch + 1\n        return WriteGrant { token: self.epoch }\n    }\n\n    fn rollback(mut self) {\n        self.epoch = self.epoch - 1\n    }\n\n    fn apply(mut self, grant: WriteGrant, d: string) {\n        let tok = grant.token\n        if tok != self.epoch {\n            return\n        }\n        self.data = d\n    }\n}\n\nfn main() {\n    let mut blob = BlobAuthority { data: \"genesis\", epoch: 0 }\n    blob.rollback()\n}\n",
        &[
            // Refuted outright: the symbolic state at exit contradicts the relation.
            "invariant 'self.epoch >= old(self.epoch)' of class 'BlobAuthority' is violated",
            "in method 'rollback'",
            "at this point self.epoch = old(self.epoch) - 1",
            "required by property 'monotonic' (defined at line 2)",
            "instantiated with f = self.epoch",
        ],
    );
}
