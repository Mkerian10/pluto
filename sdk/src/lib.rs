pub mod decl;
pub mod error;
pub mod index;
pub mod module;
pub mod xref;

pub use decl::{DeclKind, DeclRef};
pub use error::SdkError;
pub use module::Module;

// Re-export key plutoc types for convenience
pub use plutoc::derived::{DerivedInfo, ErrorRef, ResolvedSignature};
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
            params: vec![],
            return_type: None,
            contracts: vec![],
            body: empty_block(),
            is_pub: false,
        })
    }

    fn empty_program() -> Program {
        Program {
            imports: vec![],
            functions: vec![],
            extern_fns: vec![],
            extern_rust_crates: vec![],
            classes: vec![],
            traits: vec![],
            enums: vec![],
            app: None,
            errors: vec![],
            test_info: vec![],
            fallible_extern_fns: vec![],
        }
    }

    fn empty_derived() -> DerivedInfo {
        DerivedInfo {
            fn_error_sets: Default::default(),
            fn_signatures: Default::default(),
        }
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
            fields: vec![],
            methods: vec![],
            invariants: vec![],
            impl_traits: vec![],
            uses: vec![],
            is_pub: false,
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
}
