//! Stable declaration identity that survives re-parsing.
//!
//! UUIDs are generated fresh on every parse (`Uuid::new_v4()`), so they cannot serve
//! as stable cache keys across compilations. `DeclKey` provides a deterministic identity
//! based on (file, kind, name) that remains stable as long as the declaration's name
//! and kind don't change.

use std::collections::HashMap;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::modules::SourceMap;
use crate::parser::ast::Program;

/// A deterministic declaration key: (file, kind, name).
///
/// Two parses of the same source file will produce identical `DeclKey`s for each
/// declaration, even though their UUIDs differ.
#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DeclKey {
    /// Source file path (canonical).
    pub file: String,
    /// Declaration kind: "function", "class", "enum", "trait", "error", "app", "stage",
    /// "field", "param", "method", "enum_variant", "trait_method".
    pub kind: String,
    /// Declaration name. Methods use "ClassName.method_name".
    /// Monomorphized copies use "name<TypeA, TypeB>".
    pub name: String,
}

impl DeclKey {
    pub fn new(file: impl Into<String>, kind: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            kind: kind.into(),
            name: name.into(),
        }
    }

    /// Compute a stable SHA-256 hash of this key.
    pub fn stable_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.file.as_bytes());
        hasher.update(b"\0");
        hasher.update(self.kind.as_bytes());
        hasher.update(b"\0");
        hasher.update(self.name.as_bytes());
        hasher.finalize().into()
    }
}

/// Bidirectional mapping between UUIDs (ephemeral, per-parse) and DeclKeys (stable).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DeclKeyMap {
    pub uuid_to_key: HashMap<Uuid, DeclKey>,
    pub key_to_uuid: HashMap<DeclKey, Uuid>,
}

