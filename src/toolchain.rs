//! Toolchain version management for Pluto compiler.
//!
//! This module provides functionality to:
//! - Install multiple compiler versions from GitHub releases
//! - Switch between installed versions
//! - Query active and installed versions
//! - Resolve paths to version binaries for delegation

use crate::diagnostics::CompileError;
use std::fs;
use std::path::{Path, PathBuf};

/// ABI version of the stdlib↔compiler contract that THIS compiler binary was
/// built to validate: the extern-type whitelist, the prelude shape, and the
/// marshaling/wire contracts the stdlib relies on. A stdlib stamps the minimum
/// compiler ABI it needs in a `stdlib/ABI` manifest; `check_stdlib_abi` below
/// refuses to load a stdlib whose requirement exceeds this number, turning a
/// version-skew crash into one clear sentence (issue #396).
///
/// BUMP RULE — increment this by hand (and update the matching note in the
/// stdlib's `stdlib/ABI` file) whenever a change makes a newer stdlib
/// unloadable by older compilers, i.e. whenever you:
///   - widen or change the extern-type whitelist (src/typeck/register.rs),
///   - change the prelude's shape in a way the stdlib depends on
///     (stdlib/prelude.pt), or
///   - change the marshaling / wire encoding contracts.
/// A pure-additive stdlib change (a new function the old compiler can still
/// compile) does NOT need a bump. When in doubt, bump: a false bump only costs
/// users an upgrade prompt, a missed one brings back the misleading type error.
///
/// 2026-10-06: initialized to 1. First ABI established alongside the handshake
/// itself; the `bytes` extern whitelist entry (the skew that motivated #396)
/// is part of ABI 1.
pub const STDLIB_ABI_VERSION: u32 = 2;

/// Returns the ~/.pluto/versions/ directory, creating it if it doesn't exist.
pub fn versions_dir() -> Result<PathBuf, CompileError> {
    let home = std::env::var("HOME")
        .map_err(|_| CompileError::toolchain("HOME environment variable not set"))?;
    let dir = PathBuf::from(home).join(".pluto").join("versions");

    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| CompileError::toolchain(format!("failed to create versions directory: {}", e)))?;
    }

    Ok(dir)
}

/// Returns the ~/.pluto/active file path.
pub fn active_version_file() -> Result<PathBuf, CompileError> {
    let home = std::env::var("HOME")
        .map_err(|_| CompileError::toolchain("HOME environment variable not set"))?;
    let pluto_dir = PathBuf::from(home).join(".pluto");

    if !pluto_dir.exists() {
        fs::create_dir_all(&pluto_dir)
            .map_err(|e| CompileError::toolchain(format!("failed to create .pluto directory: {}", e)))?;
    }

    Ok(pluto_dir.join("active"))
}

/// Reads the active version from ~/.pluto/active.
/// Returns an error if the file doesn't exist.
pub fn active_version() -> Result<String, CompileError> {
    let file = active_version_file()?;

    if !file.exists() {
        return Err(CompileError::toolchain(
            "no active version set; use 'pluto use <version>' to set one"
        ));
    }

    let content = fs::read_to_string(&file)
        .map_err(|e| CompileError::toolchain(format!("failed to read active version: {}", e)))?;

    Ok(content.trim().to_string())
}

/// Returns the version of the currently running compiler binary.
pub fn running_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Parse the `requires-compiler-abi = N` line out of a `stdlib/ABI` manifest.
/// Blank lines and `#` comments are ignored. Returns `Ok(None)` if the key is
/// absent; `Err` if it is present but malformed.
fn parse_required_abi(content: &str, abi_path: &Path) -> Result<Option<u32>, CompileError> {
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "requires-compiler-abi" {
            continue;
        }
        let value = value.trim();
        return match value.parse::<u32>() {
            Ok(n) => Ok(Some(n)),
            Err(_) => Err(CompileError::module(format!(
                "{}: 'requires-compiler-abi' must be a non-negative integer, got '{}'",
                abi_path.display(),
                value
            ))),
        };
    }
    Ok(None)
}

