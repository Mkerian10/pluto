//! Compiler service API.
//!
//! This module defines the `CompilerService` trait, which provides a protocol-agnostic
//! interface to all compiler operations. Implementations can be:
//! - `InProcessServer`: Direct calls to the compiler library (Phase 5)
//! - Socket server: Remote calls via protocol (Phase 6)
//!
//! The trait is used by multiple frontends:
//! - CLI: Terminal-based interface with human-readable formatting
//! - MCP: JSON-RPC interface for AI agents
//! - Future: Language server protocol, web API, etc.

pub mod types;
pub mod in_process;

pub use in_process::InProcessServer;

use std::path::Path;
use types::*;
use uuid::Uuid;

/// Protocol-agnostic compiler service interface.
///
/// All operations return protocol-agnostic result types that frontends can format
/// for their specific protocols (terminal output, JSON-RPC, etc.).
///
/// ## Method Organization
///
/// Methods are grouped by concern:
/// 1. **Module Management** (5 methods) - Loading and tracking modules
/// 2. **Declaration Inspection** (3 methods) - Querying declarations
/// 3. **Cross-References & Analysis** (7 methods) - Finding usages, call graphs, etc.
/// 4. **Source Access** (2 methods) - Reading source code
/// 5. **Compilation & Execution** (4 methods) - Building and running programs
/// 6. **Analysis** (1 method) - Enriching binaries with derived data
/// 7. **Documentation** (2 methods) - Language and stdlib docs
///
/// ## Implementations
///
/// - `InProcessServer`: Calls compiler library directly, maintains module cache
/// - Future socket server: Sends requests over network to remote compiler service
pub trait CompilerService {
    // ========== Module Management (5 methods) ==========

    /// Load and analyze a .pluto source file or PLTO binary.
    ///
    /// Returns a summary of all top-level declarations. The returned path should be used
    /// as the key for subsequent queries. Modules are cached; repeated loads return cached data.
    fn load_module(&mut self, path: &Path, opts: &LoadOptions) -> Result<ModuleSummary, ServiceError>;

    /// Scan a directory for .pluto files and load all of them.
    ///
    /// Returns a summary of which files loaded successfully and which failed.
    /// Use `list_modules` and `find_declaration` to query the loaded project.
    fn load_project(&mut self, root: &Path, opts: &LoadOptions) -> Result<ProjectSummary, ServiceError>;

    /// List all currently loaded modules with metadata.
    fn list_modules(&self) -> Vec<ModuleInfo>;

    /// Reload a module from disk, discarding the cached version.
    ///
    /// Useful when files are modified outside the service.
    fn reload_module(&mut self, path: &Path, opts: &LoadOptions) -> Result<ModuleSummary, ServiceError>;

    /// Show status of all loaded modules, including whether they are stale.
    ///
    /// A module is stale if it has been modified on disk since it was loaded.
    fn module_status(&self) -> Vec<ModuleStatus>;

    // ========== Declaration Inspection (3 methods) ==========

    /// List declarations in a loaded module.
    ///
    /// Optionally filter by kind: function, class, enum, trait, error, app.
    fn list_declarations(&self, path: &Path, filter: Option<DeclKind>) -> Result<Vec<DeclSummary>, ServiceError>;

    /// Get detailed information about a specific declaration by UUID.
    ///
    /// Returns params, types, error sets, methods, fields, and pretty-printed source text.
    fn get_declaration(&self, path: &Path, id: Uuid) -> Result<DeclDetail, ServiceError>;

    /// Search for a declaration by name across all loaded modules.
    ///
    /// Returns matches from every module. Optionally filter by kind.
    fn find_declaration(&self, name: &str, filter: Option<DeclKind>) -> Vec<DeclMatch>;

    // ========== Cross-References & Analysis (7 methods) ==========

    /// Find all call sites that invoke a given function across all loaded modules.
    fn callers_of(&self, id: Uuid) -> Vec<XrefSite>;

