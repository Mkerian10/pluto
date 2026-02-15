use std::path::Path;

use crate::binary;
use crate::parser::ast::{
    EnumVariant, Field, Function, Param, Program, TraitMethod, TypeExpr,
};
use crate::span::Spanned;
use crate::xref;

/// Compare two TypeExpr values semantically (ignoring spans).
fn types_equal(t1: &Spanned<TypeExpr>, t2: &Spanned<TypeExpr>) -> bool {
    match (&t1.node, &t2.node) {
        (TypeExpr::Named(n1), TypeExpr::Named(n2)) => n1 == n2,
        (TypeExpr::Array(a1), TypeExpr::Array(a2)) => types_equal(a1, a2),
        (
            TypeExpr::Qualified {
                module: m1,
                name: n1,
            },
            TypeExpr::Qualified {
                module: m2,
                name: n2,
            },
        ) => m1 == m2 && n1 == n2,
        (
            TypeExpr::Fn {
                params: p1,
                return_type: r1,
            },
            TypeExpr::Fn {
                params: p2,
                return_type: r2,
            },
        ) => {
            p1.len() == p2.len()
                && p1.iter().zip(p2.iter()).all(|(a, b)| types_equal(a, b))
                && types_equal(r1, r2)
        }
        (
            TypeExpr::Generic {
                name: n1,
                type_args: args1,
            },
            TypeExpr::Generic {
                name: n2,
                type_args: args2,
            },
        ) => {
            n1 == n2
                && args1.len() == args2.len()
                && args1
                    .iter()
                    .zip(args2.iter())
                    .all(|(a, b)| types_equal(a, b))
        }
        (TypeExpr::Nullable(t1), TypeExpr::Nullable(t2)) => types_equal(t1, t2),
        (TypeExpr::Stream(t1), TypeExpr::Stream(t2)) => types_equal(t1, t2),
        _ => false,
    }
}

/// Summary of what changed during a sync operation.
#[derive(Debug, Clone)]
pub struct SyncResult {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<String>,
    pub unchanged: usize,
}

/// Errors that can occur during sync.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("failed to read .pt file: {0}")]
    ReadPt(std::io::Error),
    #[error("failed to read .pluto file: {0}")]
    ReadPluto(std::io::Error),
    #[error("failed to write .pluto file: {0}")]
    WritePluto(std::io::Error),
    #[error("failed to parse .pt file: {0}")]
    Parse(crate::diagnostics::CompileError),
    #[error("failed to deserialize .pluto binary: {0}")]
    Deserialize(binary::BinaryError),
    #[error("failed to serialize .pluto binary: {0}")]
    Serialize(binary::BinaryError),
}

