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
                        lifecycle: ci.lifecycle.clone(),
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
                    if cls_info.impl_traits.iter().any(|t| t == trait_name) {
                        if let Some(&uuid) = class_name_to_uuid.get(cls_name.as_str()) {
                            implementors.push(uuid);
                        }
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

        DerivedInfo {
            fn_error_sets,
            fn_signatures,
            class_infos,
            trait_infos,
            enum_infos,
            error_infos,
            di_order,
            trait_implementors,
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
