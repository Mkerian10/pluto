pub mod decl;
pub mod editor;
pub mod error;
pub mod index;
pub mod module;
pub mod xref;

pub use decl::{DeclKind, DeclRef};
pub use editor::{ModuleEditor, DeleteResult, DanglingRef};
pub use error::SdkError;
pub use module::Module;

// Re-export key plutoc types for convenience
pub use plutoc::derived::{
    DerivedInfo, ErrorRef, ResolvedSignature,
    ResolvedClassInfo, ResolvedTraitInfo, ResolvedEnumInfo,
    ResolvedErrorInfo, ResolvedFieldInfo, ResolvedVariantInfo,
};
pub use plutoc::parser::ast::Program;
pub use plutoc::span::{Span, Spanned};
pub use plutoc::typeck::types::PlutoType;

#[cfg(test)]
mod tests {
    use super::*;
    use plutoc::binary::serialize_program;
    use plutoc::derived::DerivedInfo;
    use plutoc::lexer;
    use plutoc::parser::Parser;
    use plutoc::span::{Span as PlutoSpan, Spanned as PlutoSpanned};
    use plutoc::parser::ast::*;
    use uuid::Uuid;

    fn sp<T>(node: T) -> PlutoSpanned<T> {
        PlutoSpanned::new(node, PlutoSpan::dummy())
    }

    fn empty_block() -> PlutoSpanned<Block> {
        sp(Block { stmts: vec![] })
    }

    fn make_function(name: &str) -> PlutoSpanned<Function> {
        sp(Function {
            id: Uuid::new_v4(),
            name: sp(name.to_string()),
            type_params: vec![],
            type_param_bounds: std::collections::HashMap::new(),
            params: vec![],
            return_type: None,
            contracts: vec![],
            body: empty_block(),
            is_pub: false,
            is_override: false,
            is_generator: false,
        })
    }

    fn empty_program() -> Program {
        Program {
            imports: vec![],
            functions: vec![],
            extern_fns: vec![],
            classes: vec![],
            traits: vec![],
            enums: vec![],
            app: None,
            stages: vec![],
            system: None,
            errors: vec![],
            test_info: vec![],
            tests: None,
            fallible_extern_fns: vec![],
        }
    }

    fn empty_derived() -> DerivedInfo {
        DerivedInfo::default()
    }

    /// Parse a source string into a Program (no typeck).
    fn parse(source: &str) -> Program {
        let tokens = lexer::lex(source).expect("lex failed");
        let mut parser = Parser::new(&tokens, source);
        parser.parse_program().expect("parse failed")
    }

    #[test]
    fn from_bytes_round_trip() {
        let source = "fn main() {\n}\n";
        let program = parse(source);
        let derived = empty_derived();
        let bytes = serialize_program(&program, source, &derived).unwrap();
        let module = Module::from_bytes(&bytes).unwrap();
        assert_eq!(module.source(), source);
        assert_eq!(module.functions().len(), 1);
        assert_eq!(module.functions()[0].name(), "main");
    }

    #[test]
    fn get_by_uuid() {
        let source = "fn foo() {\n}\n\nfn main() {\n}\n";
        let program = parse(source);
        let foo_id = program.functions[0].node.id;
        let derived = empty_derived();
        let bytes = serialize_program(&program, source, &derived).unwrap();
        let module = Module::from_bytes(&bytes).unwrap();

        let decl = module.get(foo_id).unwrap();
        assert_eq!(decl.name(), "foo");
        assert_eq!(decl.kind(), DeclKind::Function);
        assert!(decl.as_function().is_some());
    }

    #[test]
    fn find_by_name() {
        let source = "fn foo() {\n}\n\nfn main() {\n}\n";
        let program = parse(source);
        let derived = empty_derived();
        let bytes = serialize_program(&program, source, &derived).unwrap();
        let module = Module::from_bytes(&bytes).unwrap();

        let results = module.find("foo");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind(), DeclKind::Function);

        let missing = module.find("nonexistent");
        assert!(missing.is_empty());
    }

    #[test]
    fn listing_classes_and_enums() {
        let source = r#"class Point {
    x: int
    y: int
}

enum Color {
    Red
    Green
}