/// Sync human edits from a `.pt` text file back to a `.pluto` binary,
/// preserving UUIDs from the existing binary where declarations match by name.
///
/// If `pluto_path` does not exist, creates a fresh `.pluto` binary (all new UUIDs).
pub fn sync_pt_to_pluto(pt_path: &Path, pluto_path: &Path) -> Result<SyncResult, SyncError> {
    // 1. Read and parse the .pt text file
    let pt_source = std::fs::read_to_string(pt_path).map_err(SyncError::ReadPt)?;
    let mut new_program = crate::parse_for_editing(&pt_source).map_err(SyncError::Parse)?;

    // 2. Try to load the existing .pluto binary (if it exists)
    let old_program = if pluto_path.exists() {
        let data = std::fs::read(pluto_path).map_err(SyncError::ReadPluto)?;
        if binary::is_binary_format(&data) {
            let (program, _source, _derived) =
                binary::deserialize_program(&data).map_err(SyncError::Deserialize)?;
            Some(program)
        } else {
            None
        }
    } else {
        None
    };

    // 3. Transplant UUIDs from old to new (if old exists)
    let result = if let Some(old) = &old_program {
        transplant_program(&mut new_program, old)
    } else {
        // No old program — everything is "added"
        let mut added = Vec::new();
        for f in &new_program.functions {
            added.push(format!("fn {}", f.node.name.node));
        }
        for c in &new_program.classes {
            added.push(format!("class {}", c.node.name.node));
        }
        for e in &new_program.enums {
            added.push(format!("enum {}", e.node.name.node));
        }
        for t in &new_program.traits {
            added.push(format!("trait {}", t.node.name.node));
        }
        for err in &new_program.errors {
            added.push(format!("error {}", err.node.name.node));
        }
        if let Some(app) = &new_program.app {
            added.push(format!("app {}", app.node.name.node));
        }
        for stage in &new_program.stages {
            added.push(format!("stage {}", stage.node.name.node));
        }
        SyncResult {
            added,
            removed: Vec::new(),
            modified: Vec::new(),
            unchanged: 0,
        }
    };

    // 4. Resolve cross-references on the updated program
    xref::resolve_cross_refs(&mut new_program);

    // 5. Write to .pluto binary with stale derived data (meta = None)
    crate::plto_store::write_canonical_stale(pluto_path, &new_program, &pt_source)
        .map_err(|e| match e {
            crate::plto_store::StoreError::Binary(b) => SyncError::Serialize(b),
            crate::plto_store::StoreError::Io(io) => SyncError::WritePluto(io),
        })?;

    Ok(result)
}

// --- UUID transplanting ---

/// Top-level orchestrator: match declarations between old and new programs by name,
/// copying UUIDs from old to new for matches. Collects add/remove/modify stats.
fn transplant_program(new: &mut Program, old: &Program) -> SyncResult {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();
    let mut unchanged: usize = 0;

    // Functions
    transplant_decls(
        &mut new.functions,
        &old.functions,
        |f| f.node.name.node.clone(),
        |new_f, old_f| {
            new_f.node.id = old_f.node.id;
            transplant_params(&mut new_f.node.params, &old_f.node.params);
        },
        Some(function_similarity),
        &mut added,
        &mut removed,
        &mut modified,
        &mut unchanged,
        "fn",
    );

    // Classes
    transplant_decls(
        &mut new.classes,
        &old.classes,
        |c| c.node.name.node.clone(),
        |new_c, old_c| {
            new_c.node.id = old_c.node.id;
            transplant_fields(&mut new_c.node.fields, &old_c.node.fields);
            transplant_methods(&mut new_c.node.methods, &old_c.node.methods);
        },
        None::<fn(&_, &_) -> f64>,
        &mut added,
        &mut removed,
        &mut modified,
        &mut unchanged,
        "class",
    );

    // Enums
    transplant_decls(
        &mut new.enums,
        &old.enums,
        |e| e.node.name.node.clone(),
        |new_e, old_e| {
            new_e.node.id = old_e.node.id;
            transplant_variants(&mut new_e.node.variants, &old_e.node.variants);
        },
        None::<fn(&_, &_) -> f64>,
        &mut added,
        &mut removed,
        &mut modified,
        &mut unchanged,
        "enum",
    );

    // Traits
    transplant_decls(
        &mut new.traits,
        &old.traits,
        |t| t.node.name.node.clone(),
        |new_t, old_t| {
            new_t.node.id = old_t.node.id;
            transplant_trait_methods(&mut new_t.node.methods, &old_t.node.methods);
        },
        None::<fn(&_, &_) -> f64>,
        &mut added,
        &mut removed,
        &mut modified,
        &mut unchanged,
        "trait",
    );

    // Errors
    transplant_decls(
        &mut new.errors,
        &old.errors,
        |e| e.node.name.node.clone(),
        |new_e, old_e| {
            new_e.node.id = old_e.node.id;
            transplant_fields(&mut new_e.node.fields, &old_e.node.fields);
        },
        None::<fn(&_, &_) -> f64>,
        &mut added,
        &mut removed,
        &mut modified,
        &mut unchanged,
        "error",
    );

    // App (at most one — match by name)
    match (&mut new.app, &old.app) {
        (Some(new_app), Some(old_app)) => {
            if new_app.node.name.node == old_app.node.name.node {
                new_app.node.id = old_app.node.id;
                transplant_fields(&mut new_app.node.inject_fields, &old_app.node.inject_fields);
                transplant_methods(&mut new_app.node.methods, &old_app.node.methods);
                // Count app as unchanged (or modified if content differs, but we simplify)
                unchanged += 1;
            } else {
                removed.push(format!("app {}", old_app.node.name.node));
                added.push(format!("app {}", new_app.node.name.node));
            }
        }
        (Some(new_app), None) => {
            added.push(format!("app {}", new_app.node.name.node));
        }
        (None, Some(old_app)) => {
            removed.push(format!("app {}", old_app.node.name.node));
        }
        (None, None) => {}
    }

    SyncResult {
        added,
        removed,
        modified,
        unchanged,
    }
}

