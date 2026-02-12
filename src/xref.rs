use std::collections::HashMap;
use uuid::Uuid;

use crate::parser::ast::*;
use crate::span::Spanned;
use crate::typeck::env::mangle_method;
use crate::visit::{walk_expr_mut, walk_stmt_mut, VisitMut};

/// Index of declaration names to their UUIDs, built from the final program AST.
struct DeclIndex {
    /// Function name → UUID (includes top-level fns, class methods as ClassName$method, app methods as AppName$method)
    fn_index: HashMap<String, Uuid>,
    /// Class name → UUID
    class_index: HashMap<String, Uuid>,
    /// Enum name → UUID
    enum_index: HashMap<String, Uuid>,
    /// (enum_name, variant_name) → variant UUID
    variant_index: HashMap<(String, String), Uuid>,
    /// Error name → UUID
    error_index: HashMap<String, Uuid>,
}

impl DeclIndex {
    fn build(program: &Program) -> Self {
        let mut fn_index = HashMap::new();
        let mut class_index = HashMap::new();
        let mut enum_index = HashMap::new();
        let mut variant_index = HashMap::new();
        let mut error_index = HashMap::new();

        for f in &program.functions {
            fn_index.insert(f.node.name.node.clone(), f.node.id);
        }

        for c in &program.classes {
            class_index.insert(c.node.name.node.clone(), c.node.id);
            for m in &c.node.methods {
                let mangled = mangle_method(&c.node.name.node, &m.node.name.node);
                fn_index.insert(mangled, m.node.id);
            }
        }

        for e in &program.enums {
            enum_index.insert(e.node.name.node.clone(), e.node.id);
            for v in &e.node.variants {
                variant_index.insert(
                    (e.node.name.node.clone(), v.name.node.clone()),
                    v.id,
                );
            }
        }

        for err in &program.errors {
            error_index.insert(err.node.name.node.clone(), err.node.id);
        }

        if let Some(app) = &program.app {
            for m in &app.node.methods {
                let mangled = mangle_method(&app.node.name.node, &m.node.name.node);
                fn_index.insert(mangled, m.node.id);
            }
        }

        for stage in &program.stages {
            for m in &stage.node.methods {
                let mangled = mangle_method(&stage.node.name.node, &m.node.name.node);
                fn_index.insert(mangled, m.node.id);
            }
        }

        // Also index extern functions (they have no UUID in the AST, so skip them)
        // Trait methods have bodies but are indexed through class impls, not directly callable by name

        DeclIndex {
            fn_index,
            class_index,
            enum_index,
            variant_index,
            error_index,
        }
    }
}

/// Resolve cross-references in the program AST.
/// Populates `target_id`, `enum_id`, `variant_id`, and `error_id` fields
/// on Expr and Stmt nodes by looking up declaration names in the index.
///
/// Names not found (builtins like `print`, `expect`, etc.) are left as `None`.
pub fn resolve_cross_refs(program: &mut Program) {
    let index = DeclIndex::build(program);

    // Walk all top-level functions
    for f in &mut program.functions {
        resolve_block(&mut f.node.body.node, &index);
    }

    // Walk class methods
    for c in &mut program.classes {
        for m in &mut c.node.methods {
            resolve_block(&mut m.node.body.node, &index);
        }
        // Walk invariant expressions
        let mut resolver = XrefResolver { index: &index };
        for inv in &mut c.node.invariants {
            resolver.visit_expr_mut(&mut inv.node.expr);
        }
    }

    // Walk app methods
    if let Some(app) = &mut program.app {
        for m in &mut app.node.methods {
            resolve_block(&mut m.node.body.node, &index);
        }
    }

    // Walk stage methods
    for stage in &mut program.stages {
        for m in &mut stage.node.methods {
            resolve_block(&mut m.node.body.node, &index);
        }
    }
}

struct XrefResolver<'a> {
    index: &'a DeclIndex,
}