fn main() {
}
"#;
        let program = parse(source);
        let derived = empty_derived();
        let bytes = serialize_program(&program, source, &derived).unwrap();
        let module = Module::from_bytes(&bytes).unwrap();

        assert_eq!(module.classes().len(), 1);
        assert_eq!(module.classes()[0].name(), "Point");
        assert_eq!(module.enums().len(), 1);
        assert_eq!(module.enums()[0].name(), "Color");
    }

    #[test]
    fn listing_traits_and_errors() {
        let source = r#"trait Printable {
    fn display(self) string
}

error NotFound {
    message: string
}

fn main() {
}
"#;
        let program = parse(source);
        let derived = empty_derived();
        let bytes = serialize_program(&program, source, &derived).unwrap();
        let module = Module::from_bytes(&bytes).unwrap();

        assert_eq!(module.traits().len(), 1);
        assert_eq!(module.traits()[0].name(), "Printable");
        assert_eq!(module.errors().len(), 1);
        assert_eq!(module.errors()[0].name(), "NotFound");
    }

    #[test]
    fn source_slice_extracts_correct_text() {
        let source = "fn foo() {\n}\n\nfn main() {\n}\n";
        let program = parse(source);
        let derived = empty_derived();
        let bytes = serialize_program(&program, source, &derived).unwrap();
        let module = Module::from_bytes(&bytes).unwrap();

        let foo_decl = module.find("foo")[0].span();
        let text = module.source_slice(foo_decl);
        assert_eq!(text, "foo");
    }

    #[test]
    fn enum_variant_lookup() {
        let source = r#"enum Color {
    Red
    Green
    Blue
}

