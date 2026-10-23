use pluto_sdk::{DeclKind, Module};
use std::path::PathBuf;

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("examples")
}

#[test]
fn load_binary_ast_example_from_source() {
    let path = examples_dir().join("binary-ast").join("main.pt");
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
    let path = examples_dir().join("binary-ast").join("main.pt");
    let module = Module::from_source_file(&path).unwrap();

    // Serialize to binary and reload
    let bytes = pluto::binary::serialize_program(module.program(), module.source(), module.derived()).unwrap();
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

// Nested declarations (fields, params, app methods) are indexed by UUID, and
// `get`/`find` must resolve them to the right declaration.
#[test]
fn get_resolves_fields_params_and_app_methods() {
    let src = r#"
error NotFound {
    key: string
}

class Store {
    count: int

    fn size(self) int {
        return self.count
    }
}

app Main {
    fn main(self) {
        print(helper(1))
    }

    fn extra(self, flag: bool) {
    }
}

fn helper(amount: int) int {
    return amount
}
"#;
    let module = Module::from_source(src).unwrap();
    let program = module.program();

    let store = &program.classes[0].node;
    let field = module.get(store.fields[0].id).expect("class field resolves");
    assert_eq!((field.kind(), field.name()), (DeclKind::Field, "count"));

    let err_field = module.get(program.errors[0].node.fields[0].id).expect("error field resolves");
    assert_eq!(err_field.name(), "key");

    let helper = &program.functions[0].node;
    let param = module.get(helper.params[0].id).expect("function param resolves");
    assert_eq!((param.kind(), param.name()), (DeclKind::Param, "amount"));

    // An app method shares parent index 0 with class 0's methods; it must
    // resolve to the app method, not Store's method at the same position.
    let app = &program.app.as_ref().unwrap().node;
    for method in &app.methods {
        let found = module.get(method.node.id).expect("app method resolves");
        assert_eq!(found.name(), method.node.name.node);
    }
    let flag = app.methods[1].node.params.iter().find(|p| p.name.node == "flag").unwrap();
    assert_eq!(module.get(flag.id).expect("app method param resolves").name(), "flag");

    // find() no longer drops fields and params.
    assert!(module.find("count").iter().any(|d| d.kind() == DeclKind::Field));
    assert!(module.find("amount").iter().any(|d| d.kind() == DeclKind::Param));
}