impl VisitMut for XrefResolver<'_> {
    fn visit_expr_mut(&mut self, expr: &mut Spanned<Expr>) {
        // Handle expressions that need ID resolution
        match &mut expr.node {
            Expr::Call { name, target_id, .. } => {
                *target_id = self.index.fn_index.get(&name.node).copied();
            }
            Expr::StructLit { name, target_id, .. } => {
                *target_id = self.index.class_index.get(&name.node).copied();
            }
            Expr::EnumUnit { enum_name, variant, enum_id, variant_id, .. } => {
                *enum_id = self.index.enum_index.get(&enum_name.node).copied();
                *variant_id = self.index.variant_index.get(
                    &(enum_name.node.clone(), variant.node.clone())
                ).copied();
            }
            Expr::EnumData { enum_name, variant, enum_id, variant_id, .. } => {
                *enum_id = self.index.enum_index.get(&enum_name.node).copied();
                *variant_id = self.index.variant_index.get(
                    &(enum_name.node.clone(), variant.node.clone())
                ).copied();
            }
            Expr::ClosureCreate { fn_name, target_id, .. } => {
                *target_id = self.index.fn_index.get(fn_name).copied();
            }
            Expr::QualifiedAccess { segments } => {
                panic!(
                    "QualifiedAccess should be resolved by module flattening before xref. Segments: {:?}",
                    segments.iter().map(|s| &s.node).collect::<Vec<_>>()
                )
            }
            Expr::StringInterp { parts } => {
                for part in parts {
                    if let StringInterpPart::Expr(e) = part {
                        self.visit_expr_mut(e);
                    }
                }
                return;
            }
            _ => {}
        }
        // Recurse into sub-expressions
        walk_expr_mut(self, expr);
    }

    fn visit_stmt_mut(&mut self, stmt: &mut Spanned<Stmt>) {
        // Handle statements that need ID resolution
        match &mut stmt.node {
            Stmt::Match { expr, arms } => {
                self.visit_expr_mut(expr);
                for arm in arms {
                    arm.enum_id = self.index.enum_index.get(&arm.enum_name.node).copied();
                    arm.variant_id = self.index.variant_index.get(
                        &(arm.enum_name.node.clone(), arm.variant_name.node.clone())
                    ).copied();
                    self.visit_block_mut(&mut arm.body);
                }
                return;
            }
            Stmt::Raise { error_name, error_id, .. } => {
                *error_id = self.index.error_index.get(&error_name.node).copied();
            }
            _ => {}
        }
        // Recurse into sub-statements
        walk_stmt_mut(self, stmt);
    }
}

