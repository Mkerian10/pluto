//! Incremental compilation cache.
//!
//! Stores results from a previous compilation (content hashes, dependency graph,
//! TypeEnv side effects from body checking) so that unchanged function bodies
//! can be skipped on re-compilation.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::content_hash::{ContentHash, DeclHashes};
use crate::decl_key::{DeclKey, DeclKeyMap};
use crate::depgraph::{ChangeKind, DependencyGraph};
use crate::span::Span;
use crate::typeck::env::{Instantiation, MethodResolution, ScopeResolution, TypeEnv};
use crate::typeck::types::PlutoType;

/// Cached results from a previous compilation, keyed by DeclKey for stability across re-parses.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CompilationCache {
    /// Content hashes from last compilation, keyed by DeclKey.
    pub decl_hashes: HashMap<DeclKey, DeclHashes>,
    /// Dependency graph from last compilation (UUID-based, old UUIDs).
    pub dep_graph: DependencyGraph,
    /// DeclKey→UUID mapping from last compilation (for mapping old→new UUIDs).
    pub old_key_map: DeclKeyMap,
    /// Cached TypeEnv side effects from body checking, keyed by DeclKey.
    pub body_effects: HashMap<DeclKey, CachedBodyEffects>,
    /// Cached error sets from error inference, keyed by DeclKey (mangled fn name → error names).
    pub fn_error_sets: HashMap<DeclKey, HashSet<String>>,
}

/// TypeEnv side effects produced by checking a single function body.
/// These are the maps that `check_function` populates as it walks expressions.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CachedBodyEffects {
    /// method_resolutions entries: (mangled_fn_name, span_start) → MethodResolution
    pub method_resolutions: Vec<((String, usize), MethodResolution)>,
    /// spawn_target_fns entries: (span_start, span_end) → fn_name
    pub spawn_target_fns: Vec<((usize, usize), String)>,
    /// closure_captures entries: (start, end) → [(name, type)]
    pub closure_captures: Vec<((usize, usize), Vec<(String, PlutoType)>)>,
    /// closure_fns entries: lifted_name → [(name, type)]
    pub closure_fns: Vec<(String, Vec<(String, PlutoType)>)>,
    /// closure_return_types entries: (start, end) → PlutoType
    pub closure_return_types: Vec<((usize, usize), PlutoType)>,
    /// instantiations triggered by this function body
    pub instantiations: Vec<Instantiation>,
    /// generic_rewrites entries: (start, end) → rewritten_name
    pub generic_rewrites: Vec<((usize, usize), String)>,
    /// fallible_builtin_calls entries: (mangled_fn_name, span_start)
    pub fallible_builtin_calls: Vec<(String, usize)>,
    /// variable_decls entries: (var_name, scope_depth) → Span
    pub variable_decls: Vec<((String, usize), Span)>,
    /// variable_reads entries: (var_name, scope_depth)
    pub variable_reads: Vec<(String, usize)>,
    /// scope_resolutions entries (for DI scope blocks inside this function)
    pub scope_resolutions: Vec<((usize, usize), ScopeResolution)>,
}

/// Which declarations changed between compilations.
#[derive(Debug, Clone, Default)]
pub struct ChangeSet {
    /// DeclKeys whose impl_hash changed (body changed, signature same).
    pub impl_changed: HashSet<DeclKey>,
    /// DeclKeys whose api_hash changed (signature changed).
    pub api_changed: HashSet<DeclKey>,
    /// DeclKeys that are new (no match in old cache).
    pub added: HashSet<DeclKey>,
    /// DeclKeys in old cache but not in new program.
    pub removed: HashSet<DeclKey>,
    /// All DeclKeys that need re-checking (changed + affected via dep graph).
    pub affected: HashSet<DeclKey>,
}

/// Statistics about incremental compilation.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct IncrementalStats {
    pub total_decls: usize,
    pub changed_decls: usize,
    pub affected_decls: usize,
    pub skipped_decls: usize,
    pub cache_hit: bool,
}

