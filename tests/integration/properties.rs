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
fn declared_only_property_cannot_be_satisfied() {
    // Phase 5: an empty body is a DECLARED-ONLY property — legal to
    // declare (no in-unit proof shape exists yet), but never vacuously
    // satisfiable: 'satisfies' of it is an instantiation-site error.
    compile_should_fail_with(
        "property opaque(f: field<int>) {\n}\n\nclass C satisfies opaque(self.n) {\n    n: int\n}\n\nfn main() {}\n",
        "no checkable atoms",
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

#[test]
fn blob_unfenced_write_path_rejected() {
    // The stale-grant failure mode cannot be compiled away: an apply()
    // whose write path drops the fence (inline restatement of the std.blob
    // authority — compile_to_object has no stdlib root) is rejected by the
    // fenced instantiation's dominance proof, with two-sided blame.
    compile_should_fail_with_all(
        "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nproperty fenced(f: field, authority: field<int>, grant: type) {\n    f guarded_by (g: grant) g.token == authority\n}\n\nclass WriteGrant {\n    token: int\n}\n\nobject BlobAuthority satisfies monotonic(self.epoch), fenced(self.data, self.epoch, WriteGrant) {\n    data: string\n    epoch: int\n\n    fn grant_write(mut self) WriteGrant {\n        self.epoch = self.epoch + 1\n        return WriteGrant { token: self.epoch }\n    }\n\n    fn apply(mut self, grant: WriteGrant, d: string) {\n        self.data = d\n    }\n}\n\nfn main() {\n    let mut store = BlobAuthority { data: \"genesis\", epoch: 0 }\n    let g = store.grant_write()\n    store.apply(g, \"unfenced\")\n}\n",
        &[
            // The failing site: the write is not dominated by any grant check.
            "cannot prove guard 'data' guarded_by (g: WriteGrant) g.token == self.epoch of class 'BlobAuthority'",
            // The property-body side.
            "required by property 'fenced'",
            "instantiated with f = self.data, authority = self.epoch, grant = WriteGrant",
        ],
    );
}

/// std.blob's authority exports its safety theorem by name: the satisfies
/// clauses survive library-ification and module flattening, and surface as
/// the type's provided properties in analyze/DerivedInfo — assumable
/// downstream without reading the implementation.
#[test]
fn std_blob_authority_exports_its_theorem() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib = manifest_dir.join("stdlib");
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("main.pt");
    std::fs::write(
        &entry,
        "import std.blob\n\nfn main() {\n    let b = blob.create(\"x\")\n    print(b.epoch_now())\n}\n",
    )
    .unwrap();
    let (program, _source, derived) =
        pluto::analyze_file(&entry, Some(&stdlib)).expect("analyze succeeds");
    let class = program
        .classes
        .iter()
        .find(|c| c.node.name.node == "blob.BlobAuthority")
        .expect("blob.BlobAuthority present in flattened program");
    let info = derived
        .class_infos
        .get(&class.node.id)
        .expect("class info present");
    // The names carry the flattened module path: std.verify arrived through
    // std.blob's own import, so the properties read blob.verify.* and the
    // grant argument is the library's blob.WriteGrant.
    assert_eq!(
        info.provided_properties,
        vec![
            "blob.verify.monotonic(self.epoch)".to_string(),
            "blob.verify.fenced(self.data, self.epoch, blob.WriteGrant)".to_string(),
        ],
        "the property names are the authority's exported facts"
    );
}

// ============================================================
// Phase 5: provides / assume / fn-type requirements
// ============================================================

#[test]
fn provides_and_assume_parse_and_pretty_round_trip() {
    let src = "property idempotent(key: expr) {\n}\n\nextern fn ext_put(k: string) int assume idempotent(key = k)\n\nproperty increments(f: field<int>, by: const int) {\n    ensures f == old(f) + by\n}\n\nobject Counter {\n    n: int\n\n    fn bump(mut self) provides increments(self.n, 1) {\n        self.n = self.n + 1\n    }\n}\n\nfn main() {}\n";
    let program = pluto::parse_source(src).expect("parses");
    let ext = &program.extern_fns[0].node;
    assert_eq!(ext.assumes.len(), 1);
    assert_eq!(ext.assumes[0].node.name.node, "idempotent");
    assert_eq!(ext.assumes[0].node.line, 4);
    let arg = &ext.assumes[0].node.args[0];
    assert_eq!(arg.name.as_ref().unwrap().node, "key");
    let counter = program.classes.iter().find(|c| c.node.name.node == "Counter").unwrap();
    let bump = &counter.node.methods[0].node;
    assert_eq!(bump.provides.len(), 1);
    assert_eq!(bump.provides[0].node.name.node, "increments");
    assert_eq!(bump.provides[0].node.args.len(), 2);

    let printed = pluto::pretty::pretty_print(&program, false);
    assert!(
        printed.contains("extern fn ext_put(k: string) int assume idempotent(key = k)"),
        "printed:\n{printed}"
    );
    assert!(
        printed.contains("fn bump(mut self) provides increments(self.n, 1)"),
        "printed:\n{printed}"
    );
    // The printed form parses back to the same shapes.
    let reparsed = pluto::parse_source(&printed).expect("round-trips");
    assert_eq!(reparsed.extern_fns[0].node.assumes.len(), 1);
    let counter = reparsed.classes.iter().find(|c| c.node.name.node == "Counter").unwrap();
    assert_eq!(counter.node.methods[0].node.provides.len(), 1);
}

#[test]
fn fn_type_provides_parses_and_pretty_round_trips() {
    let src = "property idempotent(key: expr) {\n}\n\nfn go(f: fn(string) int! provides idempotent, s: string) int {\n    return f(s) catch e { 0 }\n}\n\nfn main() {}\n";
    let program = pluto::parse_source(src).expect("parses");
    let go = program.functions.iter().find(|f| f.node.name.node == "go").unwrap();
    match &go.node.params[0].ty.node {
        pluto::parser::ast::TypeExpr::Fn { fallible, provides, .. } => {
            assert!(*fallible);
            assert_eq!(provides, &vec!["idempotent".to_string()]);
        }
        other => panic!("expected fn type, got {other:?}"),
    }
    let printed = pluto::pretty::pretty_print(&program, false);
    assert!(
        printed.contains("f: fn(string) int! provides idempotent"),
        "printed:\n{printed}"
    );
    pluto::parse_source(&printed).expect("round-trips");
}

#[test]
fn fn_type_provides_with_args_rejected_at_parse() {
    compile_should_fail_with(
        "property idempotent(key: expr) {\n}\n\nfn go(f: fn(string) int! provides idempotent(key = s)) int {\n    return 0\n}\n\nfn main() {}\n",
        "matched by name",
    );
}

#[test]
fn trait_method_provides_rejected() {
    compile_should_fail_with(
        "property idempotent(key: expr) {\n}\n\ntrait Store {\n    fn put(self, k: string) int provides idempotent(key = k)\n}\n\nfn main() {}\n",
        "trait methods cannot declare 'provides'",
    );
}

// ── Discharge modes, honestly ───────────────────────────────

#[test]
fn in_unit_provides_of_declared_only_property_rejected() {
    // THE discharge-gap diagnostic: idempotent has no in-unit proof shape
    // (phase 5.5); the error must explain the gap and point at extern
    // assume — never silently promote.
    compile_should_fail_with_all(
        "property idempotent(key: expr) {\n}\n\nfn mine(s: string) int provides idempotent(key = s) {\n    return 1\n}\n\nfn main() {}\n",
        &[
            "cannot discharge 'provides idempotent' in-unit",
            "no checkable atoms",
            "phase 5.5",
            "never silently promoted",
            "assume idempotent(...)",
            "assumption surface",
        ],
    );
}

#[test]
fn provides_of_type_level_property_rejected() {
    compile_should_fail_with_all(
        "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nobject C {\n    n: int\n\n    fn bump(mut self) provides monotonic(self.n) {\n        self.n = self.n + 1\n    }\n}\n\nfn main() {}\n",
        &["property 'monotonic' is type-level", "'satisfies', not to a function via 'provides'"],
    );
}

#[test]
fn free_fn_provides_of_ensures_property_rejected() {
    compile_should_fail_with_all(
        "property increments(f: field<int>, by: const int) {\n    ensures f == old(f) + by\n}\n\nclass D {\n    n: int\n}\n\nfn standalone(d: D) int provides increments(d.n, 1) {\n    return 1\n}\n\nfn main() {}\n",
        &["has no carrying type"],
    );
}

#[test]
fn satisfies_of_ensures_property_rejected() {
    compile_should_fail_with_all(
        "property increments(f: field<int>, by: const int) {\n    ensures f == old(f) + by\n}\n\nclass C satisfies increments(self.n, 1) {\n    n: int\n}\n\nfn main() {}\n",
        &["method-level atoms (ensures / dedup)", "not", "satisfied by a type"],
    );
}

#[test]
fn mixed_level_property_rejected_at_declaration() {
    compile_should_fail_with(
        "property both(f: field<int>) {\n    invariant f >= 0\n    ensures f == old(f)\n}\n\nfn main() {}\n",
        "mixes type-level atoms",
    );
}

#[test]
fn expr_param_inside_atom_rejected() {
    compile_should_fail_with(
        "property bad(k: expr) {\n    ensures k == old(k)\n}\n\nfn main() {}\n",
        "kind 'expr' and cannot appear",
    );
}

#[test]
fn satisfies_of_expr_param_property_rejected() {
    compile_should_fail_with(
        "property keyed(f: field<int>, key: expr) {\n    invariant f >= 0\n}\n\nclass C satisfies keyed(self.n, self.n) {\n    n: int\n}\n\nfn main() {}\n",
        "'satisfies' cannot supply it",
    );
}

// ── The proven path: ensures-bodied properties on methods ───

#[test]
fn ensures_property_proven_on_method() {
    let out = compile_and_run_stdout(
        "property increments(f: field<int>, by: const int) {\n    ensures f == old(f) + by\n}\n\nobject Counter {\n    n: int\n\n    fn bump(mut self) provides increments(self.n, 1) {\n        self.n = self.n + 1\n    }\n}\n\nfn main() {\n    let mut c = Counter { n: 0 }\n    c.bump()\n    c.bump()\n    print(c.n)\n}\n",
    );
    assert_eq!(out, "2\n");
}

#[test]
fn ensures_property_violation_blames_both_sides() {
    compile_should_fail_with_all(
        "property increments(f: field<int>, by: const int) {\n    ensures f == old(f) + by\n}\n\nobject Counter {\n    n: int\n\n    fn bump(mut self) provides increments(self.n, 2) {\n        self.n = self.n + 1\n    }\n}\n\nfn main() {\n    let mut c = Counter { n: 0 }\n    c.bump()\n}\n",
        &[
            "ensures clause 'self.n == old(self.n) + 2' of method 'bump' of class 'Counter' is violated",
            "required by property 'increments' (defined at line 2)",
            "instantiated with f = self.n, by = 2",
        ],
    );
}

// ── Argument validation diagnostics ─────────────────────────

#[test]
fn provides_arity_mismatch_rejected() {
    compile_should_fail_with(
        "property increments(f: field<int>, by: const int) {\n    ensures f == old(f) + by\n}\n\nobject C {\n    n: int\n\n    fn bump(mut self) provides increments(self.n) {\n        self.n = self.n + 1\n    }\n}\n\nfn main() {}\n",
        "expects 2 arguments (f: field<int>, by: const int), found 1",
    );
}

#[test]
fn expr_param_requires_named_form() {
    compile_should_fail_with(
        "property idempotent(key: expr) {\n}\n\nextern fn ext_put(k: string) int assume idempotent(k)\n\nfn main() {}\n",
        "takes the named form",
    );
}

#[test]
fn named_form_on_positional_param_rejected() {
    compile_should_fail_with(
        "property increments(f: field<int>, by: const int) {\n    ensures f == old(f) + by\n}\n\nobject C {\n    n: int\n\n    fn bump(mut self) provides increments(f = self.n, by = 1) {\n        self.n = self.n + 1\n    }\n}\n\nfn main() {}\n",
        "is positional: the named form",
    );
}

#[test]
fn expr_arg_must_range_over_parameters() {
    compile_should_fail_with(
        "property idempotent(key: expr) {\n}\n\nextern fn ext_put(k: string) int assume idempotent(key = other)\n\nfn main() {}\n",
        "'other' is not a parameter",
    );
}

#[test]
fn unknown_property_in_provides_rejected() {
    compile_should_fail_with(
        "fn mine(s: string) int provides ghost(key = s) {\n    return 1\n}\n\nfn main() {}\n",
        "unknown property 'ghost'",
    );
}

#[test]
fn unknown_property_in_fn_type_rejected() {
    compile_should_fail_with(
        "fn go(f: fn(string) int! provides ghost) int {\n    return 0\n}\n\nfn main() {}\n",
        "unknown property 'ghost' in fn-type requirement",
    );
}

#[test]
fn extern_assume_of_field_param_property_rejected() {
    compile_should_fail_with(
        "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nextern fn ext_tick() int assume monotonic(self.n)\n\nfn main() {}\n",
        "is type-level",
    );
}

// ── Requires-matching at fn-type boundaries ─────────────────

#[test]
fn extern_assumed_provider_flows_into_requiring_type() {
    // The epistemics payoff: an ASSUMED provider satisfies the fn-type
    // requirement; the call compiles and runs.
    let out = compile_and_run_stdout(
        "property idempotent(key: expr) {\n}\n\nextern fn __pluto_string_len(s: string) int assume idempotent(key = s)\n\nfn apply_idem(f: fn(string) int! provides idempotent, s: string) int {\n    return f(s) catch e { 0 - 1 }\n}\n\nfn main() {\n    print(apply_idem(__pluto_string_len, \"hello\"))\n}\n",
    );
    assert_eq!(out, "5\n");
}

#[test]
fn non_providing_function_rejected_at_boundary() {
    compile_should_fail_with(
        "property idempotent(key: expr) {\n}\n\nfn plain(s: string) int {\n    return 7\n}\n\nfn apply_idem(f: fn(string) int! provides idempotent, s: string) int {\n    return f(s) catch e { 0 - 1 }\n}\n\nfn main() {\n    print(apply_idem(plain, \"hello\"))\n}\n",
        "expected fn(string) int! provides idempotent, found fn(string) int",
    );
}

#[test]
fn closure_rejected_at_provides_boundary() {
    // Closures provide nothing — a closure literal cannot satisfy a
    // provides-requiring fn type.
    compile_should_fail_with(
        "property idempotent(key: expr) {\n}\n\nfn apply_idem(f: fn(string) int! provides idempotent, s: string) int {\n    return f(s) catch e { 0 - 1 }\n}\n\nfn main() {\n    print(apply_idem((s: string) => 3, \"x\"))\n}\n",
        "provides idempotent, found fn(string) int",
    );
}

#[test]
fn provides_survives_variable_binding() {
    // A fn-ref wrapper (eta-expansion through closure lifting) carries the
    // provides through: bind the extern to an annotated variable, then
    // pass the variable.
    let out = compile_and_run_stdout(
        "property idempotent(key: expr) {\n}\n\nextern fn __pluto_string_len(s: string) int assume idempotent(key = s)\n\nfn apply_idem(f: fn(string) int! provides idempotent, s: string) int {\n    return f(s) catch e { 0 - 1 }\n}\n\nfn main() {\n    let g: fn(string) int! provides idempotent = __pluto_string_len\n    print(apply_idem(g, \"worldly\"))\n}\n",
    );
    assert_eq!(out, "7\n");
}

#[test]
fn unannotated_binding_without_provides_rejected() {
    // Subsumption direction: a plain fn-typed binding erases the provides,
    // so passing it onward is a boundary rejection — the claim never
    // launders through an unannotated type.
    compile_should_fail_with(
        "property idempotent(key: expr) {\n}\n\nextern fn __pluto_string_len(s: string) int assume idempotent(key = s)\n\nfn apply_idem(f: fn(string) int! provides idempotent, s: string) int {\n    return f(s) catch e { 0 - 1 }\n}\n\nfn main() {\n    let g: fn(string) int = __pluto_string_len\n    print(apply_idem(g, \"worldly\"))\n}\n",
        "provides idempotent, found fn(string) int",
    );
}

// ── The assumption surface ──────────────────────────────────

#[test]
fn assumed_claims_land_in_derived_info() {
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("main.pt");
    std::fs::write(
        &entry,
        "property idempotent(key: expr) {\n}\n\nextern fn ext_put(k: string, v: string) int assume idempotent(key = k)\n\nfn main() {}\n",
    )
    .unwrap();
    let (_program, _source, derived) =
        pluto::analyze_file(&entry, None).expect("analyze succeeds");
    assert_eq!(derived.assumptions.len(), 1);
    let claim = &derived.assumptions[0];
    assert_eq!(claim.owner, "extern fn ext_put");
    assert_eq!(claim.property, "idempotent");
    assert_eq!(claim.instantiation, "key = k");
    assert_eq!(claim.line, 4);
    assert_eq!(
        claim.to_string(),
        "assume idempotent(key = k) — extern fn ext_put (line 4)"
    );
}

// ── Cross-module provides/assume ────────────────────────────

#[test]
fn cross_module_assume_and_requirement() {
    let out = run_project(&[
        (
            "main.pt",
            "import verify\n\nextern fn __pluto_string_len(s: string) int assume verify.idempotent(key = s)\n\nfn apply_idem(f: fn(string) int! provides verify.idempotent, s: string) int {\n    return f(s) catch e { 0 - 1 }\n}\n\nfn main() {\n    print(apply_idem(__pluto_string_len, \"abc\"))\n}\n",
        ),
        (
            "verify/verify.pt",
            "pub property idempotent(key: expr) {\n}\n",
        ),
    ]);
    assert_eq!(out, "3\n");
}

#[test]
fn private_property_not_assumable_across_modules() {
    compile_project_should_fail_with_all(
        &[
            (
                "main.pt",
                "import verify\n\nextern fn ext_put(k: string) int assume verify.idempotent(key = k)\n\nfn main() {}\n",
            ),
            (
                "verify/verify.pt",
                "property idempotent(key: expr) {\n}\n",
            ),
        ],
        &["'idempotent' is private to module 'verify'"],
    );
}

// ── Retry acceptance test (rfc-properties.md acceptance 3) ──

#[test]
fn retry_example_compiles_and_runs() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let entry = manifest_dir.join("examples/retry/main.pt");
    let stdlib = manifest_dir.join("stdlib");
    let out_dir = tempfile::tempdir().unwrap();
    let bin_path = out_dir.path().join("retry_bin");
    pluto::compile_file_with_stdlib(&entry, &bin_path, Some(&stdlib))
        .unwrap_or_else(|e| panic!("retry example failed to compile: {e}"));
    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(run_output.status.success());
    let stdout = String::from_utf8_lossy(&run_output.stdout);
    assert!(stdout.contains("stored (receipt 5)"), "stdout:\n{stdout}");
    // The in-unit CHECKED provider (phase 5.5): same key twice, the second
    // call performed no additional effect.
    assert!(stdout.contains("ledger receipts: 1 then 1"), "stdout:\n{stdout}");
}