/// Compute similarity score between two functions (0.0 = no match, 1.0 = perfect match).
/// Uses parameter types, return type, and parameter names.
fn function_similarity(f1: &Spanned<Function>, f2: &Spanned<Function>) -> f64 {
    let mut score = 0.0;
    let mut total_weight = 0.0;

    // Parameter count match (weight: 2.0)
    if f1.node.params.len() == f2.node.params.len() {
        score += 2.0;
    }
    total_weight += 2.0;

    // Parameter types (weight: 4.0)
    if f1.node.params.len() == f2.node.params.len() {
        let matching_types = f1.node.params.iter()
            .zip(f2.node.params.iter())
            .filter(|(p1, p2)| types_equal(&p1.ty, &p2.ty))
            .count();
        score += 4.0 * (matching_types as f64 / f1.node.params.len().max(1) as f64);
    }
    total_weight += 4.0;

    // Return type (weight: 2.0)
    let return_types_match = match (&f1.node.return_type, &f2.node.return_type) {
        (Some(r1), Some(r2)) => types_equal(r1, r2),
        (None, None) => true,
        _ => false,
    };
    if return_types_match {
        score += 2.0;
    }
    total_weight += 2.0;

    // Parameter names (weight: 1.0) - lower weight since names can change
    if f1.node.params.len() == f2.node.params.len() {
        let matching_names = f1.node.params.iter()
            .zip(f2.node.params.iter())
            .filter(|(p1, p2)| p1.name.node == p2.name.node)
            .count();
        score += 1.0 * (matching_names as f64 / f1.node.params.len().max(1) as f64);
    }
    total_weight += 1.0;

    score / total_weight
}

