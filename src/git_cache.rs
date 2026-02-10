use std::path::{Path, PathBuf};

use crate::diagnostics::CompileError;

/// Which git ref to check out after cloning.
#[derive(Debug, Clone)]
pub enum GitRef {
    DefaultBranch,
    Rev(String),
    Tag(String),
    Branch(String),
}

/// Compute the global cache directory for a git URL.
///
/// Layout: `<cache_root>/git/<hash>/`
/// where `<hash>` is a 16-char hex djb2 hash of the normalized URL.
/// `<cache_root>` defaults to `~/.pluto/cache` but can be overridden
/// with the `PLUTO_CACHE_DIR` env var.
pub fn cache_dir_for_url(url: &str) -> PathBuf {
    let root = cache_root();
    let hash = djb2_hex(&normalize_url(url));
    root.join("git").join(hash)
}

/// Clone (if not already cached) and checkout the requested ref.
/// Returns the path to the cached repo directory.
pub fn ensure_cached(
    url: &str,
    git_ref: &GitRef,
    manifest_path: &Path,
) -> Result<PathBuf, CompileError> {
    let dir = cache_dir_for_url(url);

    if !dir.exists() {
        // Clone into cache
        std::fs::create_dir_all(dir.parent().unwrap()).map_err(|e| {
            CompileError::manifest(
                format!("failed to create git cache directory: {e}"),
                manifest_path.to_path_buf(),
            )
        })?;
        run_git(
            None,
            &["clone", url, &dir.to_string_lossy()],
            url,
            manifest_path,
            "clone",
        )?;
    }

    checkout_ref(&dir, url, git_ref, manifest_path)?;

    Ok(dir)
}

/// Fetch latest from remote and reset to the requested ref.
/// Used by `plutoc update`.
pub fn fetch_and_update(
    url: &str,
    git_ref: &GitRef,
    manifest_path: &Path,
) -> Result<PathBuf, CompileError> {
    let dir = cache_dir_for_url(url);

    if !dir.exists() {
        // Not cached yet — just do a fresh clone
        return ensure_cached(url, git_ref, manifest_path);
    }

    // Fetch latest
    run_git(
        Some(&dir),
        &["fetch", "--all"],
        url,
        manifest_path,
        "fetch",
    )?;

    checkout_ref(&dir, url, git_ref, manifest_path)?;

    Ok(dir)
}

// ---- Internal helpers ----

fn checkout_ref(
    dir: &Path,
    url: &str,
    git_ref: &GitRef,
    manifest_path: &Path,
) -> Result<(), CompileError> {
    match git_ref {
        GitRef::DefaultBranch => {
            // Reset to origin's default branch HEAD
            run_git(
                Some(dir),
                &["checkout", "HEAD"],
                url,
                manifest_path,
                "checkout",
            )?;
        }
        GitRef::Rev(rev) => {
            run_git(
                Some(dir),
                &["checkout", rev],
                url,
                manifest_path,
                "checkout",
            )?;
        }
        GitRef::Tag(tag) => {
            run_git(
                Some(dir),
                &["checkout", &format!("tags/{tag}")],
                url,
                manifest_path,
                "checkout",
            )?;
        }
        GitRef::Branch(branch) => {
            // Try to checkout the branch; if it's a remote-tracking branch, create a local copy
            let result = run_git(
                Some(dir),
                &["checkout", branch],
                url,
                manifest_path,
                "checkout",
            );
            if result.is_err() {
                // Try the remote-tracking branch
                run_git(
                    Some(dir),
                    &["checkout", "-b", branch, &format!("origin/{branch}")],
                    url,
                    manifest_path,
                    "checkout",
                )?;
            }
        }
    }
    Ok(())
}

/// Run a git command, returning a descriptive error on failure.
fn run_git(
    dir: Option<&Path>,
    args: &[&str],
    url: &str,
    manifest_path: &Path,
    operation: &str,
) -> Result<(), CompileError> {
    let mut cmd = std::process::Command::new("git");
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    cmd.args(args);
    // Isolate from any parent repo and suppress interactive prompts
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env_remove("GIT_DIR");
    cmd.env_remove("GIT_WORK_TREE");
    cmd.env_remove("GIT_INDEX_FILE");
    cmd.env_remove("GIT_CEILING_DIRECTORIES");

    let output = cmd.output().map_err(|e| {
        CompileError::manifest(
            format!("git is required for git dependencies but was not found in PATH: {e}"),
            manifest_path.to_path_buf(),
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CompileError::manifest(
            format!("git {operation} failed for '{url}': {stderr}"),
            manifest_path.to_path_buf(),
        ));
    }

    Ok(())
}

/// Normalize a URL for hashing: lowercase, strip trailing slashes.
fn normalize_url(url: &str) -> String {
    url.to_lowercase().trim_end_matches('/').to_string()
}

/// djb2 hash → 16-char hex string.
fn djb2_hex(s: &str) -> String {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    format!("{:016x}", hash)
}

/// The root cache directory. Defaults to `~/.pluto/cache`, overridden by `PLUTO_CACHE_DIR`.
fn cache_root() -> PathBuf {
    if let Ok(dir) = std::env::var("PLUTO_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".pluto").join("cache")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_url_strips_trailing_slash() {
        assert_eq!(
            normalize_url("https://github.com/user/repo.git/"),
            "https://github.com/user/repo.git"
        );
    }

    #[test]
    fn normalize_url_lowercases() {
        assert_eq!(
            normalize_url("HTTPS://GitHub.com/User/Repo.git"),
            "https://github.com/user/repo.git"
        );
    }

    #[test]
    fn djb2_hex_deterministic() {
        let h1 = djb2_hex("hello");
        let h2 = djb2_hex("hello");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);
    }

    #[test]
    fn djb2_hex_different_inputs() {
        assert_ne!(djb2_hex("hello"), djb2_hex("world"));
    }

    #[test]
    fn cache_dir_for_url_deterministic() {
        let d1 = cache_dir_for_url("https://github.com/user/repo.git");
        let d2 = cache_dir_for_url("https://github.com/user/repo.git");
        assert_eq!(d1, d2);
    }

    #[test]
    fn cache_dir_for_url_normalized() {
        let d1 = cache_dir_for_url("https://github.com/user/repo.git");
        let d2 = cache_dir_for_url("https://github.com/user/repo.git/");
        assert_eq!(d1, d2);
    }
}
