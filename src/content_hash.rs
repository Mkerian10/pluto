//! Content hashing for declarations. Produces API hashes (from resolved type info)
//! and implementation hashes (from source text) to detect what changed between compilations.

use std::collections::HashMap;

use sha2::{Sha256, Digest};
use uuid::Uuid;

use crate::decl_key::DeclKeyMap;
use crate::depgraph::DependencyGraph;
use crate::derived::{
    DerivedInfo, ResolvedClassInfo, ResolvedEnumInfo, ResolvedErrorInfo, ResolvedSignature,
    ResolvedTraitInfo,
};
use crate::modules::SourceMap;
use crate::parser::ast::Program;

/// A SHA-256 content hash.
pub type ContentHash = [u8; 32];

/// Paired hashes for a single declaration: API-level and full-implementation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeclHashes {
    /// Hash of the declaration's API surface (signature, field types, etc.).
    /// Two declarations with the same `api_hash` are interchangeable from callers' perspective.
    pub api_hash: ContentHash,
    /// Hash of the full source text of the declaration.
    pub impl_hash: ContentHash,
}

/// Hash all declarations in the program, producing API and impl hashes for each.
///
/// - `impl_hash`: SHA-256 of the source text span for the declaration.
/// - `api_hash`: SHA-256 of a normalized signature string built from DerivedInfo.
pub fn hash_declarations(
    program: &Program,
    source_map: &SourceMap,
    _key_map: &DeclKeyMap,
    derived: &DerivedInfo,
) -> HashMap<Uuid, DeclHashes> {
    let mut result = HashMap::new();

    // Functions
    for f in &program.functions {
        let impl_hash = hash_span(f.span, source_map);
        let api_hash = match derived.fn_signatures.get(&f.node.id) {
            Some(sig) => hash_fn_signature(&f.node.name.node, sig),
            None => impl_hash, // fallback: use impl hash if no resolved sig
        };
        result.insert(f.node.id, DeclHashes { api_hash, impl_hash });
    }

    // Classes
    for c in &program.classes {
        let impl_hash = hash_span(c.span, source_map);
        let api_hash = match derived.class_infos.get(&c.node.id) {
            Some(info) => hash_class_api(&c.node.name.node, info),
            None => impl_hash,
        };
        result.insert(c.node.id, DeclHashes { api_hash, impl_hash });

        // Class methods
        for m in &c.node.methods {
            let m_impl_hash = hash_span(m.span, source_map);
            let m_api_hash = match derived.fn_signatures.get(&m.node.id) {
                Some(sig) => hash_fn_signature(&m.node.name.node, sig),
                None => m_impl_hash,
            };
            result.insert(m.node.id, DeclHashes { api_hash: m_api_hash, impl_hash: m_impl_hash });
        }
    }

    // Enums
    for e in &program.enums {
        let impl_hash = hash_span(e.span, source_map);
        let api_hash = match derived.enum_infos.get(&e.node.id) {
            Some(info) => hash_enum_api(&e.node.name.node, info),
            None => impl_hash,
        };
        result.insert(e.node.id, DeclHashes { api_hash, impl_hash });
    }

    // Traits
    for t in &program.traits {
        let impl_hash = hash_span(t.span, source_map);
        let api_hash = match derived.trait_infos.get(&t.node.id) {
            Some(info) => hash_trait_api(&t.node.name.node, info),
            None => impl_hash,
        };
        result.insert(t.node.id, DeclHashes { api_hash, impl_hash });
    }

    // Errors
    for e in &program.errors {
        let impl_hash = hash_span(e.span, source_map);
        let api_hash = match derived.error_infos.get(&e.node.id) {
            Some(info) => hash_error_api(&e.node.name.node, info),
            None => impl_hash,
        };
        result.insert(e.node.id, DeclHashes { api_hash, impl_hash });
    }

    // App
    if let Some(app) = &program.app {
        let impl_hash = hash_span(app.span, source_map);
        result.insert(app.node.id, DeclHashes { api_hash: impl_hash, impl_hash });
        for m in &app.node.methods {
            let m_impl_hash = hash_span(m.span, source_map);
            let m_api_hash = match derived.fn_signatures.get(&m.node.id) {
                Some(sig) => hash_fn_signature(&m.node.name.node, sig),
                None => m_impl_hash,
            };
            result.insert(m.node.id, DeclHashes { api_hash: m_api_hash, impl_hash: m_impl_hash });
        }
    }

    // Stages
    for stage in &program.stages {
        let impl_hash = hash_span(stage.span, source_map);
        result.insert(stage.node.id, DeclHashes { api_hash: impl_hash, impl_hash });
        for m in &stage.node.methods {
            let m_impl_hash = hash_span(m.span, source_map);
            let m_api_hash = match derived.fn_signatures.get(&m.node.id) {
                Some(sig) => hash_fn_signature(&m.node.name.node, sig),
                None => m_impl_hash,
            };
            result.insert(m.node.id, DeclHashes { api_hash: m_api_hash, impl_hash: m_impl_hash });
        }
    }

    result
}

