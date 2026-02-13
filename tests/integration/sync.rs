#[allow(dead_code)]
mod common;

use plutoc::binary::{deserialize_program, is_binary_format, serialize_program};
use plutoc::derived::DerivedInfo;
use plutoc::parser::ast::Program;
use plutoc::pretty::pretty_print;
use plutoc::sync::sync_pt_to_pluto;
use std::path::Path;
use uuid::Uuid;

/// Parse source, serialize to a temp .pluto binary, return (path, program).
fn emit_ast(source: &str) -> (tempfile::TempDir, Program) {
    let program = plutoc::parse_for_editing(source).unwrap();
    let derived = DerivedInfo::default();
    let bytes = serialize_program(&program, source, &derived).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let pluto_path = dir.path().join("test.pluto");
    std::fs::write(&pluto_path, &bytes).unwrap();
    (dir, program)
}

/// Write .pt text, sync to existing .pluto, return the resulting program.
fn sync_and_load(pt_text: &str, pluto_path: &Path) -> Program {
    let dir = tempfile::tempdir().unwrap();
    let pt_path = dir.path().join("test.pt");
    std::fs::write(&pt_path, pt_text).unwrap();
    sync_pt_to_pluto(&pt_path, pluto_path).unwrap();
    let data = std::fs::read(pluto_path).unwrap();
    assert!(is_binary_format(&data));
    let (program, _source, _derived) = deserialize_program(&data).unwrap();
    program
}

/// Helper: find function UUID by name.
fn fn_uuid(program: &Program, name: &str) -> Uuid {
    program
        .functions
        .iter()
        .find(|f| f.node.name.node == name)
        .unwrap_or_else(|| panic!("function '{}' not found", name))
        .node
        .id
}

/// Helper: find class UUID by name.
fn class_uuid(program: &Program, name: &str) -> Uuid {
    program
        .classes
        .iter()
        .find(|c| c.node.name.node == name)
        .unwrap_or_else(|| panic!("class '{}' not found", name))
        .node
        .id
}

/// Helper: find trait UUID by name.
fn trait_uuid(program: &Program, name: &str) -> Uuid {
    program
        .traits
        .iter()
        .find(|t| t.node.name.node == name)
        .unwrap_or_else(|| panic!("trait '{}' not found", name))
        .node
        .id
}

/// Helper: find error UUID by name.
fn error_uuid(program: &Program, name: &str) -> Uuid {
    program
        .errors
        .iter()
        .find(|e| e.node.name.node == name)
        .unwrap_or_else(|| panic!("error '{}' not found", name))
        .node
        .id
}

// --- Tests ---

#[test]
fn round_trip_identity() {
    // emit-ast → generate-pt → sync → UUIDs should match original
    let source = "fn hello() {\n    print(1)\n}\n\nfn world() {\n    print(2)\n}\n";
    let (dir, original) = emit_ast(source);
    let pluto_path = dir.path().join("test.pluto");

    // Generate .pt from the binary
    let data = std::fs::read(&pluto_path).unwrap();
    let (program, _source, _derived) = deserialize_program(&data).unwrap();
    let pt_text = pretty_print(&program);

    // Sync back
    let synced = sync_and_load(&pt_text, &pluto_path);

    // UUIDs should match
    assert_eq!(fn_uuid(&synced, "hello"), fn_uuid(&original, "hello"));
    assert_eq!(fn_uuid(&synced, "world"), fn_uuid(&original, "world"));
}

#[test]
fn uuid_preservation_after_adding_function() {
    let source = "fn hello() {\n    print(1)\n}\n\nfn world() {\n    print(2)\n}\n";
    let (dir, original) = emit_ast(source);
    let pluto_path = dir.path().join("test.pluto");

    let hello_uuid = fn_uuid(&original, "hello");
    let world_uuid = fn_uuid(&original, "world");

    // Modify .pt: add a new function
    let modified_pt = "fn hello() {\n    print(1)\n}\n\nfn world() {\n    print(2)\n}\n\nfn added() {\n    print(3)\n}\n";
    let synced = sync_and_load(modified_pt, &pluto_path);

    // Old UUIDs preserved
    assert_eq!(fn_uuid(&synced, "hello"), hello_uuid);
    assert_eq!(fn_uuid(&synced, "world"), world_uuid);

    // New function has a different UUID
    let added_uuid = fn_uuid(&synced, "added");
    assert_ne!(added_uuid, hello_uuid);
    assert_ne!(added_uuid, world_uuid);
}

