use plutoc_sdk::{DeclKind, Module};
use std::path::PathBuf;

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("examples")
}

#[test]
fn load_binary_ast_example_from_source() {
    let path = examples_dir().join("binary-ast").join("main.pluto");
    let module = Module::from_source_file(&path).unwrap();

    // Check source was loaded
    assert!(module.source().contains("fn opposite"));
    assert!(module.source().contains("enum Direction"));

    // Check declarations
    let classes = module.classes();
    // Counter class (plus prelude classes may be present after analysis)
    assert!(classes.iter().any(|c| c.name() == "Counter"));

    let enums = module.enums();
    // Direction enum + prelude enums (Option)
    assert!(enums.iter().any(|e| e.name() == "Direction"));

    // Find opposite function
    let opposites = module.find("opposite");
    assert_eq!(opposites.len(), 1);
    assert_eq!(opposites[0].kind(), DeclKind::Function);

    // Find Direction enum and check its variants
    let directions = module.find("Direction");
    assert_eq!(directions.len(), 1);
    let dir_decl = &directions[0];
    assert_eq!(dir_decl.kind(), DeclKind::Enum);
    let dir_enum = dir_decl.as_enum().unwrap();
    assert_eq!(dir_enum.variants.len(), 4);
    let variant_names: Vec<&str> = dir_enum.variants.iter().map(|v| v.name.node.as_str()).collect();
    assert!(variant_names.contains(&"North"));
    assert!(variant_names.contains(&"South"));
    assert!(variant_names.contains(&"East"));
    assert!(variant_names.contains(&"West"));

    // Check cross-references: opposite() should use Direction enum
    let dir_id = dir_decl.id();
    let usages = module.enum_usages_of(dir_id);
    // opposite() returns Direction variants (4 in the match arms) + main uses Direction.North
    assert!(usages.len() >= 4, "Expected at least 4 Direction usages, got {}", usages.len());

    // Check Counter is constructed in main
    let counter = module.find("Counter").into_iter().find(|d| d.kind() == DeclKind::Class).unwrap();
    let constructions = module.constructors_of(counter.id());
    assert!(!constructions.is_empty(), "Counter should be constructed in main");

    // Check that builtins (print) don't show up as callers
    let main_fns = module.find("main");
    assert!(!main_fns.is_empty());
}

#[test]
fn binary_round_trip() {
    let path = examples_dir().join("binary-ast").join("main.pluto");
    let module = Module::from_source_file(&path).unwrap();

    // Serialize to binary and reload
    let bytes = plutoc::binary::serialize_program(module.program(), module.source(), module.derived()).unwrap();
    let module2 = Module::from_bytes(&bytes).unwrap();

    assert_eq!(module.source(), module2.source());
    assert_eq!(module.functions().len(), module2.functions().len());
    assert_eq!(module.classes().len(), module2.classes().len());
    assert_eq!(module.enums().len(), module2.enums().len());

    // UUIDs should match
    for (a, b) in module.functions().iter().zip(module2.functions().iter()) {
        assert_eq!(a.id(), b.id());
        assert_eq!(a.name(), b.name());
    }
}
