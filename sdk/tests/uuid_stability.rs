//! UUID stability guarantees (#214, #215) and add_from_source data
//! preservation (#218).
//!
//! The stable-UUID promise: a declaration's UUID survives rename and replace,
//! and survives a *binary* round-trip (`to_bytes`/`from_bytes`). A plain-text
//! round-trip (`from_source` on pretty-printed output) necessarily assigns
//! fresh UUIDs — text carries no identity; that is what the .pluto binary
//! format is for.

use pluto_sdk::Module;

#[test]
fn rename_preserves_uuid_through_commit() {
    let module = Module::from_source(
        "fn greet(name: string) string {\n    return name\n}\n",
    )
    .expect("parse");
    let id = module.find("greet")[0].id();

    let mut editor = module.edit();
    editor.rename(id, "salute").expect("rename");
    let module = editor.commit();

    let decl = module.get(id).expect("UUID must survive rename");
    assert_eq!(decl.name(), "salute");
    assert!(module.find("greet").is_empty());
}

#[test]
fn replace_preserves_uuid_through_commit() {
    let module = Module::from_source(
        "fn double(x: int) int {\n    return x * 2\n}\n",
    )
    .expect("parse");
    let id = module.find("double")[0].id();

    let mut editor = module.edit();
    editor
        .replace_from_source(id, "fn double(x: int) int {\n    return x + x\n}\n")
        .expect("replace");
    let module = editor.commit();

    let decl = module.get(id).expect("UUID must survive replace");
    assert_eq!(decl.name(), "double");
    assert!(module.source().contains("x + x"));
}

#[test]
fn rename_class_preserves_uuid_and_updates_references() {
    let module = Module::from_source(
        "class Point {\n    x: int\n}\n\nfn origin() Point {\n    return Point { x: 0 }\n}\n",
    )
    .expect("parse");
    let id = module.find("Point")[0].id();

    let mut editor = module.edit();
    editor.rename(id, "Coord").expect("rename");
    let module = editor.commit();

    let decl = module.get(id).expect("UUID must survive class rename");
    assert_eq!(decl.name(), "Coord");
    assert!(module.source().contains("fn origin() Coord"));
    assert!(module.source().contains("Coord { x: 0 }"));
}

#[test]
fn uuid_survives_binary_round_trip() {
    let module = Module::from_source(
        "fn keep(x: int) int {\n    return x\n}\n",
    )
    .expect("parse");
    let id = module.find("keep")[0].id();

    let bytes = module.to_bytes().expect("serialize");
    let reloaded = Module::from_bytes(&bytes).expect("deserialize");

    let decl = reloaded
        .get(id)
        .expect("UUID must survive binary round-trip");
    assert_eq!(decl.name(), "keep");
}

#[test]
fn uuid_survives_edit_then_binary_round_trip() {
    let module = Module::from_source(
        "fn double(x: int) int {\n    return x * 2\n}\n",
    )
    .expect("parse");
    let id = module.find("double")[0].id();

    let mut editor = module.edit();
    editor
        .replace_from_source(id, "fn double(x: int) int {\n    return x + x\n}\n")
        .expect("replace");
    let module = editor.commit();

    let bytes = module.to_bytes().expect("serialize");
    let reloaded = Module::from_bytes(&bytes).expect("deserialize");
    assert!(reloaded.get(id).is_some(), "UUID stable through edit + binary round-trip");
}

// ── add_from_source data preservation (#218) ────────────────────────────────

#[test]
fn add_from_source_keeps_imports() {
    let module = Module::from_source("fn base() int {\n    return 1\n}\n").expect("parse");

    let mut editor = module.edit();
    editor
        .add_from_source("import math\n\nfn use_math(x: int) int {\n    return x\n}\n")
        .expect("add");
    let module = editor.commit();

    assert!(
        module.source().contains("import math"),
        "import must survive add_from_source; got:\n{}",
        module.source()
    );
}

#[test]
fn add_from_source_keeps_test_blocks() {
    let module = Module::from_source("fn base() int {\n    return 1\n}\n").expect("parse");

    let mut editor = module.edit();
    editor
        .add_from_source("test \"base works\" {\n    expect(base()).to_equal(1)\n}\n")
        .expect("add test block");
    let module = editor.commit();

    assert!(
        module.source().contains("test \"base works\""),
        "test block must round-trip through add_from_source; got:\n{}",
        module.source()
    );
    assert!(
        !module.source().contains("fn __test_"),
        "test must not degrade to a bare __test_N function; got:\n{}",
        module.source()
    );
}