#[test]
fn deletion_handling() {
    let source = "fn hello() {\n    print(1)\n}\n\nfn world() {\n    print(2)\n}\n";
    let (dir, original) = emit_ast(source);
    let pluto_path = dir.path().join("test.pluto");

    let hello_uuid = fn_uuid(&original, "hello");

    // Remove "world" from .pt
    let modified_pt = "fn hello() {\n    print(1)\n}\n";
    let result = {
        let pt_dir = tempfile::tempdir().unwrap();
        let pt_path = pt_dir.path().join("test.pt");
        std::fs::write(&pt_path, modified_pt).unwrap();
        sync_pt_to_pluto(&pt_path, &pluto_path).unwrap()
    };
    let data = std::fs::read(&pluto_path).unwrap();
    let (synced, _, _) = deserialize_program(&data).unwrap();

    // "hello" UUID preserved
    assert_eq!(fn_uuid(&synced, "hello"), hello_uuid);

    // "world" is gone
    assert!(synced.functions.iter().all(|f| f.node.name.node != "world"));

    // Result reports removal
    assert!(result.removed.contains(&"fn world".to_string()));
}

#[test]
fn rename_handling() {
    let source = "fn hello() {\n    print(1)\n}\n\nfn world() {\n    print(2)\n}\n";
    let (dir, original) = emit_ast(source);
    let pluto_path = dir.path().join("test.pluto");

    let hello_uuid = fn_uuid(&original, "hello");
    let world_uuid = fn_uuid(&original, "world");

    // Rename "world" to "universe"
    let modified_pt = "fn hello() {\n    print(1)\n}\n\nfn universe() {\n    print(2)\n}\n";
    let synced = sync_and_load(modified_pt, &pluto_path);

    // "hello" UUID preserved
    assert_eq!(fn_uuid(&synced, "hello"), hello_uuid);

    // "universe" gets a fresh UUID (not the old "world" UUID)
    let universe_uuid = fn_uuid(&synced, "universe");
    assert_ne!(universe_uuid, world_uuid);

    // "world" is gone
    assert!(synced.functions.iter().all(|f| f.node.name.node != "world"));
}

#[test]
fn nested_uuid_preservation_class_fields_and_methods() {
    let source = r#"class Foo {
    x: int
    y: string

    fn get_x(self) int {
        return self.x
    }

    fn get_y(self) string {
        return self.y
    }
}

fn main() {
    let f = Foo { x: 1, y: "hi" }
    print(f.get_x())
}
"#;
    let (dir, original) = emit_ast(source);
    let pluto_path = dir.path().join("test.pluto");

    let orig_class = &original.classes[0].node;
    let orig_class_id = orig_class.id;
    let orig_x_id = orig_class.fields.iter().find(|f| f.name.node == "x").unwrap().id;
    let orig_y_id = orig_class.fields.iter().find(|f| f.name.node == "y").unwrap().id;
    let orig_get_x_id = orig_class.methods.iter().find(|m| m.node.name.node == "get_x").unwrap().node.id;
    let orig_get_y_id = orig_class.methods.iter().find(|m| m.node.name.node == "get_y").unwrap().node.id;
    let orig_self_param_id = orig_class.methods.iter()
        .find(|m| m.node.name.node == "get_x").unwrap()
        .node.params.iter().find(|p| p.name.node == "self").unwrap().id;

    // Generate .pt and sync back
    let data = std::fs::read(&pluto_path).unwrap();
    let (program, _source, _derived) = deserialize_program(&data).unwrap();
    let pt_text = pretty_print(&program);
    let synced = sync_and_load(&pt_text, &pluto_path);

    let synced_class = &synced.classes[0].node;
    assert_eq!(synced_class.id, orig_class_id);
    assert_eq!(synced_class.fields.iter().find(|f| f.name.node == "x").unwrap().id, orig_x_id);
    assert_eq!(synced_class.fields.iter().find(|f| f.name.node == "y").unwrap().id, orig_y_id);
    assert_eq!(synced_class.methods.iter().find(|m| m.node.name.node == "get_x").unwrap().node.id, orig_get_x_id);
    assert_eq!(synced_class.methods.iter().find(|m| m.node.name.node == "get_y").unwrap().node.id, orig_get_y_id);
    assert_eq!(
        synced_class.methods.iter()
            .find(|m| m.node.name.node == "get_x").unwrap()
            .node.params.iter().find(|p| p.name.node == "self").unwrap().id,
        orig_self_param_id
    );
}

#[test]
fn nested_uuid_preservation_enum_variants() {
    let source = r#"enum Color {
    Red
    Green
    Blue
}