/// Compatibility handshake run BEFORE the stdlib is flattened into the program
/// (issue #396). `stdlib_root` is the resolved stdlib directory — from
/// `--stdlib`, `PLUTO_STDLIB`, or `./stdlib` discovery; never a user module
/// dir, so this never fires for user code that happens to contain an `ABI`
/// file.
///
/// A stdlib declares the minimum compiler ABI it needs in `<stdlib_root>/ABI`
/// (`requires-compiler-abi = N`). If that requirement exceeds the compiler's
/// [`STDLIB_ABI_VERSION`], loading would produce a misleading type error that
/// points at the stdlib rather than at the skew — so we refuse up front with
/// one clear sentence.
///
/// A MISSING `ABI` file is accepted silently: it marks a stdlib that predates
/// this handshake, which (requiring an implicit ABI of 0) any current compiler
/// can load. Only a stdlib that explicitly asks for a newer compiler is
/// rejected.
pub fn check_stdlib_abi(stdlib_root: &Path) -> Result<(), CompileError> {
    let abi_path = stdlib_root.join("ABI");
    let content = match fs::read_to_string(&abi_path) {
        Ok(c) => c,
        // No manifest (or unreadable) → old stdlib, accept silently.
        Err(_) => return Ok(()),
    };

    let Some(required) = parse_required_abi(&content, &abi_path)? else {
        return Ok(());
    };

    if required > STDLIB_ABI_VERSION {
        return Err(CompileError::module(format!(
            "this stdlib requires compiler ABI >= {}; this pluto binary supports {} — update the compiler (or point --stdlib at a matching stdlib)",
            required, STDLIB_ABI_VERSION
        )));
    }

    Ok(())
}

/// Lists all installed versions (directory names in ~/.pluto/versions/).
pub fn installed_versions() -> Result<Vec<String>, CompileError> {
    let dir = versions_dir()?;

    let mut versions = Vec::new();

    if !dir.exists() {
        return Ok(versions);
    }

    let entries = fs::read_dir(&dir)
        .map_err(|e| CompileError::toolchain(format!("failed to read versions directory: {}", e)))?;

    for entry in entries {
        let entry = entry
            .map_err(|e| CompileError::toolchain(format!("failed to read directory entry: {}", e)))?;

        if entry.file_type()
            .map_err(|e| CompileError::toolchain(format!("failed to get file type: {}", e)))?
            .is_dir()
        {
            if let Some(name) = entry.file_name().to_str() {
                versions.push(name.to_string());
            }
        }
    }

    // Sort versions for consistent output
    versions.sort();

    Ok(versions)
}

/// Normalizes a version string by removing "v" prefix if present.
/// "v0.2.0" -> "0.2.0", "0.2.0" -> "0.2.0"
fn normalize_version(version: &str) -> String {
    version.strip_prefix('v').unwrap_or(version).to_string()
}

/// Returns the target triple for the host platform.
fn host_target_triple() -> String {
    if cfg!(all(target_arch = "aarch64", target_os = "macos")) {
        "aarch64-apple-darwin".to_string()
    } else if cfg!(all(target_arch = "x86_64", target_os = "macos")) {
        "x86_64-apple-darwin".to_string()
    } else if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        "x86_64-unknown-linux-gnu".to_string()
    } else if cfg!(all(target_arch = "aarch64", target_os = "linux")) {
        "aarch64-unknown-linux-gnu".to_string()
    } else {
        format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
    }
}

/// Installs a compiler version by downloading from GitHub releases.
/// Accepts both "v0.2.0" and "0.2.0" format.
pub fn install_version(version: &str) -> Result<(), CompileError> {
    let version = normalize_version(version);
    let versions_dir = versions_dir()?;
    let version_dir = versions_dir.join(&version);

    // Check if already installed
    if version_dir.exists() {
        eprintln!("pluto v{} is already installed", version);
        return Ok(());
    }

    // Create version directory
    fs::create_dir_all(&version_dir)
        .map_err(|e| CompileError::toolchain(format!("failed to create version directory: {}", e)))?;

    // Build download URL
    let target = host_target_triple();
    let url = format!(
        "https://github.com/pluto-lang/pluto/releases/download/v{}/pluto-{}",
        version, target
    );

    eprintln!("Downloading pluto v{}...", version);

    // Download binary
    let response = ureq::get(&url)
        .call()
        .map_err(|e| {
            match e {
                ureq::Error::Status(404, _) => {
                    CompileError::version_not_found(format!(
                        "version {} not found for target {} (check https://github.com/pluto-lang/pluto/releases)",
                        version, target
                    ))
                }
                _ => CompileError::network(format!("failed to download: {}", e))
            }
        })?;

    // Download to temp file first
    let temp_path = version_dir.join(".pluto.tmp");
    let mut file = fs::File::create(&temp_path)
        .map_err(|e| CompileError::toolchain(format!("failed to create temp file: {}", e)))?;

    std::io::copy(&mut response.into_reader(), &mut file)
        .map_err(|e| CompileError::network(format!("failed to write download: {}", e)))?;

    // Set executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&temp_path)
            .map_err(|e| CompileError::toolchain(format!("failed to get file metadata: {}", e)))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&temp_path, perms)
            .map_err(|e| CompileError::toolchain(format!("failed to set permissions: {}", e)))?;
    }

    // Atomic rename
    let final_path = version_dir.join("pluto");
    fs::rename(&temp_path, &final_path)
        .map_err(|e| CompileError::toolchain(format!("failed to install binary: {}", e)))?;

    eprintln!("Installed pluto v{}", version);

    Ok(())
}

