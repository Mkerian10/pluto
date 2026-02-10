//! Derived analysis data extracted from TypeEnv after type checking.
//!
//! This module bridges the gap between the compiler's transient type analysis
//! (stored in `TypeEnv`) and the serialized PLTO binary format. It captures
//! error sets, resolved function signatures, and fallibility — keyed by AST
//! node UUID — so that consumers (like the SDK) can query type information
//! without re-running the compiler.

use std::collections::BTreeMap;
use uuid::Uuid;

use crate::parser::ast::Program;
use crate::typeck::env::TypeEnv;
use crate::typeck::types::PlutoType;

/// Map an AST function node to the key used in TypeEnv.
/// `class_name`: `Some("Counter")` for methods, `None` for top-level fns.
pub fn typeenv_key(fn_name: &str, class_name: Option<&str>) -> String {
    match class_name {
        Some(cls) => format!("{}_{}", cls, fn_name),
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

        DerivedInfo {
            fn_error_sets,
            fn_signatures,
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
        assert_eq!(typeenv_key("increment", Some("Counter")), "Counter_increment");
        assert_eq!(typeenv_key("main", Some("MyApp")), "MyApp_main");
    }

    #[test]
    fn typeenv_key_module_prefixed() {
        // Module-prefixed functions keep their name as-is (prefix is part of the name)
        assert_eq!(typeenv_key("math.add", None), "math.add");
    }
}