/// Compute a transitive dependency hash for a declaration.
///
/// This is own `impl_hash` combined with the sorted `api_hash` values of all
/// transitive dependencies. If this hash hasn't changed between compilations,
/// the compilation result is reusable.
pub fn dep_hash(
    id: Uuid,
    content_hashes: &HashMap<Uuid, DeclHashes>,
    dep_graph: &DependencyGraph,
) -> ContentHash {
    let mut hasher = Sha256::new();

    // Own impl hash
    if let Some(hashes) = content_hashes.get(&id) {
        hasher.update(hashes.impl_hash);
    }

    // Collect transitive dependency api_hashes
    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![id];
    let mut dep_hashes: Vec<ContentHash> = Vec::new();

    while let Some(current) = stack.pop() {
        for &(dep_id, _edge) in dep_graph.deps_of(current) {
            if visited.insert(dep_id) {
                if let Some(h) = content_hashes.get(&dep_id) {
                    dep_hashes.push(h.api_hash);
                }
                stack.push(dep_id);
            }
        }
    }

    // Sort for determinism
    dep_hashes.sort();
    for h in &dep_hashes {
        hasher.update(h);
    }

    hasher.finalize().into()
}

// --- Internal helpers ---

fn sha256(data: &[u8]) -> ContentHash {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn hash_span(span: crate::span::Span, source_map: &SourceMap) -> ContentHash {
    if let Some((_path, source)) = source_map.get_source(span.file_id) {
        let start = span.start.min(source.len());
        let end = span.end.min(source.len());
        sha256(source[start..end].as_bytes())
    } else {
        [0u8; 32]
    }
}

fn hash_fn_signature(name: &str, sig: &ResolvedSignature) -> ContentHash {
    let mut s = format!("fn {}(", name);
    for (i, p) in sig.param_types.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&format!("{}", p));
    }
    s.push_str(&format!(") -> {}", sig.return_type));
    if sig.is_fallible {
        s.push_str(" !");
    }
    sha256(s.as_bytes())
}

fn hash_class_api(name: &str, info: &ResolvedClassInfo) -> ContentHash {
    let mut s = format!("class {}", name);
    // Fields
    for f in &info.fields {
        s.push_str(&format!("\n  {}: {}", f.name, f.ty));
        if f.is_injected {
            s.push_str(" [injected]");
        }
    }
    // Method signatures
    for (mname, msig) in &info.methods {
        s.push_str(&format!("\n  fn {}(", mname));
        for (i, p) in msig.param_types.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&format!("{}", p));
        }
        s.push_str(&format!(") -> {}", msig.return_type));
        if msig.is_fallible {
            s.push_str(" !");
        }
    }
    // Implemented traits
    for t in &info.impl_traits {
        s.push_str(&format!("\n  impl {}", t));
    }
    sha256(s.as_bytes())
}

fn hash_enum_api(name: &str, info: &ResolvedEnumInfo) -> ContentHash {
    let mut s = format!("enum {}", name);
    for v in &info.variants {
        s.push_str(&format!("\n  {}", v.name));
        for f in &v.fields {
            s.push_str(&format!("\n    {}: {}", f.name, f.ty));
        }
    }
    sha256(s.as_bytes())
}

