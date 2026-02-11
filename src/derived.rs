//! Derived analysis data extracted from TypeEnv after type checking.
//!
//! This module bridges the gap between the compiler's transient type analysis
//! (stored in `TypeEnv`) and the serialized PLTO binary format. It captures
//! error sets, resolved function signatures, fallibility, class/trait/enum/error
//! type information, and DI wiring — keyed by AST node UUID — so that consumers
//! (like the SDK) can query type information without re-running the compiler.

use std::collections::BTreeMap;
use uuid::Uuid;

use crate::parser::ast::{Lifecycle, Program};
use crate::typeck::env::{mangle_method, TypeEnv};
use crate::typeck::types::PlutoType;

/// Map an AST function node to the key used in TypeEnv.
/// `class_name`: `Some("Counter")` for methods, `None` for top-level fns.
pub fn typeenv_key(fn_name: &str, class_name: Option<&str>) -> String {
    match class_name {
        Some(cls) => mangle_method(cls, fn_name),
        None => fn_name.to_string(),
    }
}

/// Derived analysis data for a program, indexed by AST node UUID.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DerivedInfo {
    /// Error sets: function UUID -> list of errors it can raise (sorted by name).
    pub fn_error_sets: BTreeMap<Uuid, Vec<ErrorRef>>,
    /// Resolved function signatures: function UUID -> resolved param/return types.
    pub fn_signatures: BTreeMap<Uuid, ResolvedSignature>,
    /// Resolved class type info: class UUID -> fields, methods, traits, lifecycle.
    #[serde(default)]
    pub class_infos: BTreeMap<Uuid, ResolvedClassInfo>,
    /// Resolved trait type info: trait UUID -> methods with signatures.
    #[serde(default)]
    pub trait_infos: BTreeMap<Uuid, ResolvedTraitInfo>,
    /// Resolved enum type info: enum UUID -> variants with fields.
    #[serde(default)]
    pub enum_infos: BTreeMap<Uuid, ResolvedEnumInfo>,
    /// Resolved error type info: error UUID -> fields with types.
    #[serde(default)]
    pub error_infos: BTreeMap<Uuid, ResolvedErrorInfo>,
    /// DI instantiation order (class UUIDs in topological order).
    #[serde(default)]
    pub di_order: Vec<Uuid>,
    /// Trait UUID -> list of class UUIDs that implement it.
    #[serde(default)]
    pub trait_implementors: BTreeMap<Uuid, Vec<Uuid>>,
    /// Test dependency hashes: test display_name -> hash of all transitive dependencies.
    #[serde(default)]
    pub test_dep_hashes: BTreeMap<String, String>,
}

/// A reference to an error declaration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorRef {
    /// UUID of the error declaration (`None` for extern/unresolved errors).
    pub id: Option<Uuid>,
    /// Error type name (human-readable, for display).
    pub name: String,
}

/// Resolved function signature with concrete types.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedSignature {
    pub param_types: Vec<PlutoType>,
    pub return_type: PlutoType,
    pub is_fallible: bool,
}

/// Resolved class info extracted from TypeEnv.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedClassInfo {
    pub fields: Vec<ResolvedFieldInfo>,
    pub methods: Vec<(String, ResolvedSignature)>,
    pub impl_traits: Vec<String>,
    pub lifecycle: Lifecycle,
    pub is_pub: bool,
}

/// A single resolved field with its concrete type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedFieldInfo {
    pub name: String,
    pub ty: PlutoType,
    pub is_injected: bool,
}

/// Resolved trait info extracted from TypeEnv.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedTraitInfo {
    pub methods: Vec<(String, ResolvedSignature)>,
    pub default_methods: Vec<String>,
    pub implementors: Vec<Uuid>,
}

/// Resolved enum info extracted from TypeEnv.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedEnumInfo {
    pub variants: Vec<ResolvedVariantInfo>,
}