fn main() {
}
"#;
        let program = parse(source);
        let red_id = program.enums[0].node.variants[0].id;
        let derived = empty_derived();
        let bytes = serialize_program(&program, source, &derived).unwrap();
        let module = Module::from_bytes(&bytes).unwrap();

        let red = module.get(red_id).unwrap();
        assert_eq!(red.name(), "Red");
        assert_eq!(red.kind(), DeclKind::EnumVariant);
    }

    #[test]
    fn callers_of_with_xrefs() {
        // Build a program with resolved xrefs
        let mut program = empty_program();
        let target_fn = make_function("greet");
        let target_id = target_fn.node.id;

        let call_span = PlutoSpan::new(100, 110);
        let mut caller = make_function("main");
        caller.node.body = sp(Block {
            stmts: vec![PlutoSpanned::new(Stmt::Expr(PlutoSpanned::new(Expr::Call {
                name: sp("greet".to_string()),
                args: vec![],
                type_args: vec![],
                target_id: Some(target_id),
            }, call_span)), call_span)],
        });

        program.functions.push(target_fn);
        program.functions.push(caller);

        let source = "";
        let derived = empty_derived();
        let bytes = serialize_program(&program, source, &derived).unwrap();
        let module = Module::from_bytes(&bytes).unwrap();

        let sites = module.callers_of(target_id);
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].caller.name.node, "main");
        assert_eq!(sites[0].target_id, target_id);
    }

    #[test]
    fn constructors_of_with_xrefs() {
        let mut program = empty_program();
        let class_id = Uuid::new_v4();
        program.classes.push(sp(ClassDecl {
            id: class_id,
            name: sp("Point".to_string()),
            type_params: vec![],
            type_param_bounds: std::collections::HashMap::new(),
            fields: vec![],
            methods: vec![],
            invariants: vec![],
            impl_traits: vec![],
            uses: vec![],
            is_pub: false,
            lifecycle: Lifecycle::Singleton,
        }));

        let lit_span = PlutoSpan::new(200, 220);
        let mut caller = make_function("main");
        caller.node.body = sp(Block {
            stmts: vec![PlutoSpanned::new(Stmt::Expr(PlutoSpanned::new(Expr::StructLit {
                name: sp("Point".to_string()),
                type_args: vec![],
                fields: vec![],
                target_id: Some(class_id),
            }, lit_span)), lit_span)],
        });
        program.functions.push(caller);

        let source = "";
        let derived = empty_derived();
        let bytes = serialize_program(&program, source, &derived).unwrap();
        let module = Module::from_bytes(&bytes).unwrap();

        let sites = module.constructors_of(class_id);
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].function.name.node, "main");
        assert_eq!(sites[0].target_id, class_id);
    }

    #[test]
    fn enum_usages_of_with_xrefs() {
        let mut program = empty_program();
        let enum_id = Uuid::new_v4();
        let variant_id = Uuid::new_v4();
        program.enums.push(sp(EnumDecl {
            id: enum_id,
            name: sp("Color".to_string()),
            type_params: vec![],
            type_param_bounds: std::collections::HashMap::new(),
            variants: vec![EnumVariant {
                id: variant_id,
                name: sp("Red".to_string()),
                fields: vec![],
            }],
            is_pub: false,
        }));

        let usage_span = PlutoSpan::new(300, 310);
        let mut caller = make_function("main");
        caller.node.body = sp(Block {
            stmts: vec![PlutoSpanned::new(Stmt::Expr(PlutoSpanned::new(Expr::EnumUnit {
                enum_name: sp("Color".to_string()),
                variant: sp("Red".to_string()),
                type_args: vec![],
                enum_id: Some(enum_id),
                variant_id: Some(variant_id),
            }, usage_span)), usage_span)],
        });
        program.functions.push(caller);

        let source = "";
        let derived = empty_derived();
        let bytes = serialize_program(&program, source, &derived).unwrap();
        let module = Module::from_bytes(&bytes).unwrap();

        let sites = module.enum_usages_of(enum_id);
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].enum_id, enum_id);
        assert_eq!(sites[0].variant_id, variant_id);
    }

    #[test]
    fn raise_sites_of_with_xrefs() {
        let mut program = empty_program();
        let err_id = Uuid::new_v4();
        program.errors.push(sp(ErrorDecl {
            id: err_id,
            name: sp("NotFound".to_string()),
            fields: vec![],
            is_pub: false,
        }));

        let raise_span = PlutoSpan::new(400, 420);
        let mut caller = make_function("main");
        caller.node.body = sp(Block {
            stmts: vec![PlutoSpanned::new(Stmt::Raise {
                error_name: sp("NotFound".to_string()),
                fields: vec![],
                error_id: Some(err_id),
            }, raise_span)],
        });
        program.functions.push(caller);

        let source = "";
        let derived = empty_derived();
        let bytes = serialize_program(&program, source, &derived).unwrap();
        let module = Module::from_bytes(&bytes).unwrap();

        let sites = module.raise_sites_of(err_id);
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].error_id, err_id);
    }

    #[test]
    fn builtins_not_in_callers() {
        // Calls to builtins (target_id: None) should not appear in any callers list
        let mut program = empty_program();
        let mut caller = make_function("main");
        caller.node.body = sp(Block {
            stmts: vec![sp(Stmt::Expr(sp(Expr::Call {
                name: sp("print".to_string()),
                args: vec![],
                type_args: vec![],
                target_id: None,
            })))],
        });
        program.functions.push(caller);

        let source = "";
        let derived = empty_derived();
        let bytes = serialize_program(&program, source, &derived).unwrap();
        let module = Module::from_bytes(&bytes).unwrap();

        // No function should have callers (print has no target_id)
        for f in module.functions() {
            assert!(module.callers_of(f.id()).is_empty());
        }
    }

    #[test]
    fn app_declaration() {
        let mut program = empty_program();
        program.app = Some(sp(AppDecl {
            id: Uuid::new_v4(),
            name: sp("MyApp".to_string()),
            inject_fields: vec![],
            ambient_types: vec![],
            lifecycle_overrides: vec![],
            methods: vec![make_function("main")],
        }));

        let source = "";
        let derived = empty_derived();
        let bytes = serialize_program(&program, source, &derived).unwrap();
        let module = Module::from_bytes(&bytes).unwrap();

        let app = module.app().unwrap();
        assert_eq!(app.name(), "MyApp");
        assert_eq!(app.kind(), DeclKind::App);
    }

    #[test]
    fn raw_program_access() {
        let source = "fn main() {\n}\n";
        let program = parse(source);
        let derived = empty_derived();
        let bytes = serialize_program(&program, source, &derived).unwrap();
        let module = Module::from_bytes(&bytes).unwrap();

        let prog = module.program();
        assert_eq!(prog.functions.len(), 1);
    }

    // =======================================================================
    // Editor tests
    // =======================================================================

    #[test]
    fn from_source_creates_module() {
        let source = "fn foo() int {\n    return 42\n}\n\nfn main() {\n    foo()\n}\n";
        let module = Module::from_source(source).unwrap();
        assert_eq!(module.functions().len(), 2);
        assert_eq!(module.functions()[0].name(), "foo");
        assert_eq!(module.functions()[1].name(), "main");
    }

    #[test]
    fn add_function_from_source() {
        let source = "fn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let mut editor = module.edit();

        let id = editor.add_from_source("fn greet() {\n    print(\"hello\")\n}\n").unwrap();

        let module = editor.commit();
        assert_eq!(module.functions().len(), 2);

        let decl = module.find("greet");
        assert_eq!(decl.len(), 1);
        assert_eq!(decl[0].id(), id);
        assert!(module.source().contains("fn greet()"));
    }

    #[test]
    fn add_class_from_source() {
        let source = "fn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let mut editor = module.edit();

        let id = editor.add_from_source("class Point {\n    x: int\n    y: int\n}\n").unwrap();

        let module = editor.commit();
        assert_eq!(module.classes().len(), 1);
        assert_eq!(module.classes()[0].name(), "Point");
        assert_eq!(module.classes()[0].id(), id);
        assert!(module.source().contains("class Point"));
    }

    #[test]
    fn add_enum_from_source() {
        let source = "fn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let mut editor = module.edit();

        let id = editor.add_from_source("enum Color {\n    Red\n    Green\n    Blue\n}\n").unwrap();

        let module = editor.commit();
        assert_eq!(module.enums().len(), 1);
        assert_eq!(module.enums()[0].name(), "Color");
        assert_eq!(module.enums()[0].id(), id);
    }

    #[test]
    fn add_trait_from_source() {
        let source = "fn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let mut editor = module.edit();

        let id = editor.add_from_source("trait Printable {\n    fn display(self) string\n}\n").unwrap();

        let module = editor.commit();
        assert_eq!(module.traits().len(), 1);
        assert_eq!(module.traits()[0].name(), "Printable");
        assert_eq!(module.traits()[0].id(), id);
    }

    #[test]
    fn add_error_from_source() {
        let source = "fn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let mut editor = module.edit();

        let id = editor.add_from_source("error NotFound {\n    message: string\n}\n").unwrap();

        let module = editor.commit();
        assert_eq!(module.errors().len(), 1);
        assert_eq!(module.errors()[0].name(), "NotFound");
        assert_eq!(module.errors()[0].id(), id);
    }

    #[test]
    fn add_method_to_class() {
        let source = "class Greeter {\n    name: string\n}\n\nfn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let class_id = module.classes()[0].id();
        let mut editor = module.edit();

        let method_id = editor.add_method_from_source(class_id, "fn greet(self) string {\n    return self.name\n}\n").unwrap();

        let module = editor.commit();
        // Verify the method exists through the raw program
        let cls = &module.program().classes[0].node;
        assert_eq!(cls.methods.len(), 1);
        assert_eq!(cls.methods[0].node.name.node, "greet");
        assert_eq!(cls.methods[0].node.id, method_id);
    }

    #[test]
    fn add_field_to_class() {
        let source = "class Point {\n    x: int\n}\n\nfn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let class_id = module.classes()[0].id();
        let mut editor = module.edit();

        let field_id = editor.add_field(class_id, "y", "int").unwrap();

        let module = editor.commit();
        let cls = &module.program().classes[0].node;
        assert_eq!(cls.fields.len(), 2);
        assert_eq!(cls.fields[1].name.node, "y");
        assert_eq!(cls.fields[1].id, field_id);
        assert!(module.source().contains("y: int"));
    }

    #[test]
    fn replace_function_preserves_uuid() {
        let source = "fn greet() {\n    print(\"hello\")\n}\n\nfn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let greet_id = module.find("greet")[0].id();
        let mut editor = module.edit();

        editor.replace_from_source(greet_id, "fn greet() {\n    print(\"goodbye\")\n}\n").unwrap();

        let module = editor.commit();
        // UUID preserved
        let decl = module.find("greet");
        assert_eq!(decl.len(), 1);
        assert_eq!(decl[0].id(), greet_id);
        // New body reflected in source
        assert!(module.source().contains("goodbye"));
        assert!(!module.source().contains("hello"));
    }

    #[test]
    fn replace_class_matches_nested_uuids() {
        let source = "class Point {\n    x: int\n    y: int\n}\n\nfn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let class_id = module.classes()[0].id();
        let old_x_id = module.program().classes[0].node.fields[0].id;
        let old_y_id = module.program().classes[0].node.fields[1].id;
        let mut editor = module.edit();

        // Replace with same fields (UUIDs should be preserved) plus a new field
        editor.replace_from_source(class_id, "class Point {\n    x: int\n    y: int\n    z: float\n}\n").unwrap();

        let module = editor.commit();
        assert_eq!(module.classes()[0].id(), class_id);
        let fields = &module.program().classes[0].node.fields;
        assert_eq!(fields.len(), 3);
        // x and y should keep their UUIDs
        assert_eq!(fields[0].id, old_x_id);
        assert_eq!(fields[1].id, old_y_id);
        // z should have a new UUID (not matching any old field)
        assert_ne!(fields[2].id, old_x_id);
        assert_ne!(fields[2].id, old_y_id);
    }

    #[test]
    fn replace_kind_mismatch_errors() {
        let source = "fn greet() {\n}\n\nfn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let greet_id = module.find("greet")[0].id();
        let mut editor = module.edit();

        // Try to replace a function with a class
        let result = editor.replace_from_source(greet_id, "class Greeter {\n    name: string\n}\n");
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("function"), "error should mention function: {}", err_msg);
    }

    #[test]
    fn delete_function() {
        let source = "fn greet() {\n    print(\"hello\")\n}\n\nfn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let greet_id = module.find("greet")[0].id();
        let mut editor = module.edit();

        let result = editor.delete(greet_id).unwrap();
        assert!(result.source.contains("greet"));

        let module = editor.commit();
        assert_eq!(module.functions().len(), 1);
        assert_eq!(module.functions()[0].name(), "main");
        assert!(!module.source().contains("greet"));
    }

    #[test]
    fn delete_reports_dangling_refs() {
        let source = "fn greet() {\n    print(\"hello\")\n}\n\nfn main() {\n    greet()\n}\n";
        let module = Module::from_source(source).unwrap();
        let greet_id = module.find("greet")[0].id();
        let mut editor = module.edit();

        let result = editor.delete(greet_id).unwrap();
        // After deletion, the call to greet() in main still has target_id pointing to deleted UUID
        assert_eq!(result.dangling.len(), 1);
        assert_eq!(result.dangling[0].name, "greet");
    }

    #[test]
    fn rename_function_updates_call_sites() {
        let source = "fn greet() {\n    print(\"hello\")\n}\n\nfn main() {\n    greet()\n}\n";
        let module = Module::from_source(source).unwrap();
        let greet_id = module.find("greet")[0].id();
        let mut editor = module.edit();

        editor.rename(greet_id, "hello").unwrap();

        let module = editor.commit();
        // Declaration renamed
        assert!(module.find("greet").is_empty());
        let decls = module.find("hello");
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].id(), greet_id);

        // Source reflects the rename
        assert!(module.source().contains("fn hello()"));
        // Call site should be updated (checked via source after pretty-print)
        assert!(module.source().contains("hello()"));
        assert!(!module.source().contains("greet"));
    }

    #[test]
    fn rename_class_updates_struct_lits_and_types() {
        let source = "class Point {\n    x: int\n    y: int\n}\n\nfn make() Point {\n    return Point { x: 1, y: 2 }\n}\n\nfn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let point_id = module.classes()[0].id();
        let mut editor = module.edit();

        editor.rename(point_id, "Vec2").unwrap();

        let module = editor.commit();
        assert!(module.find("Point").is_empty());
        assert_eq!(module.find("Vec2").len(), 1);

        let src = module.source();
        assert!(src.contains("class Vec2"));
        assert!(src.contains("Vec2 {"));
        // Return type should be updated
        assert!(src.contains("Vec2"));
        assert!(!src.contains("Point"));
    }

    #[test]
    fn rename_enum_updates_usages() {
        let source = "enum Color {\n    Red\n    Green\n}\n\nfn main() {\n    let c = Color.Red\n}\n";
        let module = Module::from_source(source).unwrap();
        let color_id = module.enums()[0].id();
        let mut editor = module.edit();

        editor.rename(color_id, "Hue").unwrap();

        let module = editor.commit();
        let src = module.source();
        assert!(src.contains("enum Hue"));
        assert!(src.contains("Hue.Red"));
        assert!(!src.contains("Color"));
    }

    #[test]
    fn commit_rebuilds_source_and_index() {
        let source = "fn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let mut editor = module.edit();

        editor.add_from_source("fn foo() int {\n    return 42\n}\n").unwrap();

        let module = editor.commit();
        // Source regenerated
        assert!(module.source().contains("fn foo()"));
        assert!(module.source().contains("fn main()"));

        // Index works for the new function
        let foo = module.find("foo");
        assert_eq!(foo.len(), 1);
        assert_eq!(foo[0].kind(), DeclKind::Function);
    }

    #[test]
    fn commit_resolves_xrefs() {
        let source = "fn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let mut editor = module.edit();

        let foo_id = editor.add_from_source("fn foo() {\n}\n").unwrap();

        // Add a function that calls foo
        editor.add_from_source("fn bar() {\n    foo()\n}\n").unwrap();

        let module = editor.commit();
        // After commit, xrefs should be resolved: callers_of(foo_id) should find bar
        let callers = module.callers_of(foo_id);
        assert_eq!(callers.len(), 1);
    }

    #[test]
    fn multiple_edits_then_commit() {
        let source = "fn greet() {\n    print(\"hello\")\n}\n\nfn main() {\n    greet()\n}\n";
        let module = Module::from_source(source).unwrap();
        let greet_id = module.find("greet")[0].id();
        let mut editor = module.edit();

        // Add a new function
        let _helper_id = editor.add_from_source("fn helper() {\n}\n").unwrap();

        // Rename greet to hello
        editor.rename(greet_id, "hello").unwrap();

        let module = editor.commit();
        assert_eq!(module.functions().len(), 3); // main + hello + helper
        assert!(module.find("greet").is_empty());
        assert_eq!(module.find("hello").len(), 1);
        assert_eq!(module.find("helper").len(), 1);
    }

    #[test]
    fn parse_error_returns_sdk_error() {
        let source = "fn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let mut editor = module.edit();

        let result = editor.add_from_source("fn {{{ invalid");
        assert!(result.is_err());
    }

    #[test]
    fn uuid_not_found_returns_error() {
        let source = "fn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let mut editor = module.edit();

        let fake_id = Uuid::new_v4();
        assert!(editor.replace_from_source(fake_id, "fn foo() {\n}\n").is_err());
        assert!(editor.delete(fake_id).is_err());
        assert!(editor.rename(fake_id, "new_name").is_err());
    }

    #[test]
    fn source_slice_bounds_check() {
        let source = "fn main() {\n}\n";
        let module = Module::from_source(source).unwrap();

        // Out-of-bounds span should return empty string, not panic
        let stale_span = Span::new(9999, 10005);
        assert_eq!(module.source_slice(stale_span), "");
    }

    #[test]
    fn add_multiple_declarations_rejected() {
        let source = "fn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let mut editor = module.edit();

        let result = editor.add_from_source("fn a() {\n}\n\nfn b() {\n}\n");
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("multiple"));
    }

    #[test]
    fn add_method_to_non_class_errors() {
        let source = "fn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let main_id = module.functions()[0].id();
        let mut editor = module.edit();

        // main is a function, not a class
        let result = editor.add_method_from_source(main_id, "fn foo(self) {\n}\n");
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("class"));
    }

    #[test]
    fn add_field_to_non_class_errors() {
        let source = "fn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let main_id = module.functions()[0].id();
        let mut editor = module.edit();

        let result = editor.add_field(main_id, "x", "int");
        assert!(result.is_err());
    }

    #[test]
    fn rename_error_updates_raise_sites() {
        let source = "error NotFound {\n    message: string\n}\n\nfn main() {\n    raise NotFound { message: \"gone\" }\n}\n";
        let module = Module::from_source(source).unwrap();
        let err_id = module.errors()[0].id();
        let mut editor = module.edit();

        editor.rename(err_id, "Missing").unwrap();

        let module = editor.commit();
        let src = module.source();
        assert!(src.contains("error Missing"));
        assert!(src.contains("raise Missing"));
        assert!(!src.contains("NotFound"));
    }

    #[test]
    fn rename_trait_updates_impl_traits() {
        // In Pluto, `impl Trait` goes before the class body brace: `class Foo impl Printable {`
        let source = "trait Printable {\n    fn display(self) string\n}\n\nclass Foo impl Printable {\n    val: int\n\n    fn display(self) string {\n        return \"foo\"\n    }\n}\n\nfn main() {\n}\n";
        let module = Module::from_source(source).unwrap();
        let trait_id = module.traits()[0].id();
        let mut editor = module.edit();

        editor.rename(trait_id, "Displayable").unwrap();

        let module = editor.commit();
        let src = module.source();
        assert!(src.contains("trait Displayable"));
        assert!(src.contains("impl Displayable"));
        assert!(!src.contains("Printable"));
    }
}