/// Generic helper to transplant UUIDs for a list of declarations.
/// Uses `name_fn` to extract names, `transplant_fn` to copy UUIDs and recurse.
/// Falls back to similarity matching when name matching fails.
#[allow(clippy::too_many_arguments)]
fn transplant_decls<T>(
    new_items: &mut [T],
    old_items: &[T],
    name_fn: impl Fn(&T) -> String,
    transplant_fn: impl Fn(&mut T, &T),
    similarity_fn: Option<impl Fn(&T, &T) -> f64>,
    added: &mut Vec<String>,
    removed: &mut Vec<String>,
    _modified: &mut Vec<String>,
    unchanged: &mut usize,
    kind: &str,
) {
    const SIMILARITY_THRESHOLD: f64 = 0.7; // Only match if similarity >= 70%

    // Track which old items have been matched (by index)
    let mut old_matched = vec![false; old_items.len()];
    // Track which new items were matched (by index)
    let mut new_matched = vec![false; new_items.len()];

    // First pass: match by name
    for (new_idx, new_item) in new_items.iter_mut().enumerate() {
        let name = name_fn(new_item);
        for (old_idx, old_item) in old_items.iter().enumerate() {
            if !old_matched[old_idx] && name_fn(old_item) == name {
                transplant_fn(new_item, old_item);
                *unchanged += 1;
                old_matched[old_idx] = true;
                new_matched[new_idx] = true;
                break;
            }
        }
    }

    // Second pass: similarity matching for unmatched items (if similarity_fn provided)
    if let Some(sim_fn) = similarity_fn {
        for (new_idx, new_item) in new_items.iter_mut().enumerate() {
            if new_matched[new_idx] {
                continue; // Already matched by name
            }

            // Find the best matching old item among unmatched ones
            let mut best_match: Option<(usize, f64)> = None;
            for (old_idx, old_item) in old_items.iter().enumerate() {
                if old_matched[old_idx] {
                    continue; // Already matched
                }

                let similarity = sim_fn(new_item, old_item);
                if similarity >= SIMILARITY_THRESHOLD {
                    if let Some((_, best_score)) = best_match {
                        if similarity > best_score {
                            best_match = Some((old_idx, similarity));
                        }
                    } else {
                        best_match = Some((old_idx, similarity));
                    }
                }
            }

            // If we found a good match, transplant the UUID
            if let Some((old_idx, _score)) = best_match {
                transplant_fn(new_item, &old_items[old_idx]);
                old_matched[old_idx] = true;
                new_matched[new_idx] = true;
                *unchanged += 1; // UUID preserved (rename detected)
            }
        }
    }

    // Mark unmatched new items as added
    for (new_idx, new_item) in new_items.iter().enumerate() {
        if !new_matched[new_idx] {
            let name = name_fn(new_item);
            added.push(format!("{kind} {name}"));
        }
    }

    // Detect removals (old items that weren't matched)
    for (old_idx, old_item) in old_items.iter().enumerate() {
        if !old_matched[old_idx] {
            let name = name_fn(old_item);
            removed.push(format!("{kind} {name}"));
        }
    }
}

/// Match params by name, copy UUIDs.
fn transplant_params(new_params: &mut [Param], old_params: &[Param]) {
    for new_p in new_params.iter_mut() {
        for old_p in old_params {
            if new_p.name.node == old_p.name.node {
                new_p.id = old_p.id;
                break;
            }
        }
    }
}

/// Match fields by name, copy UUIDs.
fn transplant_fields(new_fields: &mut [Field], old_fields: &[Field]) {
    for new_f in new_fields.iter_mut() {
        for old_f in old_fields {
            if new_f.name.node == old_f.name.node {
                new_f.id = old_f.id;
                break;
            }
        }
    }
}

/// Match methods by name, copy UUIDs and recurse into params.
fn transplant_methods(new_methods: &mut [Spanned<Function>], old_methods: &[Spanned<Function>]) {
    for new_m in new_methods.iter_mut() {
        for old_m in old_methods {
            if new_m.node.name.node == old_m.node.name.node {
                new_m.node.id = old_m.node.id;
                transplant_params(&mut new_m.node.params, &old_m.node.params);
                break;
            }
        }
    }
}

/// Match enum variants by name, copy UUIDs and recurse into fields.
fn transplant_variants(new_variants: &mut [EnumVariant], old_variants: &[EnumVariant]) {
    for new_v in new_variants.iter_mut() {
        for old_v in old_variants {
            if new_v.name.node == old_v.name.node {
                new_v.id = old_v.id;
                transplant_fields(&mut new_v.fields, &old_v.fields);
                break;
            }
        }
    }
}