/// Detect what changed between the old cache and the new program.
///
/// Compares content hashes from the old cache against new hashes computed from
/// the current program. Returns a `ChangeSet` describing what changed and what's
/// affected via the dependency graph.
pub fn detect_changes(
    old_cache: &CompilationCache,
    new_key_map: &DeclKeyMap,
    new_hashes: &HashMap<Uuid, DeclHashes>,
) -> ChangeSet {
    let mut cs = ChangeSet::default();

    // Build set of all new DeclKeys
    let new_keys: HashSet<&DeclKey> = new_key_map.uuid_to_key.values().collect();

    // Check each new declaration against old cache
    for (uuid, new_key) in &new_key_map.uuid_to_key {
        match old_cache.decl_hashes.get(new_key) {
            None => {
                // New declaration — not in old cache
                cs.added.insert(new_key.clone());
            }
            Some(old_hashes) => {
                if let Some(new_h) = new_hashes.get(uuid) {
                    if old_hashes.api_hash != new_h.api_hash {
                        cs.api_changed.insert(new_key.clone());
                    } else if old_hashes.impl_hash != new_h.impl_hash {
                        cs.impl_changed.insert(new_key.clone());
                    }
                    // else: unchanged
                }
            }
        }
    }

    // Check for removed declarations
    for old_key in old_cache.decl_hashes.keys() {
        if !new_keys.contains(old_key) {
            cs.removed.insert(old_key.clone());
        }
    }

    // Compute affected set via dependency graph:
    // Map changed DeclKeys → old UUIDs → run dep_graph.affected() → map back to DeclKeys
    let mut changed_uuids: HashMap<Uuid, ChangeKind> = HashMap::new();

    for key in cs.api_changed.iter().chain(cs.added.iter()).chain(cs.removed.iter()) {
        if let Some(&old_uuid) = old_cache.old_key_map.get_uuid(key) {
            changed_uuids.insert(old_uuid, ChangeKind::ApiAndImpl);
        }
    }
    for key in &cs.impl_changed {
        if let Some(&old_uuid) = old_cache.old_key_map.get_uuid(key) {
            changed_uuids.entry(old_uuid).or_insert(ChangeKind::ImplOnly);
        }
    }

    let affected_uuids = old_cache.dep_graph.affected(&changed_uuids);

    // Map affected UUIDs back to DeclKeys
    for uuid in &affected_uuids {
        if let Some(key) = old_cache.old_key_map.get_key(uuid) {
            cs.affected.insert(key.clone());
        }
    }

    // Union with directly changed + added sets (they are always affected)
    cs.affected.extend(cs.api_changed.iter().cloned());
    cs.affected.extend(cs.impl_changed.iter().cloned());
    cs.affected.extend(cs.added.iter().cloned());

    cs
}

/// Restore cached body effects for unaffected functions into the TypeEnv.
///
/// For each unaffected DeclKey that has cached effects, inserts the cached
/// entries back into the appropriate TypeEnv maps.
pub fn restore_body_effects(
    cached_effects: &HashMap<DeclKey, CachedBodyEffects>,
    unaffected_keys: &HashSet<DeclKey>,
    env: &mut TypeEnv,
) {
    for key in unaffected_keys {
        if let Some(effects) = cached_effects.get(key) {
            for ((fn_name, span_start), resolution) in &effects.method_resolutions {
                env.method_resolutions.insert(
                    (fn_name.clone(), *span_start),
                    resolution.clone(),
                );
            }
            for ((start, end), fn_name) in &effects.spawn_target_fns {
                env.spawn_target_fns.insert((*start, *end), fn_name.clone());
            }
            for ((start, end), captures) in &effects.closure_captures {
                env.closure_captures.insert((*start, *end), captures.clone());
            }
            for (name, captures) in &effects.closure_fns {
                env.closure_fns.insert(name.clone(), captures.clone());
            }
            for ((start, end), ret_ty) in &effects.closure_return_types {
                env.closure_return_types.insert((*start, *end), ret_ty.clone());
            }
            for inst in &effects.instantiations {
                env.instantiations.insert(inst.clone());
            }
            for ((start, end), name) in &effects.generic_rewrites {
                env.generic_rewrites.insert((*start, *end), name.clone());
            }
            for (fn_name, span_start) in &effects.fallible_builtin_calls {
                env.fallible_builtin_calls.insert((fn_name.clone(), *span_start));
            }
            for ((name, depth), span) in &effects.variable_decls {
                env.variable_decls.insert((name.clone(), *depth), *span);
            }
            for (name, depth) in &effects.variable_reads {
                env.variable_reads.insert((name.clone(), *depth));
            }
            for ((start, end), resolution) in &effects.scope_resolutions {
                env.scope_resolutions.insert((*start, *end), resolution.clone());
            }
        }
    }
}