fn hash_trait_api(name: &str, info: &ResolvedTraitInfo) -> ContentHash {
    let mut s = format!("trait {}", name);
    for (mname, msig) in &info.methods {
        s.push_str(&format!("\n  fn {}(", mname));
        for (i, p) in msig.param_types.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&format!("{}", p));
        }
        s.push_str(&format!(") -> {}", msig.return_type));
        if msig.is_fallible {
            s.push_str(" !");
        }
    }
    sha256(s.as_bytes())
}

fn hash_error_api(name: &str, info: &ResolvedErrorInfo) -> ContentHash {
    let mut s = format!("error {}", name);
    for f in &info.fields {
        s.push_str(&format!("\n  {}: {}", f.name, f.ty));
    }
    sha256(s.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::derived::ResolvedSignature;
    use crate::typeck::types::PlutoType;

    #[test]
    fn test_hash_deterministic() {
        let sig = ResolvedSignature {
            param_types: vec![PlutoType::Int, PlutoType::String],
            return_type: PlutoType::Bool,
            is_fallible: false,
        };
        let h1 = hash_fn_signature("foo", &sig);
        let h2 = hash_fn_signature("foo", &sig);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_different_name() {
        let sig = ResolvedSignature {
            param_types: vec![PlutoType::Int],
            return_type: PlutoType::Void,
            is_fallible: false,
        };
        let h1 = hash_fn_signature("foo", &sig);
        let h2 = hash_fn_signature("bar", &sig);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_different_params() {
        let sig1 = ResolvedSignature {
            param_types: vec![PlutoType::Int],
            return_type: PlutoType::Void,
            is_fallible: false,
        };
        let sig2 = ResolvedSignature {
            param_types: vec![PlutoType::String],
            return_type: PlutoType::Void,
            is_fallible: false,
        };
        assert_ne!(hash_fn_signature("foo", &sig1), hash_fn_signature("foo", &sig2));
    }

    #[test]
    fn test_hash_fallible_differs() {
        let sig1 = ResolvedSignature {
            param_types: vec![],
            return_type: PlutoType::Void,
            is_fallible: false,
        };
        let sig2 = ResolvedSignature {
            param_types: vec![],
            return_type: PlutoType::Void,
            is_fallible: true,
        };
        assert_ne!(hash_fn_signature("foo", &sig1), hash_fn_signature("foo", &sig2));
    }

    #[test]
    fn test_api_vs_impl_hash() {
        // Two functions with the same signature but different bodies
        // should have the same api_hash but different impl_hash.
        let sig = ResolvedSignature {
            param_types: vec![PlutoType::Int],
            return_type: PlutoType::Int,
            is_fallible: false,
        };
        let api1 = hash_fn_signature("add", &sig);
        let api2 = hash_fn_signature("add", &sig);
        assert_eq!(api1, api2);

        // Different source text → different impl hashes
        let impl1 = sha256(b"fn add(x: int) -> int { x + 1 }");
        let impl2 = sha256(b"fn add(x: int) -> int { x + 2 }");
        assert_ne!(impl1, impl2);
    }

    #[test]
    fn test_dep_hash_no_deps() {
        let id = Uuid::from_u128(1);
        let mut hashes = HashMap::new();
        hashes.insert(id, DeclHashes {
            api_hash: sha256(b"api"),
            impl_hash: sha256(b"impl"),
        });
        let graph = DependencyGraph::new();
        let h = dep_hash(id, &hashes, &graph);
        // Should be deterministic
        let h2 = dep_hash(id, &hashes, &graph);
        assert_eq!(h, h2);
    }

    #[test]
    fn test_dep_hash_with_deps() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);

        let mut hashes = HashMap::new();
        hashes.insert(a, DeclHashes {
            api_hash: sha256(b"a_api"),
            impl_hash: sha256(b"a_impl"),
        });
        hashes.insert(b, DeclHashes {
            api_hash: sha256(b"b_api"),
            impl_hash: sha256(b"b_impl"),
        });

        let mut graph = DependencyGraph::new();
        graph.add_edge(a, b, crate::depgraph::DepEdge::Calls);

        let h_with_dep = dep_hash(a, &hashes, &graph);
        let h_no_dep = dep_hash(a, &hashes, &DependencyGraph::new());
        assert_ne!(h_with_dep, h_no_dep);
    }
}