fn resolve_block(block: &mut Block, index: &DeclIndex) {
    let mut resolver = XrefResolver { index };
    for stmt in &mut block.stmts {
        resolver.visit_stmt_mut(stmt);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::{Span, Spanned};
    use uuid::Uuid;

    fn sp<T>(node: T) -> Spanned<T> {
        Spanned::new(node, Span::dummy())
    }

    fn empty_block() -> Spanned<Block> {
        sp(Block { stmts: vec![] })
    }

    fn make_function(name: &str) -> Spanned<Function> {
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

    #[test]
    fn test_call_resolves_to_function() {
        let mut program = empty_program();
        let target_fn = make_function("greet");
        let target_id = target_fn.node.id;

        // Caller function that calls greet()
        let mut caller = make_function("main");
        caller.node.body = sp(Block {
            stmts: vec![sp(Stmt::Expr(sp(Expr::Call {
                name: sp("greet".to_string()),
                args: vec![],
                type_args: vec![],
                target_id: None,
            })))],
        });

        program.functions.push(target_fn);
        program.functions.push(caller);

        resolve_cross_refs(&mut program);

        // The call in main should now point to greet's UUID
        if let Stmt::Expr(ref e) = program.functions[1].node.body.node.stmts[0].node {
            if let Expr::Call { target_id: ref tid, .. } = e.node {
                assert_eq!(*tid, Some(target_id));
                return;
            }
        }
        panic!("expected Call expr");
    }

    #[test]
    fn test_struct_lit_resolves_to_class() {
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

        let mut caller = make_function("main");
        caller.node.body = sp(Block {
            stmts: vec![sp(Stmt::Expr(sp(Expr::StructLit {
                name: sp("Point".to_string()),
                type_args: vec![],
                fields: vec![],
                target_id: None,
            })))],
        });
        program.functions.push(caller);

        resolve_cross_refs(&mut program);

        if let Stmt::Expr(ref e) = program.functions[0].node.body.node.stmts[0].node {
            if let Expr::StructLit { target_id: ref tid, .. } = e.node {
                assert_eq!(*tid, Some(class_id));
                return;
            }
        }
        panic!("expected StructLit expr");
    }

    #[test]
    fn test_enum_unit_resolves() {
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

        let mut caller = make_function("main");
        caller.node.body = sp(Block {
            stmts: vec![sp(Stmt::Expr(sp(Expr::EnumUnit {
                enum_name: sp("Color".to_string()),
                variant: sp("Red".to_string()),
                type_args: vec![],
                enum_id: None,
                variant_id: None,
            })))],
        });
        program.functions.push(caller);

        resolve_cross_refs(&mut program);

        if let Stmt::Expr(ref e) = program.functions[0].node.body.node.stmts[0].node {
            if let Expr::EnumUnit { enum_id: ref eid, variant_id: ref vid, .. } = e.node {
                assert_eq!(*eid, Some(enum_id));
                assert_eq!(*vid, Some(variant_id));
                return;
            }
        }
        panic!("expected EnumUnit expr");
    }

    #[test]
    fn test_enum_data_resolves() {
        let mut program = empty_program();
        let enum_id = Uuid::new_v4();
        let variant_id = Uuid::new_v4();
        program.enums.push(sp(EnumDecl {
            id: enum_id,
            name: sp("Shape".to_string()),
            type_params: vec![],
            type_param_bounds: std::collections::HashMap::new(),
            variants: vec![EnumVariant {
                id: variant_id,
                name: sp("Circle".to_string()),
                fields: vec![],
            }],
            is_pub: false,
        }));

        let mut caller = make_function("main");
        caller.node.body = sp(Block {
            stmts: vec![sp(Stmt::Expr(sp(Expr::EnumData {
                enum_name: sp("Shape".to_string()),
                variant: sp("Circle".to_string()),
                type_args: vec![],
                fields: vec![],
                enum_id: None,
                variant_id: None,
            })))],
        });
        program.functions.push(caller);

        resolve_cross_refs(&mut program);

        if let Stmt::Expr(ref e) = program.functions[0].node.body.node.stmts[0].node {
            if let Expr::EnumData { enum_id: ref eid, variant_id: ref vid, .. } = e.node {
                assert_eq!(*eid, Some(enum_id));
                assert_eq!(*vid, Some(variant_id));
                return;
            }
        }
        panic!("expected EnumData expr");
    }

    #[test]
    fn test_raise_resolves_to_error() {
        let mut program = empty_program();
        let err_id = Uuid::new_v4();
        program.errors.push(sp(ErrorDecl {
            id: err_id,
            name: sp("NotFound".to_string()),
            fields: vec![],
            is_pub: false,
        }));

        let mut caller = make_function("main");
        caller.node.body = sp(Block {
            stmts: vec![sp(Stmt::Raise {
                error_name: sp("NotFound".to_string()),
                fields: vec![],
                error_id: None,
            })],
        });
        program.functions.push(caller);

        resolve_cross_refs(&mut program);

        if let Stmt::Raise { error_id, .. } = &program.functions[0].node.body.node.stmts[0].node {
            assert_eq!(*error_id, Some(err_id));
            return;
        }
        panic!("expected Raise stmt");
    }

    #[test]
    fn test_match_arm_resolves() {
        let mut program = empty_program();
        let enum_id = Uuid::new_v4();
        let variant_id = Uuid::new_v4();
        program.enums.push(sp(EnumDecl {
            id: enum_id,
            name: sp("Option".to_string()),
            type_params: vec![],
            type_param_bounds: std::collections::HashMap::new(),
            variants: vec![EnumVariant {
                id: variant_id,
                name: sp("Some".to_string()),
                fields: vec![],
            }],
            is_pub: false,
        }));

        let mut caller = make_function("main");
        caller.node.body = sp(Block {
            stmts: vec![sp(Stmt::Match {
                expr: sp(Expr::IntLit(0)),
                arms: vec![MatchArm {
                    enum_name: sp("Option".to_string()),
                    variant_name: sp("Some".to_string()),
                    type_args: vec![],
                    bindings: vec![],
                    body: empty_block(),
                    enum_id: None,
                    variant_id: None,
                }],
            })],
        });
        program.functions.push(caller);

        resolve_cross_refs(&mut program);

        if let Stmt::Match { arms, .. } = &program.functions[0].node.body.node.stmts[0].node {
            assert_eq!(arms[0].enum_id, Some(enum_id));
            assert_eq!(arms[0].variant_id, Some(variant_id));
            return;
        }
        panic!("expected Match stmt");
    }

    #[test]
    fn test_builtin_leaves_none() {
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

        resolve_cross_refs(&mut program);

        if let Stmt::Expr(ref e) = program.functions[0].node.body.node.stmts[0].node {
            if let Expr::Call { target_id, .. } = &e.node {
                assert_eq!(*target_id, None);
                return;
            }
        }
        panic!("expected Call expr");
    }

    #[test]
    fn test_closure_create_resolves() {
        let mut program = empty_program();
        let lifted_fn = make_function("__closure_0");
        let lifted_id = lifted_fn.node.id;

        let mut caller = make_function("main");
        caller.node.body = sp(Block {
            stmts: vec![sp(Stmt::Expr(sp(Expr::ClosureCreate {
                fn_name: "__closure_0".to_string(),
                captures: vec![],
                target_id: None,
            })))],
        });

        program.functions.push(lifted_fn);
        program.functions.push(caller);

        resolve_cross_refs(&mut program);

        if let Stmt::Expr(ref e) = program.functions[1].node.body.node.stmts[0].node {
            if let Expr::ClosureCreate { target_id, .. } = &e.node {
                assert_eq!(*target_id, Some(lifted_id));
                return;
            }
        }
        panic!("expected ClosureCreate expr");
    }

    #[test]
    fn test_module_prefixed_names() {
        let mut program = empty_program();
        // After module flattening, functions get prefixed names like "math.add"
        let math_add = make_function("math.add");
        let math_add_id = math_add.node.id;
        program.functions.push(math_add);

        let mut caller = make_function("main");
        caller.node.body = sp(Block {
            stmts: vec![sp(Stmt::Expr(sp(Expr::Call {
                name: sp("math.add".to_string()),
                args: vec![],
                type_args: vec![],
                target_id: None,
            })))],
        });
        program.functions.push(caller);

        resolve_cross_refs(&mut program);

        if let Stmt::Expr(ref e) = program.functions[1].node.body.node.stmts[0].node {
            if let Expr::Call { target_id, .. } = &e.node {
                assert_eq!(*target_id, Some(math_add_id));
                return;
            }
        }
        panic!("expected Call expr");
    }

    #[test]
    fn test_monomorphized_names() {
        let mut program = empty_program();
        // After monomorphization, generic functions get mangled names like "identity$$int"
        let identity_int = make_function("identity$$int");
        let identity_int_id = identity_int.node.id;
        program.functions.push(identity_int);

        let mut caller = make_function("main");
        caller.node.body = sp(Block {
            stmts: vec![sp(Stmt::Expr(sp(Expr::Call {
                name: sp("identity$$int".to_string()),
                args: vec![],
                type_args: vec![],
                target_id: None,
            })))],
        });
        program.functions.push(caller);

        resolve_cross_refs(&mut program);

        if let Stmt::Expr(ref e) = program.functions[1].node.body.node.stmts[0].node {
            if let Expr::Call { target_id, .. } = &e.node {
                assert_eq!(*target_id, Some(identity_int_id));
                return;
            }
        }
        panic!("expected Call expr");
    }

    #[test]
    fn test_class_method_indexed() {
        let mut program = empty_program();
        let method_id = Uuid::new_v4();
        program.classes.push(sp(ClassDecl {
            id: Uuid::new_v4(),
            name: sp("Greeter".to_string()),
            type_params: vec![],
            type_param_bounds: std::collections::HashMap::new(),
            fields: vec![],
            methods: vec![sp(Function {
                id: method_id,
                name: sp("hello".to_string()),
                type_params: vec![],
                type_param_bounds: std::collections::HashMap::new(),
                params: vec![],
                return_type: None,
                contracts: vec![],
                body: empty_block(),
                is_pub: false,
                is_override: false,
                is_generator: false,
            })],
            invariants: vec![],
            impl_traits: vec![],
            uses: vec![],
            is_pub: false,
            lifecycle: Lifecycle::Singleton,
        }));

        // After codegen method mangling, calls use "Greeter$hello"
        let mut caller = make_function("main");
        caller.node.body = sp(Block {
            stmts: vec![sp(Stmt::Expr(sp(Expr::Call {
                name: sp("Greeter$hello".to_string()),
                args: vec![],
                type_args: vec![],
                target_id: None,
            })))],
        });
        program.functions.push(caller);

        resolve_cross_refs(&mut program);

        if let Stmt::Expr(ref e) = program.functions[0].node.body.node.stmts[0].node {
            if let Expr::Call { target_id, .. } = &e.node {
                assert_eq!(*target_id, Some(method_id));
                return;
            }
        }
        panic!("expected Call expr");
    }
}