/// A single resolved enum variant with its fields.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedVariantInfo {
    pub name: String,
    pub fields: Vec<ResolvedFieldInfo>,
}

/// Resolved error declaration info extracted from TypeEnv.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedErrorInfo {
    pub fields: Vec<ResolvedFieldInfo>,
}

/// Collects all function names, class names, and enum names that a test transitively depends on.
/// Returns a sorted list for stable hashing.
fn collect_test_dependencies(
    test_fn_name: &str,
    program: &Program,
    visited: &mut std::collections::HashSet<String>,
    deps: &mut Vec<String>,
) {
    if visited.contains(test_fn_name) {
        return;
    }
    visited.insert(test_fn_name.to_string());
    deps.push(test_fn_name.to_string());

    // Find the test function
    let test_fn = program.functions.iter().find(|f| f.node.name.node == test_fn_name);
    if let Some(func) = test_fn {
        // Collect dependencies from the function body
        collect_deps_from_block(&func.node.body.node, program, visited, deps);
    }
}

/// Recursively collect dependencies from a block of statements
fn collect_deps_from_block(
    block: &crate::parser::ast::Block,
    program: &Program,
    visited: &mut std::collections::HashSet<String>,
    deps: &mut Vec<String>,
) {
    use crate::parser::ast::Stmt;

    for stmt in &block.stmts {
        match &stmt.node {
            Stmt::Let { value, .. } => {
                collect_deps_from_expr(&value.node, program, visited, deps);
            }
            Stmt::Return(Some(expr)) => {
                collect_deps_from_expr(&expr.node, program, visited, deps);
            }
            Stmt::Assign { value, .. } => {
                collect_deps_from_expr(&value.node, program, visited, deps);
            }
            Stmt::FieldAssign { object, value, .. } => {
                collect_deps_from_expr(&object.node, program, visited, deps);
                collect_deps_from_expr(&value.node, program, visited, deps);
            }
            Stmt::If { condition, then_block, else_block } => {
                collect_deps_from_expr(&condition.node, program, visited, deps);
                collect_deps_from_block(&then_block.node, program, visited, deps);
                if let Some(else_blk) = else_block {
                    collect_deps_from_block(&else_blk.node, program, visited, deps);
                }
            }
            Stmt::While { condition, body } => {
                collect_deps_from_expr(&condition.node, program, visited, deps);
                collect_deps_from_block(&body.node, program, visited, deps);
            }
            Stmt::For { iterable, body, .. } => {
                collect_deps_from_expr(&iterable.node, program, visited, deps);
                collect_deps_from_block(&body.node, program, visited, deps);
            }
            Stmt::IndexAssign { object, index, value } => {
                collect_deps_from_expr(&object.node, program, visited, deps);
                collect_deps_from_expr(&index.node, program, visited, deps);
                collect_deps_from_expr(&value.node, program, visited, deps);
            }
            Stmt::Match { expr, arms } => {
                collect_deps_from_expr(&expr.node, program, visited, deps);
                for arm in arms {
                    collect_deps_from_block(&arm.body.node, program, visited, deps);
                }
            }
            Stmt::Raise { fields, .. } => {
                for (_name, expr) in fields {
                    collect_deps_from_expr(&expr.node, program, visited, deps);
                }
            }
            Stmt::Select { arms, default } => {
                for arm in arms {
                    use crate::parser::ast::SelectOp;
                    match &arm.op {
                        SelectOp::Recv { channel, .. } => {
                            collect_deps_from_expr(&channel.node, program, visited, deps);
                        }
                        SelectOp::Send { channel, value } => {
                            collect_deps_from_expr(&channel.node, program, visited, deps);
                            collect_deps_from_expr(&value.node, program, visited, deps);
                        }
                    }
                    collect_deps_from_block(&arm.body.node, program, visited, deps);
                }
                if let Some(default_block) = default {
                    collect_deps_from_block(&default_block.node, program, visited, deps);
                }
            }
            Stmt::Scope { seeds, body, .. } => {
                for seed in seeds {
                    collect_deps_from_expr(&seed.node, program, visited, deps);
                }
                collect_deps_from_block(&body.node, program, visited, deps);
            }
            Stmt::Yield { value } => {
                collect_deps_from_expr(&value.node, program, visited, deps);
            }
            Stmt::Expr(expr) => {
                collect_deps_from_expr(&expr.node, program, visited, deps);
            }
            _ => {}
        }
    }
}

