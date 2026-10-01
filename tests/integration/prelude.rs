mod common;
use common::compile_and_run_stdout;

// ── Prelude is empty (Option<T> removed in favor of T? nullable types) ──
// These tests verify the prelude infrastructure still works even when empty.

#[test]
fn prelude_empty_program_compiles() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(42)\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn prelude_user_can_define_option_enum() {
    // Since Option is no longer in the prelude, users can define their own
    let out = compile_and_run_stdout(
        "enum Option<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n    let o = Option<int>.Some { value: 42 }\n    match o {\n        Option.Some { value: v } {\n            print(v)\n        }\n        Option.None {\n            print(0)\n        }\n    }\n}",
    );
    assert_eq!(out, "42\n");
}

// ── Prelude classes can carry contracts ─────────────────────────────────
// The prelude is injected AFTER module flattening (pipeline stage 5 vs 4),
// so its QualifiedAccess nodes (e.g. `self.offset` in a contract) must be
// resolved at prelude parse time. These tests exercise that path: FieldInfo
// in stdlib/prelude.pt declares `invariant self.offset >= 0`.

#[test]
fn prelude_class_invariant_does_not_panic() {
    // Regression test: this used to panic at contracts.rs with
    // "QualifiedAccess should be resolved by module flattening before contracts"
    let out = compile_and_run_stdout("fn main() {\n    print(1)\n}");
    assert_eq!(out, "1\n");
}

#[test]
fn prelude_class_invariant_discharges_on_valid_construction() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let f = FieldInfo { name: \"a\", type_name: \"int\", offset: 4 }\n    print(f.offset)\n}",
    );
    assert_eq!(out, "4\n");
}

#[test]
fn prelude_class_invariant_rejects_violating_construction() {
    common::compile_should_fail_with(
        "fn main() {\n    let f = FieldInfo { name: \"a\", type_name: \"int\", offset: -1 }\n    print(f.offset)\n}",
        "violates its invariant",
    );
}

#[test]
fn prelude_class_invariant_reflection_still_works() {
    // Reflection synthesizes FieldInfo constructions (src/reflection.rs);
    // they must coexist with the invariant on FieldInfo.
    let out = compile_and_run_stdout(
        "class User {\n    username: string\n    age: int\n}\n\nfn main() {\n    let k = TypeInfo::kind<User>()\n    match k {\n        TypeKind.Class { info } {\n            print(info.fields.len())\n        }\n        TypeKind.Primitive { name } { print(0) }\n        TypeKind.Enum { info } { print(0) }\n        TypeKind.Array { element_type } { print(0) }\n        TypeKind.Map { key_type, value_type } { print(0) }\n        TypeKind.Set { element_type } { print(0) }\n        TypeKind.Nullable { inner_type } { print(0) }\n        TypeKind.Function { param_types, return_type } { print(0) }\n        TypeKind.Void { print(0) }\n    }\n}",
    );
    assert_eq!(out, "2\n");
}