impl DeclKeyMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a UUID ↔ DeclKey mapping.
    pub fn insert(&mut self, uuid: Uuid, key: DeclKey) {
        self.uuid_to_key.insert(uuid, key.clone());
        self.key_to_uuid.insert(key, uuid);
    }

    /// Look up a DeclKey by UUID.
    pub fn get_key(&self, uuid: &Uuid) -> Option<&DeclKey> {
        self.uuid_to_key.get(uuid)
    }

    /// Look up a UUID by DeclKey.
    pub fn get_uuid(&self, key: &DeclKey) -> Option<&Uuid> {
        self.key_to_uuid.get(key)
    }

    /// Build a DeclKeyMap from a fully-flattened Program and a SourceMap.
    ///
    /// Uses each declaration's `span.file_id` to look up its source file path
    /// from the SourceMap. For declarations without a valid file_id (e.g., prelude
    /// injections), falls back to "<unknown>".
    pub fn build(program: &Program, source_map: &SourceMap) -> Self {
        let mut map = Self::new();
        let unknown = "<unknown>".to_string();

        // Helper to get file path string from a span's file_id
        let file_of = |file_id: u32| -> String {
            source_map
                .files
                .get(file_id as usize)
                .map(|(path, _)| path.to_string_lossy().into_owned())
                .unwrap_or_else(|| unknown.clone())
        };

        // Top-level functions
        for f in &program.functions {
            let file = file_of(f.span.file_id);
            map.insert(
                f.node.id,
                DeclKey::new(&file, "function", &f.node.name.node),
            );
            // Params
            for p in &f.node.params {
                map.insert(
                    p.id,
                    DeclKey::new(&file, "param", format!("{}.{}", f.node.name.node, p.name.node)),
                );
            }
        }

        // Classes
        for c in &program.classes {
            let file = file_of(c.span.file_id);
            map.insert(
                c.node.id,
                DeclKey::new(&file, "class", &c.node.name.node),
            );
            // Fields
            for field in &c.node.fields {
                map.insert(
                    field.id,
                    DeclKey::new(&file, "field", format!("{}.{}", c.node.name.node, field.name.node)),
                );
            }
            // Methods
            for method in &c.node.methods {
                let method_name = format!("{}.{}", c.node.name.node, method.node.name.node);
                map.insert(
                    method.node.id,
                    DeclKey::new(&file, "method", &method_name),
                );
                // Method params
                for p in &method.node.params {
                    map.insert(
                        p.id,
                        DeclKey::new(&file, "param", format!("{}.{}", method_name, p.name.node)),
                    );
                }
            }
        }

        // Traits
        for t in &program.traits {
            let file = file_of(t.span.file_id);
            map.insert(
                t.node.id,
                DeclKey::new(&file, "trait", &t.node.name.node),
            );
            // Trait methods
            for m in &t.node.methods {
                map.insert(
                    m.id,
                    DeclKey::new(&file, "trait_method", format!("{}.{}", t.node.name.node, m.name.node)),
                );
            }
        }

        // Enums
        for e in &program.enums {
            let file = file_of(e.span.file_id);
            map.insert(
                e.node.id,
                DeclKey::new(&file, "enum", &e.node.name.node),
            );
            // Variants
            for v in &e.node.variants {
                map.insert(
                    v.id,
                    DeclKey::new(&file, "enum_variant", format!("{}.{}", e.node.name.node, v.name.node)),
                );
                // Variant fields
                for field in &v.fields {
                    map.insert(
                        field.id,
                        DeclKey::new(
                            &file,
                            "field",
                            format!("{}.{}.{}", e.node.name.node, v.name.node, field.name.node),
                        ),
                    );
                }
            }
        }

        // Errors
        for e in &program.errors {
            let file = file_of(e.span.file_id);
            map.insert(
                e.node.id,
                DeclKey::new(&file, "error", &e.node.name.node),
            );
            // Error fields
            for field in &e.node.fields {
                map.insert(
                    field.id,
                    DeclKey::new(&file, "field", format!("{}.{}", e.node.name.node, field.name.node)),
                );
            }
        }

        // App
        if let Some(app) = &program.app {
            let file = file_of(app.span.file_id);
            map.insert(
                app.node.id,
                DeclKey::new(&file, "app", &app.node.name.node),
            );
            // App fields (inject fields)
            for field in &app.node.inject_fields {
                map.insert(
                    field.id,
                    DeclKey::new(&file, "field", format!("{}.{}", app.node.name.node, field.name.node)),
                );
            }
            // App methods
            for method in &app.node.methods {
                let method_name = format!("{}.{}", app.node.name.node, method.node.name.node);
                map.insert(
                    method.node.id,
                    DeclKey::new(&file, "method", &method_name),
                );
                for p in &method.node.params {
                    map.insert(
                        p.id,
                        DeclKey::new(&file, "param", format!("{}.{}", method_name, p.name.node)),
                    );
                }
            }
        }

        // Stages
        for stage in &program.stages {
            let file = file_of(stage.span.file_id);
            map.insert(
                stage.node.id,
                DeclKey::new(&file, "stage", &stage.node.name.node),
            );
            // Stage fields
            for field in &stage.node.inject_fields {
                map.insert(
                    field.id,
                    DeclKey::new(&file, "field", format!("{}.{}", stage.node.name.node, field.name.node)),
                );
            }
            // Required methods
            for rm in &stage.node.required_methods {
                map.insert(
                    rm.node.id,
                    DeclKey::new(&file, "method", format!("{}.{}", stage.node.name.node, rm.node.name.node)),
                );
            }
            // Stage methods
            for method in &stage.node.methods {
                let method_name = format!("{}.{}", stage.node.name.node, method.node.name.node);
                map.insert(
                    method.node.id,
                    DeclKey::new(&file, "method", &method_name),
                );
                for p in &method.node.params {
                    map.insert(
                        p.id,
                        DeclKey::new(&file, "param", format!("{}.{}", method_name, p.name.node)),
                    );
                }
            }
        }

        // System
        if let Some(system) = &program.system {
            let file = file_of(system.span.file_id);
            map.insert(
                system.node.id,
                DeclKey::new(&file, "system", &system.node.name.node),
            );
            for member in &system.node.members {
                map.insert(
                    member.id,
                    DeclKey::new(
                        &file,
                        "system_member",
                        format!("{}.{}", system.node.name.node, member.name.node),
                    ),
                );
            }
        }

        // Tests
        if let Some(tests) = &program.tests {
            let file = file_of(tests.span.file_id);
            map.insert(
                tests.node.id,
                DeclKey::new(&file, "tests", &tests.node.strategy),
            );
        }

        map
    }

    /// Register a monomorphized copy. The key uses the mangled name.
    pub fn insert_monomorphized(
        &mut self,
        uuid: Uuid,
        origin_file: &str,
        kind: &str,
        mangled_name: &str,
    ) {
        self.insert(uuid, DeclKey::new(origin_file, kind, mangled_name));
    }

    /// Number of entries in the map.
    pub fn len(&self) -> usize {
        self.uuid_to_key.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.uuid_to_key.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decl_key_equality() {
        let k1 = DeclKey::new("src/main.pluto", "function", "add");
        let k2 = DeclKey::new("src/main.pluto", "function", "add");
        assert_eq!(k1, k2);
        assert_eq!(k1.stable_hash(), k2.stable_hash());
    }

    #[test]
    fn decl_key_different_name() {
        let k1 = DeclKey::new("src/main.pluto", "function", "add");
        let k2 = DeclKey::new("src/main.pluto", "function", "sub");
        assert_ne!(k1, k2);
        assert_ne!(k1.stable_hash(), k2.stable_hash());
    }

    #[test]
    fn decl_key_different_kind() {
        let k1 = DeclKey::new("src/main.pluto", "function", "Foo");
        let k2 = DeclKey::new("src/main.pluto", "class", "Foo");
        assert_ne!(k1, k2);
    }

    #[test]
    fn decl_key_different_file() {
        let k1 = DeclKey::new("a.pluto", "function", "add");
        let k2 = DeclKey::new("b.pluto", "function", "add");
        assert_ne!(k1, k2);
    }

    #[test]
    fn decl_key_map_bidirectional() {
        let mut map = DeclKeyMap::new();
        let uuid = Uuid::new_v4();
        let key = DeclKey::new("test.pluto", "function", "main");
        map.insert(uuid, key.clone());

        assert_eq!(map.get_key(&uuid), Some(&key));
        assert_eq!(map.get_uuid(&key), Some(&uuid));
    }
}