    /// Find all sites where a class is constructed via struct literal.
    fn constructors_of(&self, id: Uuid) -> Vec<XrefSite>;

    /// Find all usages of an enum variant across all loaded modules.
    fn enum_usages_of(&self, id: Uuid) -> Vec<XrefSite>;

    /// Find all sites where a given error is raised across all loaded modules.
    fn raise_sites_of(&self, id: Uuid) -> Vec<XrefSite>;

    /// Find all usages of a declaration: calls, constructions, enum usages, and raise sites.
    ///
    /// Returns unified results with usage kind and module path.
    fn usages_of(&self, id: Uuid) -> Vec<XrefSite>;

    /// Build a call graph starting from a function UUID.
    ///
    /// Supports both directions: 'callees' (who this calls) and 'callers' (who calls this).
    /// Returns a tree structure with cycle detection.
    fn call_graph(&self, id: Uuid, opts: &CallGraphOptions) -> Result<CallGraphResult, ServiceError>;

    /// Get error handling info for a function: whether it is fallible and its error set.
    fn error_set(&self, path: &Path, id: Uuid) -> Result<ErrorSetInfo, ServiceError>;

    // ========== Source Access (2 methods) ==========

    /// Get source text from a loaded module, optionally at a specific byte range.
    ///
    /// If no range specified, returns the entire source.
    fn get_source(&self, path: &Path, range: Option<ByteRange>) -> Result<String, ServiceError>;

    /// Pretty-print a loaded module or specific declaration as Pluto source text.
    ///
    /// Optionally include UUID hints.
    fn pretty_print(&self, path: &Path, id: Option<Uuid>, include_uuids: bool) -> Result<String, ServiceError>;

    // ========== Compilation & Execution (4 methods) ==========

    /// Type-check a .pluto source file and return structured diagnostics.
    ///
    /// Returns errors and warnings with spans including line:col.
    /// Does NOT produce a binary. Compiler errors are returned in CheckResult, not as ServiceError.
    fn check(&self, path: &Path, opts: &CompileOptions) -> CheckResult;

    /// Compile a .pluto source file to a native binary.
    ///
    /// Returns the output path on success or structured error diagnostics on failure.
    fn compile(&self, path: &Path, output: &Path, opts: &CompileOptions) -> CompileResult;

    /// Compile and execute a .pluto source file, capturing stdout/stderr.
    ///
    /// Default timeout: 10s, max: 60s. Returns compilation errors or execution results.
    fn run(&self, path: &Path, opts: &RunOptions) -> RunResult;

    /// Compile in test mode and execute the test runner, capturing stdout/stderr.
    ///
    /// Default timeout: 30s, max: 60s. Returns compilation errors or test execution results.
    fn test(&self, path: &Path, opts: &TestOptions) -> TestResult;

    // ========== Analysis (1 method) ==========

    /// Analyze a file and update its PLTO binary with derived analysis data.
    ///
    /// This enriches the binary with type information, error sets, cross-references, etc.
    fn analyze_and_update(&self, path: &Path, opts: &LoadOptions) -> Result<(), ServiceError>;

    // ========== Documentation (2 methods) ==========

    /// Get Pluto language reference documentation.
    ///
    /// Optionally filter by topic: types, operators, statements, declarations, strings,
    /// errors, closures, generics, modules, contracts, concurrency, gotchas.
    /// Returns markdown-formatted reference text.
    fn language_docs(&self, topic: Option<&str>) -> Result<String, ServiceError>;

    /// Get Pluto stdlib documentation.
    ///
    /// Without a module name, lists all available stdlib modules.
    /// With a module name (e.g. 'strings', 'fs', 'math'), returns all pub function
    /// signatures and descriptions from that module.
    fn stdlib_docs(&self, module: Option<&str>) -> Result<String, ServiceError>;
}