/// Restore cached fn_errors for unaffected functions into the TypeEnv.
pub fn restore_fn_errors(
    cached_fn_errors: &HashMap<DeclKey, HashSet<String>>,
    unaffected_keys: &HashSet<DeclKey>,
    key_map: &DeclKeyMap,
    env: &mut TypeEnv,
) {
    // We need to map DeclKey → mangled function name in env.fn_errors.
    // The DeclKey name for functions is the plain name, for methods it's "Class.method".
    // The env.fn_errors key is the mangled name: "fn_name" or "Class$method".
    for key in unaffected_keys {
        if let Some(errors) = cached_fn_errors.get(key) {
            let mangled = decl_key_to_fn_errors_key(key);
            if !errors.is_empty() {
                env.fn_errors.insert(mangled, errors.clone());
            }
        }
    }
    let _ = key_map; // used conceptually but mapping is done via key name
}

/// Convert a DeclKey to the mangled name used in env.fn_errors.
fn decl_key_to_fn_errors_key(key: &DeclKey) -> String {
    if key.kind == "method" {
        // DeclKey name is "Class.method", env key is "Class$method"
        let parts: Vec<&str> = key.name.splitn(2, '.').collect();
        if parts.len() == 2 {
            format!("{}${}", parts[0], parts[1])
        } else {
            key.name.clone()
        }
    } else {
        key.name.clone()
    }
}

/// Capture TypeEnv side effects for a specific function body.
///
/// Extracts entries from TypeEnv that were produced by checking the function
/// identified by its mangled name and span range.
pub fn capture_body_effects(
    env: &TypeEnv,
    fn_mangled_name: &str,
    fn_span: (usize, usize),
) -> CachedBodyEffects {
    let mut effects = CachedBodyEffects::default();

    // method_resolutions: keyed by (fn_name, span_start)
    for ((name, span_start), resolution) in &env.method_resolutions {
        if name == fn_mangled_name {
            effects
                .method_resolutions
                .push(((name.clone(), *span_start), resolution.clone()));
        }
    }

    // spawn_target_fns: keyed by (start, end) — check span containment
    for (&(start, end), fn_name) in &env.spawn_target_fns {
        if start >= fn_span.0 && end <= fn_span.1 {
            effects
                .spawn_target_fns
                .push(((start, end), fn_name.clone()));
        }
    }

    // closure_captures: keyed by (start, end)
    for (&(start, end), captures) in &env.closure_captures {
        if start >= fn_span.0 && end <= fn_span.1 {
            effects
                .closure_captures
                .push(((start, end), captures.clone()));
        }
    }

    // closure_fns: keyed by lifted name — match prefix pattern
    // Lifted closures are named like "__closure_{fn_name}_{N}" but we can't
    // reliably prefix-match. Instead, we capture ALL closure_fns during body check
    // since they're only added during check_function. We'll track them per-function
    // by checking if the closure span is within the function span.
    // For now, we skip this — closure_fns are populated during closure lifting,
    // not during body checking.

    // closure_return_types: keyed by (start, end)
    for (&(start, end), ret_ty) in &env.closure_return_types {
        if start >= fn_span.0 && end <= fn_span.1 {
            effects
                .closure_return_types
                .push(((start, end), ret_ty.clone()));
        }
    }

    // instantiations: we can't easily attribute to a specific function,
    // so we skip — they're collected globally and re-run during monomorphize.

    // generic_rewrites: keyed by (start, end)
    for (&(start, end), name) in &env.generic_rewrites {
        if start >= fn_span.0 && end <= fn_span.1 {
            effects
                .generic_rewrites
                .push(((start, end), name.clone()));
        }
    }

    // fallible_builtin_calls: keyed by (fn_name, span_start)
    for (name, span_start) in &env.fallible_builtin_calls {
        if name == fn_mangled_name {
            effects
                .fallible_builtin_calls
                .push((name.clone(), *span_start));
        }
    }

    // variable_decls/reads: these are keyed by (name, scope_depth), not span,
    // so we can't easily filter by function. For warnings, this is fine —
    // the warning pass re-runs fully regardless.

    // scope_resolutions: keyed by (start, end)
    for (&(start, end), resolution) in &env.scope_resolutions {
        if start >= fn_span.0 && end <= fn_span.1 {
            effects
                .scope_resolutions
                .push(((start, end), resolution.clone()));
        }
    }

    effects
}

