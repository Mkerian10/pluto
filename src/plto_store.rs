//! Shared persistence API for canonical .pluto binary files.
//!
//! All writes to .pluto files should go through this module to ensure:
//! - Canonical AST shape (parse_for_editing + xref)
//! - Fresh derived metadata
//! - Atomic writes (temp file + rename on Unix)

use crate::binary::{self, BinaryError};
use crate::derived::DerivedInfo;
use crate::parser::ast::Program;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("binary serialization error: {0}")]
    Binary(#[from] BinaryError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Write canonical .pluto file with fresh derived data.
pub fn write_canonical(
    path: &Path,
    program: &Program,
    source: &str,
    mut derived: DerivedInfo,
) -> Result<(), StoreError> {
    // Ensure source hash is fresh
    derived.source_hash = DerivedInfo::compute_source_hash(source);

    let bytes = binary::serialize_program(program, source, &derived)?;

    // Atomic write on Unix: temp file + rename
    #[cfg(unix)]
    {
        let temp_path = path.with_extension("pluto.tmp");
        std::fs::write(&temp_path, &bytes)?;
        std::fs::rename(&temp_path, path)?;
    }

    // Non-atomic fallback for Windows
    #[cfg(not(unix))]
    {
        std::fs::write(path, &bytes)?;
    }

    Ok(())
}

/// Write canonical .pluto file with stale/empty derived data (for sync).
pub fn write_canonical_stale(
    path: &Path,
    program: &Program,
    source: &str,
) -> Result<(), StoreError> {
    let derived = DerivedInfo::default(); // source_hash = "" (stale)
    let bytes = binary::serialize_program(program, source, &derived)?;

    #[cfg(unix)]
    {
        let temp_path = path.with_extension("pluto.tmp");
        std::fs::write(&temp_path, &bytes)?;
        std::fs::rename(&temp_path, path)?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(path, &bytes)?;
    }

    Ok(())
}

/// Read canonical .pluto file (supports v2 and v3).
pub fn read_canonical(path: &Path) -> Result<(Program, String, DerivedInfo), StoreError> {
    let bytes = std::fs::read(path)?;
    let (program, source, derived) = binary::deserialize_program(&bytes)?;
    Ok((program, source, derived))
}

/// Check if derived data is fresh for a .pluto file.
pub fn is_derived_fresh(path: &Path) -> Result<bool, StoreError> {
    let (_, source, derived) = read_canonical(path)?;
    Ok(!derived.is_stale(&source))
}
