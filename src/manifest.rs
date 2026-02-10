use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::diagnostics::CompileError;
use crate::git_cache::{self, GitRef};
use crate::lexer;

/// Per-package dependency scope: maps dep_name -> resolved absolute path.
pub type DependencyScope = BTreeMap<String, PathBuf>;

/// A node in the package graph.
pub struct PackageNode {
    pub name: String,
    pub root_dir: PathBuf,
    pub dependencies: DependencyScope,
}

/// The full resolved package graph passed to module resolution.
pub struct PackageGraph {
    /// Canonical root_dir of the entry project. None if no manifest found.
    pub root_dir: Option<PathBuf>,
    /// All packages in the graph (including root if manifest exists), keyed by canonical root_dir.
    pub packages: BTreeMap<PathBuf, PackageNode>,
}

static EMPTY_SCOPE: std::sync::LazyLock<DependencyScope> = std::sync::LazyLock::new(BTreeMap::new);

impl PackageGraph {
    pub fn empty() -> Self {
        Self { root_dir: None, packages: BTreeMap::new() }
    }

    /// Returns root deps, or empty scope if no manifest.
    pub fn root_deps(&self) -> &DependencyScope {
        self.root_dir.as_ref()
            .and_then(|d| self.packages.get(d))
            .map(|n| &n.dependencies)
            .unwrap_or(&EMPTY_SCOPE)
    }

    /// Returns deps for a given canonical package dir, or empty scope if unknown.
    /// Callers MUST pass already-canonicalized paths.
    pub fn deps_for(&self, canonical_dir: &Path) -> &DependencyScope {
        self.packages.get(canonical_dir)
            .map(|n| &n.dependencies)
            .unwrap_or(&EMPTY_SCOPE)
    }
}

// ---- TOML deserialization types ----

#[derive(Deserialize)]
struct TomlManifest {
    package: Option<TomlPackage>,
    #[serde(default)]
    dependencies: BTreeMap<String, TomlDep>,
}

#[derive(Deserialize)]
struct TomlPackage {
    name: Option<String>,
    #[serde(default = "default_version")]
    #[allow(dead_code)]
    version: String,
}

fn default_version() -> String {
    "0.1.0".to_string()
}

#[derive(Deserialize)]
struct TomlDep {
    path: Option<String>,
    git: Option<String>,
    rev: Option<String>,
    tag: Option<String>,
    branch: Option<String>,
}

// ---- Dependency spec validation ----

/// Validate a single dependency spec from pluto.toml.
/// Returns either a path string or (git_url, GitRef) pair.
fn validate_dep_spec(
    dep_name: &str,
    dep: &TomlDep,
    manifest_path: &Path,
) -> Result<DepKind, CompileError> {
    let has_path = dep.path.is_some();
    let has_git = dep.git.is_some();
    let has_rev = dep.rev.is_some();
    let has_tag = dep.tag.is_some();
    let has_branch = dep.branch.is_some();

    // Must have exactly one of path or git
    if has_path && has_git {
        return Err(CompileError::manifest(
            format!("dependency '{}': specify either 'path' or 'git', not both", dep_name),
            manifest_path.to_path_buf(),
        ));
    }
    if !has_path && !has_git {
        return Err(CompileError::manifest(
            format!("dependency '{}': must specify 'path' or 'git'", dep_name),
            manifest_path.to_path_buf(),
        ));
    }

    // rev/tag/branch only valid with git
    if has_path && (has_rev || has_tag || has_branch) {
        return Err(CompileError::manifest(
            format!("dependency '{}': 'rev'/'tag'/'branch' are only valid with git dependencies", dep_name),
            manifest_path.to_path_buf(),
        ));
    }

    // At most one of rev/tag/branch
    let ref_count = [has_rev, has_tag, has_branch].iter().filter(|&&x| x).count();
    if ref_count > 1 {
        return Err(CompileError::manifest(
            format!("dependency '{}': specify at most one of 'rev', 'tag', 'branch'", dep_name),
            manifest_path.to_path_buf(),
        ));
    }

    if has_path {
        Ok(DepKind::Path(dep.path.clone().unwrap()))
    } else {
        let url = dep.git.clone().unwrap();
        let git_ref = if let Some(rev) = &dep.rev {
            GitRef::Rev(rev.clone())
        } else if let Some(tag) = &dep.tag {
            GitRef::Tag(tag.clone())
        } else if let Some(branch) = &dep.branch {
            GitRef::Branch(branch.clone())
        } else {
            GitRef::DefaultBranch
        };
        Ok(DepKind::Git(url, git_ref))
    }
}

