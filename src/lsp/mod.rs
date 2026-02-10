mod analysis;
mod diagnostics;
mod goto_def;
mod hover;
mod line_index;
mod server;
mod symbols;

pub use server::run_lsp_server;

use std::path::{Path, PathBuf};

use lsp_types::Uri;

/// Convert a file path to an LSP Uri.
fn path_to_uri(path: &Path) -> Uri {
    let abs = if path.is_absolute() {
        path.to_string_lossy().to_string()
    } else {
        path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string()
    };
    // file:///absolute/path
    let s = format!("file://{}", abs);
    s.parse().unwrap_or_else(|_| "file:///unknown".parse().unwrap())
}

/// Convert an LSP Uri to a file path. Returns None if not a file:// URI.
fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let s = uri.as_str();
    if let Some(rest) = s.strip_prefix("file://") {
        Some(PathBuf::from(rest))
    } else {
        None
    }
}