#[test]
fn retry_example_rejects_non_provider() {
    // The boundary half of acceptance test 3, against std.verify itself.
    compile_project_should_fail_with_all(
        &[
            (
                "main.pt",
                "import verify\n\nfn plain(s: string) int {\n    return 7\n}\n\nfn with_retry(f: fn(string) int! provides verify.idempotent, req: string) int {\n    return f(req) catch e { 0 - 1 }\n}\n\nfn main() {\n    print(with_retry(plain, \"x\"))\n}\n",
            ),
            (
                "verify/verify.pt",
                "pub property idempotent(key: expr) {\n}\n",
            ),
        ],
        &["expected fn(string) int! provides verify.idempotent, found fn(string) int"],
    );
}

// ── Phase 5.5: the dedup-guard discharge (CHECKED) ──────────

// A dedup-bodied property shared by the tests below.
const DEDUP_PROP: &str = "property idempotent(key: expr) {\n    dedup key\n}\n\n";

#[test]
fn dedup_atom_parses_and_pretty_round_trips() {
    let src = "property idempotent(key: expr) {\n    dedup key\n}\n\nfn main() {}\n";
    let program = pluto::parse_source(src).expect("parses");
    assert_eq!(program.properties.len(), 1);
    let printed = pluto::pretty::pretty_print(&program, false);
    assert!(printed.contains("dedup key"), "pretty output:\n{printed}");
    let reparsed = pluto::parse_source(&printed).expect("pretty output reparses");
    assert_eq!(reparsed.properties.len(), 1);
}