/// Recursively collect dependencies from an expression
fn collect_deps_from_expr(
    expr: &crate::parser::ast::Expr,
    program: &Program,
    visited: &mut std::collections::HashSet<String>,
    deps: &mut Vec<String>,
) {
    use crate::parser::ast::Expr;

    match expr {
        Expr::Call { name, args, .. } => {
            let fn_name = &name.node;
            // Recursively collect dependencies from called function
            collect_test_dependencies(fn_name, program, visited, deps);
            // Also collect deps from args
            for arg in args {
                collect_deps_from_expr(&arg.node, program, visited, deps);
            }
        }
        Expr::StructLit { name, fields, .. } => {
            // Track class usage
            let class_name = &name.node;
            if !visited.contains(class_name) {
                visited.insert(class_name.clone());
                deps.push(class_name.clone());
            }
            for (_field_name, field_expr) in fields {
                collect_deps_from_expr(&field_expr.node, program, visited, deps);
            }
        }
        Expr::EnumUnit { enum_name, .. } | Expr::EnumData { enum_name, .. } => {
            // Track enum usage
            let enum_name_str = &enum_name.node;
            if !visited.contains(enum_name_str) {
                visited.insert(enum_name_str.clone());
                deps.push(enum_name_str.clone());
            }
            if let Expr::EnumData { fields, .. } = expr {
                for (_field_name, field_expr) in fields {
                    collect_deps_from_expr(&field_expr.node, program, visited, deps);
                }
            }
        }
        Expr::MethodCall { object, args, .. } => {
            collect_deps_from_expr(&object.node, program, visited, deps);
            for arg in args {
                collect_deps_from_expr(&arg.node, program, visited, deps);
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_deps_from_expr(&lhs.node, program, visited, deps);
            collect_deps_from_expr(&rhs.node, program, visited, deps);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_deps_from_expr(&operand.node, program, visited, deps);
        }
        Expr::FieldAccess { object, .. } => {
            collect_deps_from_expr(&object.node, program, visited, deps);
        }
        Expr::ArrayLit { elements } => {
            for elem in elements {
                collect_deps_from_expr(&elem.node, program, visited, deps);
            }
        }
        Expr::Index { object, index } => {
            collect_deps_from_expr(&object.node, program, visited, deps);
            collect_deps_from_expr(&index.node, program, visited, deps);
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let crate::parser::ast::StringInterpPart::Expr(e) = part {
                    collect_deps_from_expr(&e.node, program, visited, deps);
                }
            }
        }
        Expr::Closure { body, .. } => {
            collect_deps_from_block(&body.node, program, visited, deps);
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_deps_from_expr(&k.node, program, visited, deps);
                collect_deps_from_expr(&v.node, program, visited, deps);
            }
        }
        Expr::SetLit { elements, .. } => {
            for elem in elements {
                collect_deps_from_expr(&elem.node, program, visited, deps);
            }
        }
        Expr::ClosureCreate { fn_name, .. } => {
            // Track closure dependencies
            collect_test_dependencies(fn_name, program, visited, deps);
        }
        Expr::Propagate { expr } | Expr::NullPropagate { expr } => {
            collect_deps_from_expr(&expr.node, program, visited, deps);
        }
        Expr::Catch { expr, handler } => {
            collect_deps_from_expr(&expr.node, program, visited, deps);
            match handler {
                crate::parser::ast::CatchHandler::Shorthand(e) => {
                    collect_deps_from_expr(&e.node, program, visited, deps);
                }
                crate::parser::ast::CatchHandler::Wildcard { body, .. } => {
                    collect_deps_from_block(&body.node, program, visited, deps);
                }
            }
        }
        Expr::Cast { expr, .. } => {
            collect_deps_from_expr(&expr.node, program, visited, deps);
        }
        Expr::Range { start, end, .. } => {
            collect_deps_from_expr(&start.node, program, visited, deps);
            collect_deps_from_expr(&end.node, program, visited, deps);
        }
        Expr::Spawn { call } => {
            collect_deps_from_expr(&call.node, program, visited, deps);
        }
        // Literals don't add dependencies
        _ => {}
    }
}