/// Update the cache with results from the current compilation.
pub fn update_cache(
    cache: &mut CompilationCache,
    new_key_map: &DeclKeyMap,
    new_hashes: &HashMap<Uuid, DeclHashes>,
    new_dep_graph: &DependencyGraph,
    new_effects: &HashMap<DeclKey, CachedBodyEffects>,
    new_fn_errors: &HashMap<DeclKey, HashSet<String>>,
) {
    // Replace dependency graph and key map entirely
    cache.dep_graph = new_dep_graph.clone();
    cache.old_key_map = new_key_map.clone();

    // Update hashes: convert UUID-keyed hashes to DeclKey-keyed
    cache.decl_hashes.clear();
    for (uuid, hashes) in new_hashes {
        if let Some(key) = new_key_map.get_key(uuid) {
            cache.decl_hashes.insert(key.clone(), hashes.clone());
        }
    }

    // Merge body effects: new effects override, keep unaffected from old cache
    for (key, effects) in new_effects {
        cache.body_effects.insert(key.clone(), effects.clone());
    }

    // Merge fn_errors similarly
    for (key, errors) in new_fn_errors {
        cache.fn_error_sets.insert(key.clone(), errors.clone());
    }

    // Remove entries for removed declarations
    cache.body_effects.retain(|k, _| cache.decl_hashes.contains_key(k));
    cache.fn_error_sets.retain(|k, _| cache.decl_hashes.contains_key(k));
}