#[test]
fn dedup_happy_path_on_entity() {
    // The canonical shape: check, armed insert, effect. Entity methods are
    // serialized, closing the check→insert→effect window. Observable
    // behavior: same key twice ⇒ no additional effect.
    let out = compile_and_run_stdout(&format!(
        "{DEDUP_PROP}object Payments {{\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, k: string, amt: int) int provides idempotent(key = k) {{\n        if self.seen.contains(k) {{\n            return self.total\n        }}\n        self.seen.insert(k)\n        self.total = self.total + amt\n        return self.total\n    }}\n}}\n\nfn main() {{\n    let mut p = Payments {{ seen: Set<string> {{}}, total: 0 }}\n    print(p.apply(\"a\", 5))\n    print(p.apply(\"a\", 5))\n    print(p.apply(\"b\", 2))\n}}\n"
    ));
    assert_eq!(out, "5\n5\n7\n");
}

#[test]
fn dedup_happy_path_on_value_class_negated_check() {
    // Value classes are accepted too (the claim is per-copy — values do
    // not share), and the `if !contains { insert; effect }` then-branch
    // shape arms inside the branch.
    let out = compile_and_run_stdout(&format!(
        "{DEDUP_PROP}class Store {{\n    seen: Set<string>\n    n: int\n\n    fn put(mut self, k: string) int provides idempotent(key = k) {{\n        if !self.seen.contains(k) {{\n            self.seen.insert(k)\n            self.n = self.n + 1\n        }}\n        return self.n\n    }}\n}}\n\nfn main() {{\n    let mut s = Store {{ seen: Set<string> {{}}, n: 0 }}\n    print(s.put(\"x\"))\n    print(s.put(\"x\"))\n}}\n"
    ));
    assert_eq!(out, "1\n1\n");
}