/// Compute stable dependency hashes for all tests
fn compute_test_dependency_hashes(program: &Program) -> BTreeMap<String, String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut test_dep_hashes = BTreeMap::new();

    for test_info in &program.test_info {
        let mut visited = std::collections::HashSet::new();
        let mut deps = Vec::new();

        // Collect all transitive dependencies
        collect_test_dependencies(&test_info.fn_name, program, &mut visited, &mut deps);

        // Sort for stable hashing
        deps.sort();

        // Hash the sorted dependency list along with function bodies
        let mut hasher = DefaultHasher::new();
        for dep_name in &deps {
            // Hash the name
            dep_name.hash(&mut hasher);

            // Hash the function body if it's a function
            if let Some(func) = program.functions.iter().find(|f| f.node.name.node == *dep_name) {
                // Hash the function's body (simplified - using debug representation)
                // In production, you might want a more sophisticated AST hash
                format!("{:?}", func.node.body).hash(&mut hasher);
            }
            // Hash class definitions
            else if let Some(class) = program.classes.iter().find(|c| c.node.name.node == *dep_name) {
                format!("{:?}", class.node.fields).hash(&mut hasher);
                format!("{:?}", class.node.methods).hash(&mut hasher);
            }
            // Hash enum definitions
            else if let Some(enum_decl) = program.enums.iter().find(|e| e.node.name.node == *dep_name) {
                format!("{:?}", enum_decl.node.variants).hash(&mut hasher);
            }
        }

        let hash_value = hasher.finish();
        test_dep_hashes.insert(
            test_info.display_name.clone(),
            format!("{:x}", hash_value),
        );
    }

    test_dep_hashes
}