enum DepKind {
    Path(String),
    Git(String, GitRef),
}

// ---- Manifest discovery ----

/// Walk from start_dir up to .git or FS root, looking for pluto.toml.
/// Only used for the entry project.
fn find_manifest_walk(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let candidate = dir.join("pluto.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        // Stop if .git exists (file OR dir — handles worktrees/submodules)
        let git_path = dir.join(".git");
        if git_path.exists() {
            return None;
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Check <dir>/pluto.toml directly. Used for dependency nodes (no parent walk).
fn read_manifest_direct(dir: &Path) -> Option<PathBuf> {
    let candidate = dir.join("pluto.toml");
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

// ---- Parsing & validation ----

fn parse_manifest(manifest_path: &Path) -> Result<(TomlManifest, PathBuf), CompileError> {
    let content = std::fs::read_to_string(manifest_path).map_err(|e| {
        CompileError::manifest(
            format!("pluto.toml: could not read file: {e}"),
            manifest_path.to_path_buf(),
        )
    })?;

    let manifest: TomlManifest = toml::from_str(&content).map_err(|e| {
        CompileError::manifest(
            format!("pluto.toml: invalid syntax: {e}"),
            manifest_path.to_path_buf(),
        )
    })?;

    // Validate [package] section
    let package = manifest.package.as_ref().ok_or_else(|| {
        CompileError::manifest(
            "pluto.toml: missing [package] section",
            manifest_path.to_path_buf(),
        )
    })?;

    let name = package.name.as_ref().ok_or_else(|| {
        CompileError::manifest(
            "pluto.toml: missing 'name' in [package]",
            manifest_path.to_path_buf(),
        )
    })?;

    if name.trim().is_empty() {
        return Err(CompileError::manifest(
            "pluto.toml: package name must not be empty",
            manifest_path.to_path_buf(),
        ));
    }

    let manifest_dir = manifest_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    Ok((manifest, manifest_dir))
}

fn validate_dep_name(name: &str, manifest_path: &Path) -> Result<(), CompileError> {
    // Must match [a-zA-Z_][a-zA-Z0-9_]*
    let valid = !name.is_empty()
        && {
            let first = name.chars().next().unwrap();
            first.is_ascii_alphabetic() || first == '_'
        }
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');

    if !valid {
        return Err(CompileError::manifest(
            format!("pluto.toml: dependency name '{}' is not a valid identifier", name),
            manifest_path.to_path_buf(),
        ));
    }

    if name == "std" {
        return Err(CompileError::manifest(
            "pluto.toml: dependency name 'std' is reserved",
            manifest_path.to_path_buf(),
        ));
    }

    if lexer::is_keyword(name) {
        return Err(CompileError::manifest(
            format!("pluto.toml: dependency name '{}' is a reserved keyword", name),
            manifest_path.to_path_buf(),
        ));
    }

    Ok(())
}

fn validate_dep_path(name: &str, path: &Path, manifest_path: &Path) -> Result<(), CompileError> {
    if !path.exists() || !path.is_dir() {
        return Err(CompileError::manifest(
            format!(
                "pluto.toml: dependency '{}': path '{}' does not exist or is not a directory",
                name,
                path.display()
            ),
            manifest_path.to_path_buf(),
        ));
    }
    Ok(())
}

// ---- Package graph resolution ----

/// Find pluto.toml by walking from start_dir up to .git or FS root.
/// If found, parse and recursively resolve all transitive deps.
/// Returns PackageGraph. If no manifest found, returns PackageGraph::empty().
pub fn find_and_resolve(start_dir: &Path) -> Result<PackageGraph, CompileError> {
    let manifest_path = match find_manifest_walk(start_dir) {
        Some(p) => p,
        None => return Ok(PackageGraph::empty()),
    };

    let manifest_dir = manifest_path.parent().unwrap_or(Path::new("."));
    let canonical_root = manifest_dir.canonicalize().map_err(|e| {
        CompileError::manifest(
            format!("pluto.toml: cannot resolve root directory: {e}"),
            manifest_path.clone(),
        )
    })?;

    let mut resolving_stack: Vec<PathBuf> = Vec::new();
    let mut resolved_cache: HashSet<PathBuf> = HashSet::new();
    let mut packages: BTreeMap<PathBuf, PackageNode> = BTreeMap::new();

    resolve_package_node(
        &manifest_path,
        &canonical_root,
        &mut resolving_stack,
        &mut resolved_cache,
        &mut packages,
    )?;

    Ok(PackageGraph {
        root_dir: Some(canonical_root),
        packages,
    })
}

fn resolve_package_node(
    manifest_path: &Path,
    canonical_dir: &PathBuf,
    resolving_stack: &mut Vec<PathBuf>,
    resolved_cache: &mut HashSet<PathBuf>,
    packages: &mut BTreeMap<PathBuf, PackageNode>,
) -> Result<(), CompileError> {
    // Already fully resolved (handles diamond deps)
    if resolved_cache.contains(canonical_dir) {
        return Ok(());
    }

    // Cycle detection
    if resolving_stack.contains(canonical_dir) {
        let cycle_start = resolving_stack.iter().position(|p| p == canonical_dir).unwrap();
        let mut chain: Vec<String> = resolving_stack[cycle_start..]
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        chain.push(canonical_dir.display().to_string());
        return Err(CompileError::manifest(
            format!("pluto.toml: circular package dependency: {}", chain.join(" -> ")),
            manifest_path.to_path_buf(),
        ));
    }

    resolving_stack.push(canonical_dir.clone());

    let (manifest, manifest_dir) = parse_manifest(manifest_path)?;
    let pkg_name = manifest.package.as_ref()
        .and_then(|p| p.name.as_ref())
        .ok_or_else(|| CompileError::manifest(
            "pluto.toml: missing [package] section or 'name' field",
            manifest_path.to_path_buf(),
        ))?
        .clone();

    let mut dep_scope: DependencyScope = BTreeMap::new();

    for (dep_name, dep_spec) in &manifest.dependencies {
        validate_dep_name(dep_name, manifest_path)?;

        let dep_kind = validate_dep_spec(dep_name, dep_spec, manifest_path)?;

        let dep_path = match dep_kind {
            DepKind::Path(ref p) => manifest_dir.join(p),
            DepKind::Git(ref url, ref git_ref) => {
                git_cache::ensure_cached(url, git_ref, manifest_path)?
            }
        };

        if let DepKind::Path(_) = dep_kind {
            validate_dep_path(dep_name, &dep_path, manifest_path)?;
        }

        let dep_canonical = dep_path.canonicalize().map_err(|e| {
            CompileError::manifest(
                format!("pluto.toml: dependency '{}': cannot resolve path '{}': {e}", dep_name, dep_path.display()),
                manifest_path.to_path_buf(),
            )
        })?;

        dep_scope.insert(dep_name.clone(), dep_canonical.clone());

        // Recursively resolve dep's own manifest (if it has one)
        if let Some(dep_manifest_path) = read_manifest_direct(&dep_canonical) {
            resolve_package_node(
                &dep_manifest_path,
                &dep_canonical,
                resolving_stack,
                resolved_cache,
                packages,
            )?;
        }
        // If dep has no manifest, it's a leaf node — no PackageNode entry needed
        // (deps_for will return empty scope)
    }

    packages.insert(canonical_dir.clone(), PackageNode {
        name: pkg_name,
        root_dir: canonical_dir.clone(),
        dependencies: dep_scope,
    });

    resolving_stack.pop();
    resolved_cache.insert(canonical_dir.clone());

    Ok(())
}

// ---- Update command ----

/// Find the manifest and re-fetch all git dependencies (direct deps only for v1).
pub fn update_git_deps(start_dir: &Path) -> Result<Vec<String>, CompileError> {
    let manifest_path = match find_manifest_walk(start_dir) {
        Some(p) => p,
        None => return Err(CompileError::manifest(
            "no pluto.toml found",
            start_dir.to_path_buf(),
        )),
    };

    let (manifest, _manifest_dir) = parse_manifest(&manifest_path)?;

    let mut updated = Vec::new();

    for (dep_name, dep_spec) in &manifest.dependencies {
        let dep_kind = validate_dep_spec(dep_name, dep_spec, &manifest_path)?;
        if let DepKind::Git(url, git_ref) = dep_kind {
            git_cache::fetch_and_update(&url, &git_ref, &manifest_path)?;
            updated.push(dep_name.clone());
        }
    }

    Ok(updated)
}