#[test]
fn dedup_dotted_key_and_raise_on_duplicate() {
    // One-level field keys (`key = req.id`) work, and the duplicate path
    // may raise instead of returning a cached value — outcomes are not
    // effects.
    let out = compile_and_run_stdout(&format!(
        "{DEDUP_PROP}error Dup {{\n    message: string\n}}\n\nclass Req {{\n    id: string\n    amt: int\n}}\n\nobject Ledger {{\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, req: Req) int provides idempotent(key = req.id) {{\n        if self.seen.contains(req.id) {{\n            raise Dup {{ message: req.id }}\n        }}\n        self.seen.insert(req.id)\n        self.total = self.total + req.amt\n        return self.total\n    }}\n}}\n\nfn main() {{\n    let mut l = Ledger {{ seen: Set<string> {{}}, total: 0 }}\n    let r = Req {{ id: \"a\", amt: 3 }}\n    print(l.apply(r) catch e {{ 0 - 1 }})\n    print(l.apply(r) catch e {{ 0 - 1 }})\n}}\n"
    ));
    assert_eq!(out, "3\n-1\n");
}

#[test]
fn dedup_licenses_call_effects_after_armed_insert() {
    // Bare-parameter keys survive calls: the armed insert licenses a
    // SEQUENCE of call-shaped effects (SetInserted is monotone-stable —
    // the set is globally insert-only, and callees cannot change a
    // caller's local).
    let out = compile_and_run_stdout(&format!(
        "{DEDUP_PROP}extern fn __pluto_string_len(s: string) int\n\nobject P {{\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, k: string) int provides idempotent(key = k) {{\n        if self.seen.contains(k) {{\n            return self.total\n        }}\n        self.seen.insert(k)\n        let a = __pluto_string_len(k)\n        let b = __pluto_string_len(k)\n        self.total = self.total + a + b\n        return self.total\n    }}\n}}\n\nfn main() {{\n    let mut p = P {{ seen: Set<string> {{}}, total: 0 }}\n    print(p.apply(\"abc\"))\n    print(p.apply(\"abc\"))\n}}\n"
    ));
    assert_eq!(out, "6\n6\n");
}