impl DerivedInfo {
    /// Build derived info by walking the program AST and extracting type data from TypeEnv.
    pub fn build(env: &TypeEnv, program: &Program) -> Self {
        // Build a lookup from error name -> UUID using the program's error declarations
        let error_uuid_map: BTreeMap<&str, Uuid> = program
            .errors
            .iter()
            .map(|e| (e.node.name.node.as_str(), e.node.id))
            .collect();

        let mut fn_error_sets = BTreeMap::new();
        let mut fn_signatures = BTreeMap::new();

        // Process top-level functions
        for f in &program.functions {
            let name = &f.node.name.node;

            // Skip lifted closures
            if name.starts_with("__closure_") || env.closure_fns.contains_key(name) {
                continue;
            }

            let key = typeenv_key(name, None);
            Self::collect_fn_data(
                f.node.id,
                &key,
                env,
                &error_uuid_map,
                &mut fn_error_sets,
                &mut fn_signatures,
            );
        }

        // Process class methods
        for class in &program.classes {
            let class_name = &class.node.name.node;
            for method in &class.node.methods {
                let key = typeenv_key(&method.node.name.node, Some(class_name));
                Self::collect_fn_data(
                    method.node.id,
                    &key,
                    env,
                    &error_uuid_map,
                    &mut fn_error_sets,
                    &mut fn_signatures,
                );
            }
        }

        // Process app methods
        if let Some(app) = &program.app {
            let app_name = &app.node.name.node;
            for method in &app.node.methods {
                let key = typeenv_key(&method.node.name.node, Some(app_name));
                Self::collect_fn_data(
                    method.node.id,
                    &key,
                    env,
                    &error_uuid_map,
                    &mut fn_error_sets,
                    &mut fn_signatures,
                );
            }
        }

        // Process stage methods
        for stage in &program.stages {
            let stage_name = &stage.node.name.node;
            for method in &stage.node.methods {
                let key = typeenv_key(&method.node.name.node, Some(stage_name));
                Self::collect_fn_data(
                    method.node.id,
                    &key,
                    env,
                    &error_uuid_map,
                    &mut fn_error_sets,
                    &mut fn_signatures,
                );
            }
        }

        // Build name->UUID maps for classes, traits, enums
        let class_name_to_uuid: BTreeMap<&str, Uuid> = program
            .classes
            .iter()
            .map(|c| (c.node.name.node.as_str(), c.node.id))
            .collect();

        // Build class_infos
        let mut class_infos = BTreeMap::new();
        for class in &program.classes {
            let class_name = &class.node.name.node;
            if let Some(ci) = env.classes.get(class_name.as_str()) {
                let fields: Vec<ResolvedFieldInfo> = ci
                    .fields
                    .iter()
                    .map(|(name, ty, is_injected)| ResolvedFieldInfo {
                        name: name.clone(),
                        ty: ty.clone(),
                        is_injected: *is_injected,
                    })
                    .collect();

                let methods: Vec<(String, ResolvedSignature)> = ci
                    .methods
                    .iter()
                    .filter_map(|method_name| {
                        let key = typeenv_key(method_name, Some(class_name));
                        env.functions.get(&key).map(|sig| {
                            let is_fallible = env
                                .fn_errors
                                .get(&key)
                                .is_some_and(|errs| !errs.is_empty());
                            (
                                method_name.clone(),
                                ResolvedSignature {
                                    param_types: sig.params.clone(),
                                    return_type: sig.return_type.clone(),
                                    is_fallible,
                                },
                            )
                        })
                    })
                    .collect();

                class_infos.insert(
                    class.node.id,
                    ResolvedClassInfo {
                        fields,
                        methods,
                        impl_traits: ci.impl_traits.clone(),
                        lifecycle: ci.lifecycle,
                        is_pub: class.node.is_pub,
                    },
                );
            }
        }

        // Build trait_infos + trait_implementors
        let mut trait_infos = BTreeMap::new();
        let mut trait_implementors: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();

        for tr in &program.traits {
            let trait_name = &tr.node.name.node;
            if let Some(ti) = env.traits.get(trait_name.as_str()) {
                let methods: Vec<(String, ResolvedSignature)> = ti
                    .methods
                    .iter()
                    .map(|(name, sig)| {
                        let is_fallible =
                            env.is_trait_method_potentially_fallible(trait_name, name);
                        (
                            name.clone(),
                            ResolvedSignature {
                                param_types: sig.params.clone(),
                                return_type: sig.return_type.clone(),
                                is_fallible,
                            },
                        )
                    })
                    .collect();

                // Find implementors by scanning classes
                let mut implementors = Vec::new();
                for (cls_name, cls_info) in &env.classes {
                    if cls_info.impl_traits.iter().any(|t| t == trait_name)
                        && let Some(&uuid) = class_name_to_uuid.get(cls_name.as_str())
                    {
                        implementors.push(uuid);
                    }
                }
                implementors.sort();

                let trait_uuid = tr.node.id;
                trait_implementors.insert(trait_uuid, implementors.clone());

                trait_infos.insert(
                    trait_uuid,
                    ResolvedTraitInfo {
                        methods,
                        default_methods: ti.default_methods.clone(),
                        implementors,
                    },
                );
            }
        }

        // Build enum_infos
        let mut enum_infos = BTreeMap::new();
        for en in &program.enums {
            let enum_name = &en.node.name.node;
            if let Some(ei) = env.enums.get(enum_name.as_str()) {
                let variants: Vec<ResolvedVariantInfo> = ei
                    .variants
                    .iter()
                    .map(|(name, fields)| ResolvedVariantInfo {
                        name: name.clone(),
                        fields: fields
                            .iter()
                            .map(|(fname, fty)| ResolvedFieldInfo {
                                name: fname.clone(),
                                ty: fty.clone(),
                                is_injected: false,
                            })
                            .collect(),
                    })
                    .collect();

                enum_infos.insert(en.node.id, ResolvedEnumInfo { variants });
            }
        }

        // Build error_infos
        let mut error_infos = BTreeMap::new();
        for err in &program.errors {
            let err_name = &err.node.name.node;
            if let Some(erri) = env.errors.get(err_name.as_str()) {
                let fields: Vec<ResolvedFieldInfo> = erri
                    .fields
                    .iter()
                    .map(|(name, ty)| ResolvedFieldInfo {
                        name: name.clone(),
                        ty: ty.clone(),
                        is_injected: false,
                    })
                    .collect();

                error_infos.insert(err.node.id, ResolvedErrorInfo { fields });
            }
        }

        // Build di_order: map class names to UUIDs
        let di_order: Vec<Uuid> = env
            .di_order
            .iter()
            .filter_map(|name| class_name_to_uuid.get(name.as_str()).copied())
            .collect();

        // Compute test dependency hashes
        let test_dep_hashes = compute_test_dependency_hashes(program);

        DerivedInfo {
            fn_error_sets,
            fn_signatures,
            class_infos,
            trait_infos,
            enum_infos,
            error_infos,
            di_order,
            trait_implementors,
            test_dep_hashes,
        }
    }