/// Compute impl-only hashes from source text (no type info needed).
/// Used for the initial change detection before type checking.
pub fn compute_impl_hashes(
    program: &crate::parser::ast::Program,
    source_map: &crate::modules::SourceMap,
) -> HashMap<Uuid, DeclHashes> {
    use sha2::{Digest, Sha256};

    let hash_span = |span: Span| -> ContentHash {
        if let Some((_path, source)) = source_map.get_source(span.file_id) {
            let start = span.start.min(source.len());
            let end = span.end.min(source.len());
            let mut hasher = Sha256::new();
            hasher.update(source[start..end].as_bytes());
            hasher.finalize().into()
        } else {
            [0u8; 32]
        }
    };

    let mut result = HashMap::new();

    for f in &program.functions {
        let h = hash_span(f.span);
        // Use impl_hash for both api and impl — we only use impl_hash for initial comparison
        result.insert(f.node.id, DeclHashes { api_hash: h, impl_hash: h });
    }

    for c in &program.classes {
        let h = hash_span(c.span);
        result.insert(c.node.id, DeclHashes { api_hash: h, impl_hash: h });
        for m in &c.node.methods {
            let mh = hash_span(m.span);
            result.insert(m.node.id, DeclHashes { api_hash: mh, impl_hash: mh });
        }
    }

    for e in &program.enums {
        let h = hash_span(e.span);
        result.insert(e.node.id, DeclHashes { api_hash: h, impl_hash: h });
    }

    for t in &program.traits {
        let h = hash_span(t.span);
        result.insert(t.node.id, DeclHashes { api_hash: h, impl_hash: h });
    }

    for e in &program.errors {
        let h = hash_span(e.span);
        result.insert(e.node.id, DeclHashes { api_hash: h, impl_hash: h });
    }

    if let Some(app) = &program.app {
        let h = hash_span(app.span);
        result.insert(app.node.id, DeclHashes { api_hash: h, impl_hash: h });
        for m in &app.node.methods {
            let mh = hash_span(m.span);
            result.insert(m.node.id, DeclHashes { api_hash: mh, impl_hash: mh });
        }
    }

    for stage in &program.stages {
        let h = hash_span(stage.span);
        result.insert(stage.node.id, DeclHashes { api_hash: h, impl_hash: h });
        for m in &stage.node.methods {
            let mh = hash_span(m.span);
            result.insert(m.node.id, DeclHashes { api_hash: mh, impl_hash: mh });
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key(name: &str) -> DeclKey {
        DeclKey::new("test.pluto", "function", name)
    }

    fn make_hash(data: &[u8]) -> ContentHash {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(data);
        h.finalize().into()
    }

    #[test]
    fn test_empty_cache_detects_all_as_new() {
        let old_cache = CompilationCache::default();
        let mut new_key_map = DeclKeyMap::new();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        new_key_map.insert(uuid1, make_key("foo"));
        new_key_map.insert(uuid2, make_key("bar"));

        let mut new_hashes = HashMap::new();
        new_hashes.insert(uuid1, DeclHashes {
            api_hash: make_hash(b"foo_api"),
            impl_hash: make_hash(b"foo_impl"),
        });
        new_hashes.insert(uuid2, DeclHashes {
            api_hash: make_hash(b"bar_api"),
            impl_hash: make_hash(b"bar_impl"),
        });

        let cs = detect_changes(&old_cache, &new_key_map, &new_hashes);
        assert_eq!(cs.added.len(), 2);
        assert!(cs.added.contains(&make_key("foo")));
        assert!(cs.added.contains(&make_key("bar")));
        assert!(cs.impl_changed.is_empty());
        assert!(cs.api_changed.is_empty());
    }

    #[test]
    fn test_unchanged_decls_not_affected() {
        let key_foo = make_key("foo");
        let key_bar = make_key("bar");
        let hash_foo = DeclHashes {
            api_hash: make_hash(b"foo_api"),
            impl_hash: make_hash(b"foo_impl"),
        };
        let hash_bar = DeclHashes {
            api_hash: make_hash(b"bar_api"),
            impl_hash: make_hash(b"bar_impl"),
        };

        let old_uuid1 = Uuid::new_v4();
        let old_uuid2 = Uuid::new_v4();

        let mut old_cache = CompilationCache::default();
        old_cache.decl_hashes.insert(key_foo.clone(), hash_foo.clone());
        old_cache.decl_hashes.insert(key_bar.clone(), hash_bar.clone());
        old_cache.old_key_map.insert(old_uuid1, key_foo.clone());
        old_cache.old_key_map.insert(old_uuid2, key_bar.clone());

        // New compilation with same hashes
        let new_uuid1 = Uuid::new_v4();
        let new_uuid2 = Uuid::new_v4();
        let mut new_key_map = DeclKeyMap::new();
        new_key_map.insert(new_uuid1, key_foo.clone());
        new_key_map.insert(new_uuid2, key_bar.clone());

        let mut new_hashes = HashMap::new();
        new_hashes.insert(new_uuid1, hash_foo);
        new_hashes.insert(new_uuid2, hash_bar);

        let cs = detect_changes(&old_cache, &new_key_map, &new_hashes);
        assert!(cs.added.is_empty());
        assert!(cs.impl_changed.is_empty());
        assert!(cs.api_changed.is_empty());
        assert!(cs.removed.is_empty());
        assert!(cs.affected.is_empty());
    }

    #[test]
    fn test_impl_change_detected() {
        let key = make_key("foo");
        let old_hash = DeclHashes {
            api_hash: make_hash(b"foo_api"),
            impl_hash: make_hash(b"foo_impl_v1"),
        };
        let new_hash = DeclHashes {
            api_hash: make_hash(b"foo_api"),       // same API
            impl_hash: make_hash(b"foo_impl_v2"),  // different impl
        };

        let old_uuid = Uuid::new_v4();
        let mut old_cache = CompilationCache::default();
        old_cache.decl_hashes.insert(key.clone(), old_hash);
        old_cache.old_key_map.insert(old_uuid, key.clone());

        let new_uuid = Uuid::new_v4();
        let mut new_key_map = DeclKeyMap::new();
        new_key_map.insert(new_uuid, key.clone());

        let mut new_hashes = HashMap::new();
        new_hashes.insert(new_uuid, new_hash);

        let cs = detect_changes(&old_cache, &new_key_map, &new_hashes);
        assert!(cs.impl_changed.contains(&key));
        assert!(cs.api_changed.is_empty());
        assert!(cs.affected.contains(&key));
    }

    #[test]
    fn test_api_change_propagates() {
        use crate::depgraph::DepEdge;

        let key_a = make_key("a");
        let key_b = make_key("b");

        let old_uuid_a = Uuid::new_v4();
        let old_uuid_b = Uuid::new_v4();

        let mut old_cache = CompilationCache::default();
        old_cache.decl_hashes.insert(key_a.clone(), DeclHashes {
            api_hash: make_hash(b"a_api"),
            impl_hash: make_hash(b"a_impl"),
        });
        old_cache.decl_hashes.insert(key_b.clone(), DeclHashes {
            api_hash: make_hash(b"b_api_v1"),
            impl_hash: make_hash(b"b_impl_v1"),
        });
        old_cache.old_key_map.insert(old_uuid_a, key_a.clone());
        old_cache.old_key_map.insert(old_uuid_b, key_b.clone());
        // a calls b
        old_cache.dep_graph.add_edge(old_uuid_a, old_uuid_b, DepEdge::Calls);

        // New: b's API changed
        let new_uuid_a = Uuid::new_v4();
        let new_uuid_b = Uuid::new_v4();
        let mut new_key_map = DeclKeyMap::new();
        new_key_map.insert(new_uuid_a, key_a.clone());
        new_key_map.insert(new_uuid_b, key_b.clone());

        let mut new_hashes = HashMap::new();
        new_hashes.insert(new_uuid_a, DeclHashes {
            api_hash: make_hash(b"a_api"),
            impl_hash: make_hash(b"a_impl"),
        });
        new_hashes.insert(new_uuid_b, DeclHashes {
            api_hash: make_hash(b"b_api_v2"),  // API changed
            impl_hash: make_hash(b"b_impl_v2"),
        });

        let cs = detect_changes(&old_cache, &new_key_map, &new_hashes);
        assert!(cs.api_changed.contains(&key_b));
        // a should be affected because it depends on b via Calls edge
        assert!(cs.affected.contains(&key_a));
        assert!(cs.affected.contains(&key_b));
    }

    #[test]
    fn test_restore_body_effects() {
        let key = make_key("foo");
        let mut effects_map = HashMap::new();
        let mut effects = CachedBodyEffects::default();
        effects.method_resolutions.push((
            ("foo".to_string(), 42),
            MethodResolution::Builtin,
        ));
        effects.spawn_target_fns.push(((10, 20), "bar".to_string()));
        effects_map.insert(key.clone(), effects);

        let unaffected: HashSet<DeclKey> = [key].into_iter().collect();
        let mut env = TypeEnv::new();

        restore_body_effects(&effects_map, &unaffected, &mut env);

        assert!(env.method_resolutions.contains_key(&("foo".to_string(), 42)));
        assert!(env.spawn_target_fns.contains_key(&(10, 20)));
    }

    #[test]
    fn test_incremental_stats() {
        let stats = IncrementalStats {
            total_decls: 10,
            changed_decls: 2,
            affected_decls: 3,
            skipped_decls: 7,
            cache_hit: true,
        };
        assert_eq!(stats.total_decls, 10);
        assert_eq!(stats.skipped_decls, 7);
        assert!(stats.cache_hit);
    }
}