#[test]
fn dedup_effect_without_check_rejected() {
    compile_should_fail_with_all(
        &format!(
            "{DEDUP_PROP}object P {{\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, k: string) int provides idempotent(key = k) {{\n        self.total = self.total + 1\n        return self.total\n    }}\n}}\n\nfn main() {{}}\n"
        ),
        &[
            "cannot discharge 'provides idempotent'",
            "not covered by the dedup guard",
            "ARMED insert",
            "required by property 'idempotent'",
            "instantiated with key = k",
        ],
    );
}

#[test]
fn dedup_check_after_effect_rejected() {
    // The effect precedes the check: nothing dominates it.
    compile_should_fail_with(
        &format!(
            "{DEDUP_PROP}object P {{\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, k: string) int provides idempotent(key = k) {{\n        self.total = self.total + 1\n        if self.seen.contains(k) {{ return self.total }}\n        self.seen.insert(k)\n        return self.total\n    }}\n}}\n\nfn main() {{}}\n"
        ),
        "not covered by the dedup guard",
    );
}

#[test]
fn dedup_insert_missing_on_effect_path_rejected() {
    // The insert happens only on one branch; the effect follows on both.
    compile_should_fail_with(
        &format!(
            "{DEDUP_PROP}object P {{\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, k: string, flag: bool) int provides idempotent(key = k) {{\n        if self.seen.contains(k) {{ return self.total }}\n        if flag {{\n            self.seen.insert(k)\n        }}\n        self.total = self.total + 1\n        return self.total\n    }}\n}}\n\nfn main() {{}}\n"
        ),
        "not covered by the dedup guard",
    );
}