fn main() {
    let c = Color.Red
    print(0)
}
"#;
    let (dir, original) = emit_ast(source);
    let pluto_path = dir.path().join("test.pluto");

    let orig_enum = &original.enums[0].node;
    let orig_enum_id = orig_enum.id;
    let orig_red_id = orig_enum.variants.iter().find(|v| v.name.node == "Red").unwrap().id;
    let orig_green_id = orig_enum.variants.iter().find(|v| v.name.node == "Green").unwrap().id;
    let orig_blue_id = orig_enum.variants.iter().find(|v| v.name.node == "Blue").unwrap().id;

    // Round-trip
    let data = std::fs::read(&pluto_path).unwrap();
    let (program, _source, _derived) = deserialize_program(&data).unwrap();
    let pt_text = pretty_print(&program);
    let synced = sync_and_load(&pt_text, &pluto_path);

    let synced_enum = &synced.enums[0].node;
    assert_eq!(synced_enum.id, orig_enum_id);
    assert_eq!(synced_enum.variants.iter().find(|v| v.name.node == "Red").unwrap().id, orig_red_id);
    assert_eq!(synced_enum.variants.iter().find(|v| v.name.node == "Green").unwrap().id, orig_green_id);
    assert_eq!(synced_enum.variants.iter().find(|v| v.name.node == "Blue").unwrap().id, orig_blue_id);
}

#[test]
fn no_pluto_file_creates_fresh() {
    let dir = tempfile::tempdir().unwrap();
    let pluto_path = dir.path().join("fresh.pluto");
    let pt_path = dir.path().join("fresh.pt");

    let source = "fn main() {\n    print(42)\n}\n";
    std::fs::write(&pt_path, source).unwrap();

    let result = sync_pt_to_pluto(&pt_path, &pluto_path).unwrap();

    assert!(result.added.contains(&"fn main".to_string()));
    assert!(result.removed.is_empty());
    assert_eq!(result.unchanged, 0);

    // Verify the output is valid
    let data = std::fs::read(&pluto_path).unwrap();
    assert!(is_binary_format(&data));
    let (program, _, _) = deserialize_program(&data).unwrap();
    assert_eq!(program.functions.len(), 1);
    assert_eq!(program.functions[0].node.name.node, "main");
}

#[test]
fn cross_reference_resolution() {
    let source = r#"error MyError {
    msg: string
}

fn might_fail() {
    raise MyError { msg: "oops" }
}

fn main() {
    might_fail() catch e {
        print(0)
    }
}
"#;
    let (dir, _original) = emit_ast(source);
    let pluto_path = dir.path().join("test.pluto");

    // Generate .pt and sync back
    let data = std::fs::read(&pluto_path).unwrap();
    let (program, _source, _derived) = deserialize_program(&data).unwrap();
    let pt_text = pretty_print(&program);
    let synced = sync_and_load(&pt_text, &pluto_path);

    // Cross-references should be resolved: the error should have an ID
    let error_id = error_uuid(&synced, "MyError");
    assert_ne!(error_id, Uuid::nil());

    // The might_fail function should have a target_id on its raise statement
    // (checking that xref::resolve_cross_refs ran)
    let might_fail = synced.functions.iter().find(|f| f.node.name.node == "might_fail").unwrap();
    let has_raise = might_fail.node.body.node.stmts.iter().any(|stmt| {
        matches!(&stmt.node, plutoc::parser::ast::Stmt::Raise { .. })
    });
    assert!(has_raise);
}

#[test]
fn trait_uuid_preservation() {
    let source = r#"trait Greeter {
    fn greet(self) string
}

class Hello impl Greeter {
    name: string

    fn greet(self) string {
        return self.name
    }
}

fn main() {
    let h = Hello { name: "world" }
    print(0)
}
"#;
    let (dir, original) = emit_ast(source);
    let pluto_path = dir.path().join("test.pluto");

    let orig_trait_id = trait_uuid(&original, "Greeter");
    let orig_class_id = class_uuid(&original, "Hello");
    let orig_trait_method_id = original.traits[0].node.methods[0].id;

    // Round-trip
    let data = std::fs::read(&pluto_path).unwrap();
    let (program, _source, _derived) = deserialize_program(&data).unwrap();
    let pt_text = pretty_print(&program);
    let synced = sync_and_load(&pt_text, &pluto_path);

    assert_eq!(trait_uuid(&synced, "Greeter"), orig_trait_id);
    assert_eq!(class_uuid(&synced, "Hello"), orig_class_id);
    assert_eq!(synced.traits[0].node.methods[0].id, orig_trait_method_id);
}