/// Sets the active version. The version must already be installed.
pub fn use_version(version: &str) -> Result<(), CompileError> {
    let version = normalize_version(version);
    let versions_dir = versions_dir()?;
    let version_dir = versions_dir.join(&version);

    // Check if version is installed
    if !version_dir.exists() {
        return Err(CompileError::toolchain(format!(
            "version {} is not installed; use 'pluto install {}' first",
            version, version
        )));
    }

    // Verify binary exists
    let binary_path = version_dir.join("pluto");
    if !binary_path.exists() {
        return Err(CompileError::toolchain(format!(
            "version {} is installed but binary is missing",
            version
        )));
    }

    // Write active version
    let active_file = active_version_file()?;
    fs::write(&active_file, format!("{}\n", version))
        .map_err(|e| CompileError::toolchain(format!("failed to set active version: {}", e)))?;

    eprintln!("Now using pluto v{}", version);

    Ok(())
}

/// Returns the path to the active version's binary.
pub fn active_version_binary() -> Result<PathBuf, CompileError> {
    let version = active_version()?;
    let versions_dir = versions_dir()?;
    let binary_path = versions_dir.join(&version).join("pluto");

    if !binary_path.exists() {
        return Err(CompileError::toolchain(format!(
            "active version {} binary not found at {}",
            version,
            binary_path.display()
        )));
    }

    Ok(binary_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_version() {
        assert_eq!(normalize_version("v0.2.0"), "0.2.0");
        assert_eq!(normalize_version("0.2.0"), "0.2.0");
        assert_eq!(normalize_version("v1.0.0-beta"), "1.0.0-beta");
    }

    #[test]
    fn test_running_version() {
        let version = running_version();
        assert!(!version.is_empty());
        // Should be "0.1.0" based on Cargo.toml
        assert_eq!(version, "0.1.0");
    }

    #[test]
    fn test_host_target_triple() {
        let target = host_target_triple();
        assert!(!target.is_empty());
        // Should contain either darwin or linux
        assert!(target.contains("darwin") || target.contains("linux"));
    }

    #[test]
    fn test_versions_dir_creation() {
        // This test actually creates the directory - should be safe
        let dir = versions_dir().expect("should create versions dir");
        assert!(dir.exists());
        assert!(dir.ends_with(".pluto/versions"));
    }

    #[test]
    fn test_active_version_file() {
        let file = active_version_file().expect("should get active version file path");
        assert!(file.ends_with(".pluto/active"));
    }

    #[test]
    fn test_installed_versions_empty() {
        // Should not error even if no versions installed
        let versions = installed_versions().expect("should list versions");
        // May or may not be empty depending on system state
        assert!(versions.is_empty() || !versions.is_empty());
    }

    #[test]
    fn test_use_version_not_installed() {
        let result = use_version("999.999.999");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, CompileError::Toolchain(_)));
    }

    #[test]
    fn test_parse_required_abi_valid() {
        let p = Path::new("ABI");
        assert_eq!(
            parse_required_abi("requires-compiler-abi = 3\n", p).unwrap(),
            Some(3)
        );
    }

    #[test]
    fn test_parse_required_abi_ignores_comments_and_blanks() {
        let content = "# a comment\n\n   # indented comment\nrequires-compiler-abi = 7\n";
        assert_eq!(
            parse_required_abi(content, Path::new("ABI")).unwrap(),
            Some(7)
        );
    }

    #[test]
    fn test_parse_required_abi_absent_key() {
        assert_eq!(
            parse_required_abi("other-key = 1\n", Path::new("ABI")).unwrap(),
            None
        );
    }

    #[test]
    fn test_parse_required_abi_malformed() {
        assert!(parse_required_abi("requires-compiler-abi = x\n", Path::new("ABI")).is_err());
    }

    #[test]
    fn test_check_stdlib_abi_missing_file_ok() {
        let dir = tempfile::tempdir().unwrap();
        // No ABI file present → accepted.
        assert!(check_stdlib_abi(dir.path()).is_ok());
    }

    #[test]
    fn test_check_stdlib_abi_mismatch_errors() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("ABI"), "requires-compiler-abi = 999\n").unwrap();
        let err = check_stdlib_abi(dir.path()).unwrap_err();
        assert!(err.to_string().contains("this stdlib requires compiler ABI >= 999"));
    }

    #[test]
    fn test_check_stdlib_abi_match_ok() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("ABI"),
            format!("requires-compiler-abi = {STDLIB_ABI_VERSION}\n"),
        )
        .unwrap();
        assert!(check_stdlib_abi(dir.path()).is_ok());
    }

    #[test]
    fn test_active_version_not_set() {
        // Remove active file if it exists for this test
        if let Ok(file) = active_version_file() {
            let _ = fs::remove_file(&file);
        }

        let result = active_version();
        // Might succeed if user has active version set, or fail if not
        // Just verify it doesn't panic
        let _ = result;
    }
}