#[test]
fn dedup_unarmed_insert_rejected() {
    // An insert of the key with no dominating check is just a mutation —
    // an effect the (nonexistent) guard does not cover.
    compile_should_fail_with(
        &format!(
            "{DEDUP_PROP}object P {{\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, k: string) int provides idempotent(key = k) {{\n        self.seen.insert(k)\n        self.total = self.total + 1\n        return self.total\n    }}\n}}\n\nfn main() {{}}\n"
        ),
        "not covered by the dedup guard",
    );
}

#[test]
fn dedup_effect_in_duplicate_branch_rejected() {
    // The already-seen branch runs on every duplicate call: an effect
    // there is exactly the double-apply the property forbids.
    compile_should_fail_with(
        &format!(
            "{DEDUP_PROP}object P {{\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, k: string) int provides idempotent(key = k) {{\n        if self.seen.contains(k) {{\n            self.total = self.total + 1\n            return self.total\n        }}\n        self.seen.insert(k)\n        self.total = self.total + 1\n        return self.total\n    }}\n}}\n\nfn main() {{}}\n"
        ),
        "not covered by the dedup guard",
    );
}

#[test]
fn dedup_remove_from_seen_rejected() {
    // Non-monotone use anywhere in the program breaks every claim keyed on
    // the field — even in a method that provides nothing.
    compile_should_fail_with_all(
        &format!(
            "{DEDUP_PROP}object P {{\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, k: string) int provides idempotent(key = k) {{\n        if self.seen.contains(k) {{ return self.total }}\n        self.seen.insert(k)\n        self.total = self.total + 1\n        return self.total\n    }}\n\n    fn forget(mut self, k: string) {{\n        self.seen.remove(k)\n    }}\n}}\n\nfn main() {{}}\n"
        ),
        &["dedup field 'seen'", "insert-only", "'remove'"],
    );
}

