use std::path::Path;

use crate::binary;
use crate::parser::ast::{
    EnumVariant, Field, Function, Param, Program, TraitMethod,
};
use crate::span::Spanned;
use crate::xref;

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

    // 5. Serialize to .pluto binary with empty derived data
    let derived = crate::derived::DerivedInfo::default();
    let bytes =
        binary::serialize_program(&new_program, &pt_source, &derived).map_err(SyncError::Serialize)?;
    std::fs::write(pluto_path, &bytes).map_err(SyncError::WritePluto)?;

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

/// Generic helper to transplant UUIDs for a list of declarations.
/// Uses `name_fn` to extract names, `transplant_fn` to copy UUIDs and recurse.
#[allow(clippy::too_many_arguments)]
fn transplant_decls<T>(
    new_items: &mut [T],
    old_items: &[T],
    name_fn: impl Fn(&T) -> String,
    transplant_fn: impl Fn(&mut T, &T),
    added: &mut Vec<String>,
    removed: &mut Vec<String>,
    _modified: &mut Vec<String>,
    unchanged: &mut usize,
    kind: &str,
) {
    // Build a set of new names for removal detection
    let new_names: std::collections::HashSet<String> =
        new_items.iter().map(&name_fn).collect();
    let old_names: std::collections::HashSet<String> =
        old_items.iter().map(&name_fn).collect();

    // Transplant matching items
    for new_item in new_items.iter_mut() {
        let name = name_fn(new_item);
        let mut found = false;
        for old_item in old_items {
            if name_fn(old_item) == name {
                transplant_fn(new_item, old_item);
                *unchanged += 1;
                found = true;
                break;
            }
        }
        if !found {
            added.push(format!("{kind} {name}"));
        }
    }

    // Detect removals
    for old_name in &old_names {
        if !new_names.contains(old_name) {
            removed.push(format!("{kind} {old_name}"));
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
}