    fn collect_fn_data(
        id: Uuid,
        key: &str,
        env: &TypeEnv,
        error_uuid_map: &BTreeMap<&str, Uuid>,
        fn_error_sets: &mut BTreeMap<Uuid, Vec<ErrorRef>>,
        fn_signatures: &mut BTreeMap<Uuid, ResolvedSignature>,
    ) {
        // Error set
        let error_set = if let Some(errors) = env.fn_errors.get(key) {
            let mut refs: Vec<ErrorRef> = errors
                .iter()
                .map(|err_name| ErrorRef {
                    id: error_uuid_map.get(err_name.as_str()).copied(),
                    name: err_name.clone(),
                })
                .collect();
            refs.sort_by(|a, b| a.name.cmp(&b.name));
            refs
        } else {
            vec![]
        };

        let is_fallible = !error_set.is_empty();
        fn_error_sets.insert(id, error_set);

        // Resolved signature
        if let Some(sig) = env.functions.get(key) {
            fn_signatures.insert(
                id,
                ResolvedSignature {
                    param_types: sig.params.clone(),
                    return_type: sig.return_type.clone(),
                    is_fallible,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typeenv_key_top_level() {
        assert_eq!(typeenv_key("main", None), "main");
        assert_eq!(typeenv_key("add", None), "add");
    }

    #[test]
    fn typeenv_key_method() {
        assert_eq!(typeenv_key("increment", Some("Counter")), "Counter$increment");
        assert_eq!(typeenv_key("main", Some("MyApp")), "MyApp$main");
    }

    #[test]
    fn typeenv_key_module_prefixed() {
        // Module-prefixed functions keep their name as-is (prefix is part of the name)
        assert_eq!(typeenv_key("math.add", None), "math.add");
    }

    #[test]
    fn default_has_empty_collections() {
        let d = DerivedInfo::default();
        assert!(d.fn_error_sets.is_empty());
        assert!(d.fn_signatures.is_empty());
        assert!(d.class_infos.is_empty());
        assert!(d.trait_infos.is_empty());
        assert!(d.enum_infos.is_empty());
        assert!(d.error_infos.is_empty());
        assert!(d.di_order.is_empty());
        assert!(d.trait_implementors.is_empty());
    }
}