#[test]
fn dedup_field_alias_rejected() {
    // Returning (or binding, or passing) the dedup set would reopen the
    // closed write-set.
    compile_should_fail_with_all(
        &format!(
            "{DEDUP_PROP}object P {{\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, k: string) int provides idempotent(key = k) {{\n        if self.seen.contains(k) {{ return self.total }}\n        self.seen.insert(k)\n        self.total = self.total + 1\n        return self.total\n    }}\n\n    fn leak(self) Set<string> {{\n        return self.seen\n    }}\n}}\n\nfn main() {{}}\n"
        ),
        &["dedup field 'seen'", "may not be used as a value"],
    );
}

#[test]
fn dedup_foreign_insert_rejected() {
    compile_should_fail_with_all(
        &format!(
            "{DEDUP_PROP}object P {{\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, k: string) int provides idempotent(key = k) {{\n        if self.seen.contains(k) {{ return self.total }}\n        self.seen.insert(k)\n        self.total = self.total + 1\n        return self.total\n    }}\n}}\n\nfn main() {{\n    let mut p = P {{ seen: Set<string> {{}}, total: 0 }}\n    p.seen.insert(\"sneak\")\n}}\n"
        ),
        // The entity field-access rule (rfc-module-semantics.md section 3)
        // now rejects the reference-shaped read itself, before idempotency's
        // narrower foreign-insert diagnostic can fire — the attack is still
        // dead, one gate earlier. The idempotency diagnostic keeps its own
        // coverage via the value-class receiver below.
        &["cannot read field 'seen' of entity 'P'", "reference-shaped"],
    );
}

/// The same foreign-insert attack through a value-class receiver, where the
/// entity read rule does not apply: idempotency's own monotonicity guard is
/// the gate, and its diagnostic stays pinned.
#[test]
fn dedup_foreign_insert_rejected_value_class() {
    compile_should_fail_with_all(
        &format!(
            "{DEDUP_PROP}class P {{\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, k: string) int provides idempotent(key = k) {{\n        if self.seen.contains(k) {{ return self.total }}\n        self.seen.insert(k)\n        self.total = self.total + 1\n        return self.total\n    }}\n}}\n\nfn main() {{\n    let mut p = P {{ seen: Set<string> {{}}, total: 0 }}\n    p.seen.insert(\"sneak\")\n}}\n"
        ),
        &["dedup field 'seen'", "only be inserted through 'self'"],
    );
}

#[test]
fn dedup_shared_init_rejected() {
    // Construction must not alias the dedup state.
    compile_should_fail_with_all(
        &format!(
            "{DEDUP_PROP}object P {{\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, k: string) int provides idempotent(key = k) {{\n        if self.seen.contains(k) {{ return self.total }}\n        self.seen.insert(k)\n        self.total = self.total + 1\n        return self.total\n    }}\n}}\n\nfn main() {{\n    let shared = Set<string> {{}}\n    let mut p = P {{ seen: shared, total: 0 }}\n}}\n"
        ),
        &["dedup field 'seen'", "fresh set literal"],
    );
}

#[test]
fn dedup_reassigned_key_rejected() {
    // A reassigned key no longer denotes the claim's entry value: the
    // check never arms, so the effect is uncovered.
    compile_should_fail_with(
        &format!(
            "{DEDUP_PROP}object P {{\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, mut k: string) int provides idempotent(key = k) {{\n        k = \"other\"\n        if self.seen.contains(k) {{ return self.total }}\n        self.seen.insert(k)\n        self.total = self.total + 1\n        return self.total\n    }}\n}}\n\nfn main() {{}}\n"
        ),
        "not covered by the dedup guard",
    );
}

#[test]
fn dedup_effect_inside_closure_rejected() {
    // A closure created in the providing method may escape the serialized
    // dedup window; its effects can never be covered.
    compile_should_fail_with_all(
        &format!(
            "{DEDUP_PROP}object P {{\n    seen: Set<string>\n    total: int\n\n    fn apply(mut self, k: string) int provides idempotent(key = k) {{\n        if self.seen.contains(k) {{ return self.total }}\n        self.seen.insert(k)\n        let f = (x: int) => {{\n            print(x)\n            x\n        }}\n        self.total = self.total + 1\n        return self.total\n    }}\n}}\n\nfn main() {{}}\n"
        ),
        &["not covered by the dedup guard", "inside a closure body"],
    );
}