#[test]
fn error_decl_uuid_preservation() {
    let source = r#"error NotFound {
    msg: string
}

error Forbidden {
    reason: string
}

fn main() {
    print(0)
}
"#;
    let (dir, original) = emit_ast(source);
    let pluto_path = dir.path().join("test.pluto");

    let orig_nf_id = error_uuid(&original, "NotFound");
    let orig_fb_id = error_uuid(&original, "Forbidden");
    let orig_nf_field_id = original.errors.iter()
        .find(|e| e.node.name.node == "NotFound").unwrap()
        .node.fields[0].id;

    // Round-trip
    let data = std::fs::read(&pluto_path).unwrap();
    let (program, _source, _derived) = deserialize_program(&data).unwrap();
    let pt_text = pretty_print(&program);
    let synced = sync_and_load(&pt_text, &pluto_path);

    assert_eq!(error_uuid(&synced, "NotFound"), orig_nf_id);
    assert_eq!(error_uuid(&synced, "Forbidden"), orig_fb_id);
    // Field UUID also preserved
    assert_eq!(
        synced.errors.iter().find(|e| e.node.name.node == "NotFound").unwrap().node.fields[0].id,
        orig_nf_field_id
    );
}

#[test]
fn multiple_syncs_preserve_uuids() {
    let source = "fn alpha() {\n    print(1)\n}\n\nfn beta() {\n    print(2)\n}\n";
    let (dir, original) = emit_ast(source);
    let pluto_path = dir.path().join("test.pluto");

    let alpha_uuid = fn_uuid(&original, "alpha");
    let beta_uuid = fn_uuid(&original, "beta");

    // Sync 1: add a function
    let v2 = "fn alpha() {\n    print(1)\n}\n\nfn beta() {\n    print(2)\n}\n\nfn gamma() {\n    print(3)\n}\n";
    let synced1 = sync_and_load(v2, &pluto_path);
    assert_eq!(fn_uuid(&synced1, "alpha"), alpha_uuid);
    assert_eq!(fn_uuid(&synced1, "beta"), beta_uuid);
    let gamma_uuid = fn_uuid(&synced1, "gamma");

    // Sync 2: remove beta
    let v3 = "fn alpha() {\n    print(1)\n}\n\nfn gamma() {\n    print(3)\n}\n";
    let synced2 = sync_and_load(v3, &pluto_path);
    assert_eq!(fn_uuid(&synced2, "alpha"), alpha_uuid);
    assert_eq!(fn_uuid(&synced2, "gamma"), gamma_uuid);
    assert!(synced2.functions.iter().all(|f| f.node.name.node != "beta"));

    // Sync 3: add beta back — should get a NEW UUID (not the old one)
    let v4 = "fn alpha() {\n    print(1)\n}\n\nfn gamma() {\n    print(3)\n}\n\nfn beta() {\n    print(4)\n}\n";
    let synced3 = sync_and_load(v4, &pluto_path);
    assert_eq!(fn_uuid(&synced3, "alpha"), alpha_uuid);
    assert_eq!(fn_uuid(&synced3, "gamma"), gamma_uuid);
    // beta was deleted then re-added, so it should NOT have the original UUID
    assert_ne!(fn_uuid(&synced3, "beta"), beta_uuid);
}

#[test]
fn source_text_stored_in_binary() {
    let source = "fn main() {\n    print(42)\n}\n";
    let dir = tempfile::tempdir().unwrap();
    let pluto_path = dir.path().join("test.pluto");
    let pt_path = dir.path().join("test.pt");
    std::fs::write(&pt_path, source).unwrap();

    sync_pt_to_pluto(&pt_path, &pluto_path).unwrap();

    let data = std::fs::read(&pluto_path).unwrap();
    let (_program, stored_source, _derived) = deserialize_program(&data).unwrap();
    assert_eq!(stored_source, source);
}

#[test]
fn empty_derived_data() {
    let source = "fn main() {\n    print(42)\n}\n";
    let dir = tempfile::tempdir().unwrap();
    let pluto_path = dir.path().join("test.pluto");
    let pt_path = dir.path().join("test.pt");
    std::fs::write(&pt_path, source).unwrap();

    sync_pt_to_pluto(&pt_path, &pluto_path).unwrap();

    let data = std::fs::read(&pluto_path).unwrap();
    let (_program, _source, derived) = deserialize_program(&data).unwrap();
    assert!(derived.fn_error_sets.is_empty());
    assert!(derived.fn_signatures.is_empty());
}