/// Match trait methods by name, copy UUIDs and recurse into params.
fn transplant_trait_methods(new_methods: &mut [TraitMethod], old_methods: &[TraitMethod]) {
    for new_m in new_methods.iter_mut() {
        for old_m in old_methods {
            if new_m.name.node == old_m.name.node {
                new_m.id = old_m.id;
                transplant_params(&mut new_m.params, &old_m.params);
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_pt_file(source: &str) -> NamedTempFile {
        let mut f = NamedTempFile::with_suffix(".pt").unwrap();
        f.write_all(source.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn test_sync_creates_new_pluto() {
        let pt = make_pt_file("fn main() {\n    print(42)\n}\n");
        let pluto_path = std::env::temp_dir().join("test_sync_new.pluto");
        let _ = std::fs::remove_file(&pluto_path);

        let result = sync_pt_to_pluto(pt.path(), &pluto_path).unwrap();
        assert!(result.added.contains(&"fn main".to_string()));
        assert!(result.removed.is_empty());
        assert!(pluto_path.exists());

        // Verify it's a valid binary
        let data = std::fs::read(&pluto_path).unwrap();
        assert!(binary::is_binary_format(&data));
        let (program, _source, _derived) = binary::deserialize_program(&data).unwrap();
        assert_eq!(program.functions.len(), 1);
        assert_eq!(program.functions[0].node.name.node, "main");

        let _ = std::fs::remove_file(&pluto_path);
    }

    #[test]
    fn test_sync_preserves_uuids() {
        let source = "fn hello() {\n    print(1)\n}\n\nfn world() {\n    print(2)\n}\n";
        let pt = make_pt_file(source);
        let pluto_path = std::env::temp_dir().join("test_sync_preserve.pluto");
        let _ = std::fs::remove_file(&pluto_path);

        // First sync — creates fresh UUIDs
        sync_pt_to_pluto(pt.path(), &pluto_path).unwrap();
        let data = std::fs::read(&pluto_path).unwrap();
        let (first_program, _, _) = binary::deserialize_program(&data).unwrap();
        let hello_uuid = first_program.functions[0].node.id;
        let world_uuid = first_program.functions[1].node.id;

        // Second sync with same content — UUIDs should be preserved
        let pt2 = make_pt_file(source);
        sync_pt_to_pluto(pt2.path(), &pluto_path).unwrap();
        let data2 = std::fs::read(&pluto_path).unwrap();
        let (second_program, _, _) = binary::deserialize_program(&data2).unwrap();

        assert_eq!(second_program.functions[0].node.id, hello_uuid);
        assert_eq!(second_program.functions[1].node.id, world_uuid);

        let _ = std::fs::remove_file(&pluto_path);
    }

    // ===== Unit tests for transplant helpers =====

    use crate::parser::ast::{TypeExpr, Block};
    use crate::span::Span;
    use uuid::Uuid;

    fn make_param(name: &str, uuid: Uuid) -> Param {
        Param {
            id: uuid,
            name: Spanned::new(name.to_string(), Span::dummy()),
            ty: Spanned::new(TypeExpr::Named("void".to_string()), Span::dummy()),
            is_mut: false,
        }
    }

    fn make_field(name: &str, uuid: Uuid) -> Field {
        Field {
            id: uuid,
            name: Spanned::new(name.to_string(), Span::dummy()),
            ty: Spanned::new(TypeExpr::Named("void".to_string()), Span::dummy()),
            is_injected: false,
            is_ambient: false,
        }
    }

    fn make_function(name: &str, uuid: Uuid, param_uuids: Vec<(&str, Uuid)>) -> Spanned<Function> {
        Spanned::new(
            Function {
                id: uuid,
                name: Spanned::new(name.to_string(), Span::dummy()),
                type_params: vec![],
                type_param_bounds: std::collections::HashMap::new(),
                params: param_uuids
                    .into_iter()
                    .map(|(n, id)| make_param(n, id))
                    .collect(),
                return_type: None,
                contracts: vec![],
                body: Spanned::new(Block { stmts: vec![] }, Span::dummy()),
                is_pub: false,
                is_override: false,
                is_generator: false,
            },
            Span::dummy(),
        )
    }

    fn make_variant(name: &str, uuid: Uuid, field_uuids: Vec<(&str, Uuid)>) -> EnumVariant {
        EnumVariant {
            id: uuid,
            name: Spanned::new(name.to_string(), Span::dummy()),
            fields: field_uuids
                .into_iter()
                .map(|(n, id)| make_field(n, id))
                .collect(),
        }
    }

    fn make_trait_method(name: &str, uuid: Uuid, param_uuids: Vec<(&str, Uuid)>) -> TraitMethod {
        TraitMethod {
            id: uuid,
            name: Spanned::new(name.to_string(), Span::dummy()),
            params: param_uuids
                .into_iter()
                .map(|(n, id)| make_param(n, id))
                .collect(),
            return_type: None,
            contracts: vec![],
            body: None,
        }
    }

    #[test]
    fn test_transplant_params_matching_names() {
        let old_id = Uuid::new_v4();
        let mut new_params = vec![make_param("x", Uuid::new_v4())];
        let old_params = vec![make_param("x", old_id)];

        transplant_params(&mut new_params, &old_params);

        assert_eq!(new_params[0].id, old_id);
    }

    #[test]
    fn test_transplant_params_no_match() {
        let new_id = Uuid::new_v4();
        let mut new_params = vec![make_param("x", new_id)];
        let old_params = vec![make_param("y", Uuid::new_v4())];

        transplant_params(&mut new_params, &old_params);

        // UUID should remain unchanged
        assert_eq!(new_params[0].id, new_id);
    }

    #[test]
    fn test_transplant_params_multiple() {
        let old_x_id = Uuid::new_v4();
        let old_y_id = Uuid::new_v4();
        let mut new_params = vec![
            make_param("x", Uuid::new_v4()),
            make_param("y", Uuid::new_v4()),
        ];
        let old_params = vec![
            make_param("x", old_x_id),
            make_param("y", old_y_id),
        ];

        transplant_params(&mut new_params, &old_params);

        assert_eq!(new_params[0].id, old_x_id);
        assert_eq!(new_params[1].id, old_y_id);
    }

    #[test]
    fn test_transplant_fields_matching_names() {
        let old_id = Uuid::new_v4();
        let mut new_fields = vec![make_field("count", Uuid::new_v4())];
        let old_fields = vec![make_field("count", old_id)];

        transplant_fields(&mut new_fields, &old_fields);

        assert_eq!(new_fields[0].id, old_id);
    }

    #[test]
    fn test_transplant_fields_no_match() {
        let new_id = Uuid::new_v4();
        let mut new_fields = vec![make_field("count", new_id)];
        let old_fields = vec![make_field("total", Uuid::new_v4())];

        transplant_fields(&mut new_fields, &old_fields);

        assert_eq!(new_fields[0].id, new_id);
    }

    #[test]
    fn test_transplant_fields_multiple() {
        let old_x_id = Uuid::new_v4();
        let old_y_id = Uuid::new_v4();
        let mut new_fields = vec![
            make_field("x", Uuid::new_v4()),
            make_field("y", Uuid::new_v4()),
        ];
        let old_fields = vec![
            make_field("x", old_x_id),
            make_field("y", old_y_id),
        ];

        transplant_fields(&mut new_fields, &old_fields);

        assert_eq!(new_fields[0].id, old_x_id);
        assert_eq!(new_fields[1].id, old_y_id);
    }

    #[test]
    fn test_transplant_methods_matching_names() {
        let old_method_id = Uuid::new_v4();
        let old_param_id = Uuid::new_v4();
        let mut new_methods = vec![make_function("foo", Uuid::new_v4(), vec![("self", Uuid::new_v4())])];
        let old_methods = vec![make_function("foo", old_method_id, vec![("self", old_param_id)])];

        transplant_methods(&mut new_methods, &old_methods);

        assert_eq!(new_methods[0].node.id, old_method_id);
        assert_eq!(new_methods[0].node.params[0].id, old_param_id);
    }

    #[test]
    fn test_transplant_methods_no_match() {
        let new_id = Uuid::new_v4();
        let mut new_methods = vec![make_function("foo", new_id, vec![])];
        let old_methods = vec![make_function("bar", Uuid::new_v4(), vec![])];

        transplant_methods(&mut new_methods, &old_methods);

        assert_eq!(new_methods[0].node.id, new_id);
    }

    #[test]
    fn test_transplant_methods_multiple() {
        let old_foo_id = Uuid::new_v4();
        let old_bar_id = Uuid::new_v4();
        let mut new_methods = vec![
            make_function("foo", Uuid::new_v4(), vec![]),
            make_function("bar", Uuid::new_v4(), vec![]),
        ];
        let old_methods = vec![
            make_function("foo", old_foo_id, vec![]),
            make_function("bar", old_bar_id, vec![]),
        ];

        transplant_methods(&mut new_methods, &old_methods);

        assert_eq!(new_methods[0].node.id, old_foo_id);
        assert_eq!(new_methods[1].node.id, old_bar_id);
    }

    #[test]
    fn test_transplant_variants_matching_names() {
        let old_variant_id = Uuid::new_v4();
        let old_field_id = Uuid::new_v4();
        let mut new_variants = vec![make_variant("Some", Uuid::new_v4(), vec![("value", Uuid::new_v4())])];
        let old_variants = vec![make_variant("Some", old_variant_id, vec![("value", old_field_id)])];

        transplant_variants(&mut new_variants, &old_variants);

        assert_eq!(new_variants[0].id, old_variant_id);
        assert_eq!(new_variants[0].fields[0].id, old_field_id);
    }

    #[test]
    fn test_transplant_variants_no_match() {
        let new_id = Uuid::new_v4();
        let mut new_variants = vec![make_variant("Some", new_id, vec![])];
        let old_variants = vec![make_variant("None", Uuid::new_v4(), vec![])];

        transplant_variants(&mut new_variants, &old_variants);

        assert_eq!(new_variants[0].id, new_id);
    }

    #[test]
    fn test_transplant_variants_multiple() {
        let old_some_id = Uuid::new_v4();
        let old_none_id = Uuid::new_v4();
        let mut new_variants = vec![
            make_variant("Some", Uuid::new_v4(), vec![]),
            make_variant("None", Uuid::new_v4(), vec![]),
        ];
        let old_variants = vec![
            make_variant("Some", old_some_id, vec![]),
            make_variant("None", old_none_id, vec![]),
        ];

        transplant_variants(&mut new_variants, &old_variants);

        assert_eq!(new_variants[0].id, old_some_id);
        assert_eq!(new_variants[1].id, old_none_id);
    }

    #[test]
    fn test_transplant_trait_methods_matching_names() {
        let old_method_id = Uuid::new_v4();
        let old_param_id = Uuid::new_v4();
        let mut new_methods = vec![make_trait_method("print", Uuid::new_v4(), vec![("self", Uuid::new_v4())])];
        let old_methods = vec![make_trait_method("print", old_method_id, vec![("self", old_param_id)])];

        transplant_trait_methods(&mut new_methods, &old_methods);

        assert_eq!(new_methods[0].id, old_method_id);
        assert_eq!(new_methods[0].params[0].id, old_param_id);
    }

    #[test]
    fn test_transplant_trait_methods_no_match() {
        let new_id = Uuid::new_v4();
        let mut new_methods = vec![make_trait_method("print", new_id, vec![])];
        let old_methods = vec![make_trait_method("clone", Uuid::new_v4(), vec![])];

        transplant_trait_methods(&mut new_methods, &old_methods);

        assert_eq!(new_methods[0].id, new_id);
    }

    #[test]
    fn test_transplant_trait_methods_multiple() {
        let old_print_id = Uuid::new_v4();
        let old_clone_id = Uuid::new_v4();
        let mut new_methods = vec![
            make_trait_method("print", Uuid::new_v4(), vec![]),
            make_trait_method("clone", Uuid::new_v4(), vec![]),
        ];
        let old_methods = vec![
            make_trait_method("print", old_print_id, vec![]),
            make_trait_method("clone", old_clone_id, vec![]),
        ];

        transplant_trait_methods(&mut new_methods, &old_methods);

        assert_eq!(new_methods[0].id, old_print_id);
        assert_eq!(new_methods[1].id, old_clone_id);
    }
}