#[test]
fn dedup_free_fn_provides_rejected() {
    // No receiver, no dedup state: in-unit discharge is methods-only, and
    // the diagnostic points at the extern assume alternative.
    compile_should_fail_with_all(
        &format!(
            "{DEDUP_PROP}fn put(k: string) int provides idempotent(key = k) {{\n    return 1\n}}\n\nfn main() {{}}\n"
        ),
        &["'dedup' atom", "needs receiver state", "extern fn ... assume idempotent"],
    );
}

#[test]
fn dedup_property_satisfies_rejected() {
    compile_should_fail_with(
        &format!(
            "{DEDUP_PROP}class C satisfies idempotent(self.n) {{\n    n: int\n}}\n\nfn main() {{}}\n"
        ),
        "method-level atoms (ensures / dedup)",
    );
}

#[test]
fn dedup_mixed_with_ensures_rejected() {
    compile_should_fail_with(
        "property both(f: field<int>, key: expr) {\n    ensures f == old(f)\n    dedup key\n}\n\nfn main() {}\n",
        "mixes 'dedup' with other atoms",
    );
}

#[test]
fn dedup_key_must_be_expr_param() {
    compile_should_fail_with(
        "property bad(f: field<int>) {\n    dedup f\n}\n\nfn main() {}\n",
        "the key of a dedup atom must be an 'expr' parameter",
    );
}

#[test]
fn dedup_property_still_assumable_at_extern_boundary() {
    // An external system can implement the dedup internally: the ASSUMED
    // mode stays legal for dedup-shaped properties, and the claim flows
    // into requiring fn types as before.
    let out = compile_and_run_stdout(&format!(
        "{DEDUP_PROP}extern fn __pluto_string_len(s: string) int assume idempotent(key = s)\n\nfn apply_idem(f: fn(string) int! provides idempotent, s: string) int {{\n    return f(s) catch e {{ 0 - 1 }}\n}}\n\nfn main() {{\n    print(apply_idem(__pluto_string_len, \"hello\"))\n}}\n"
    ));
    assert_eq!(out, "5\n");
}

#[test]
fn delegation_closure_carries_method_provides() {
    // Strict-eta delegation: `(s) => store.put(s)` is observationally the
    // method with its receiver fixed, so it carries the method's provides
    // into requiring fn types.
    let out = compile_and_run_stdout(&format!(
        "{DEDUP_PROP}object Store {{\n    seen: Set<string>\n    n: int\n\n    fn put(mut self, k: string) int provides idempotent(key = k) {{\n        if self.seen.contains(k) {{ return self.n }}\n        self.seen.insert(k)\n        self.n = self.n + 1\n        return self.n\n    }}\n}}\n\nfn go(f: fn(string) int provides idempotent, s: string) int {{\n    return f(s)\n}}\n\nfn main() {{\n    let mut store = Store {{ seen: Set<string> {{}}, n: 0 }}\n    print(go((s: string) => store.put(s), \"a\"))\n    print(go((s: string) => store.put(s), \"a\"))\n}}\n"
    ));
    assert_eq!(out, "1\n1\n");
}

#[test]
fn non_eta_closure_still_provides_nothing() {
    // Anything short of the strict eta shape (here: a constant argument
    // instead of the forwarded parameter) provides nothing.
    compile_should_fail_with(
        &format!(
            "{DEDUP_PROP}object Store {{\n    seen: Set<string>\n    n: int\n\n    fn put(mut self, k: string) int provides idempotent(key = k) {{\n        if self.seen.contains(k) {{ return self.n }}\n        self.seen.insert(k)\n        self.n = self.n + 1\n        return self.n\n    }}\n}}\n\nfn go(f: fn(string) int provides idempotent, s: string) int {{\n    return f(s)\n}}\n\nfn main() {{\n    let mut store = Store {{ seen: Set<string> {{}}, n: 0 }}\n    print(go((s: string) => store.put(\"fixed\"), \"a\"))\n}}\n"
        ),
        "provides idempotent, found fn(string) int",
    );
}

#[test]
fn delegation_of_non_providing_method_provides_nothing() {
    compile_should_fail_with(
        &format!(
            "{DEDUP_PROP}object Store {{\n    n: int\n\n    fn put(mut self, k: string) int {{\n        self.n = self.n + 1\n        return self.n\n    }}\n}}\n\nfn go(f: fn(string) int provides idempotent, s: string) int {{\n    return f(s)\n}}\n\nfn main() {{\n    let mut store = Store {{ n: 0 }}\n    print(go((s: string) => store.put(s), \"a\"))\n}}\n"
        ),
        "provides idempotent, found fn(string) int",
    );
}
