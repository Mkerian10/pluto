use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use rmcp::{
    ErrorData as McpError,
    ServerHandler,
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::*,
    tool, tool_router, tool_handler,
};
use tokio::sync::RwLock;
use uuid::Uuid;

use pluto::server::CompilerService;
use pluto::server::InProcessServer;
use pluto::server::types as service_types;
use pluto_sdk::Module;
use pluto_sdk::decl::DeclKind;
use pluto_sdk::editor::DanglingRefKind;

use crate::serialize;
use crate::tools::*;

/// Dependency graph tracking module imports.
#[derive(Clone, Debug, Default)]
struct DependencyGraph {
    /// Map from module name (e.g., "std.strings") to canonical file path
    name_to_path: HashMap<String, String>,
    /// Map from canonical file path to module name
    path_to_name: HashMap<String, String>,
    /// Map from canonical file path to list of imported module names
    dependencies: HashMap<String, Vec<String>>,
}

/// Metadata about a loaded module
struct ModuleMetadata {
    module: Module,
    /// File modification time when loaded
    loaded_at: SystemTime,
}

impl ModuleMetadata {
    fn new(module: Module, mtime: SystemTime) -> Self {
        Self {
            module,
            loaded_at: mtime,
        }
    }

    /// Check if the file has been modified since we loaded it
    fn is_stale(&self, path: &Path) -> bool {
        if let Ok(metadata) = std::fs::metadata(path) {
            if let Ok(current_mtime) = metadata.modified() {
                return current_mtime > self.loaded_at;
            }
        }
        false
    }
}


#[derive(Clone)]
pub struct PlutoMcp {
    service: Arc<RwLock<InProcessServer>>,
    modules: Arc<RwLock<HashMap<String, ModuleMetadata>>>,
    project_root: Arc<RwLock<Option<String>>>,
    dep_graph: Arc<RwLock<DependencyGraph>>,
    tool_router: ToolRouter<Self>,
}

fn mcp_err(msg: impl Into<String>) -> McpError {
    McpError::new(ErrorCode::INVALID_PARAMS, msg.into(), None::<serde_json::Value>)
}

fn mcp_internal(msg: impl Into<String>) -> McpError {
    McpError::new(ErrorCode::INTERNAL_ERROR, msg.into(), None::<serde_json::Value>)
}

/// Canonicalize a path, falling back to the original string on error.
fn canon(path: &str) -> String {
    std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string())
}

/// Validate that a path is safe to write to (within project root).
/// Returns the canonicalized path if valid, or an error if the path escapes the project root.
async fn validate_write_path(
    project_root: &Arc<RwLock<Option<String>>>,
    path: &str,
) -> Result<PathBuf, McpError> {
    // Get project root
    let root_opt = project_root.read().await;
    let root_str = root_opt.as_ref().ok_or_else(|| {
        mcp_err("No project root set. Use load_project first to establish a project root.")
    })?;
    let root_path = PathBuf::from(root_str);
    let canonical_root = std::fs::canonicalize(&root_path)
        .map_err(|e| mcp_internal(format!("Failed to canonicalize project root: {e}")))?;

    // Canonicalize the target path
    let target = PathBuf::from(path);

    let canonical_target: PathBuf = if target.exists() {
        std::fs::canonicalize(&target)
            .map_err(|e| mcp_internal(format!("Failed to canonicalize path: {e}")))?
    } else {
        // Validate parent directory exists and is within project root
        if let Some(parent) = target.parent() {
            if parent.as_os_str().is_empty() {
                // Relative path with no parent (e.g., "file.pluto")
                // Treat as relative to project root
                canonical_root.join(&target)
            } else if !parent.exists() {
                return Err(mcp_err(format!("Parent directory does not exist: {}", parent.display())));
            } else {
                let canonical_parent = std::fs::canonicalize(parent)
                    .map_err(|e| mcp_internal(format!("Failed to canonicalize parent: {e}")))?;
                canonical_parent.join(target.file_name().unwrap())
            }
        } else {
            // No parent means root-level path
            return Err(mcp_err("Cannot write to root-level path"));
        }
    };

    // Check if the canonical target is within the project root
    if !canonical_target.starts_with(&canonical_root) {
        return Err(mcp_err(format!(
            "Path safety violation: '{}' is outside project root '{}'",
            path,
            root_str
        )));
    }

    Ok(canonical_target)
}

#[tool_router]
impl PlutoMcp {
    pub fn new() -> Self {
        Self {
            service: Arc::new(RwLock::new(InProcessServer::new())),
            modules: Arc::new(RwLock::new(HashMap::new())),
            project_root: Arc::new(RwLock::new(None)),
            dep_graph: Arc::new(RwLock::new(DependencyGraph::default())),
            tool_router: Self::tool_router(),
        }
    }

    // --- Tool 1: load_module ---
    #[tool(description = "Load and analyze a Pluto source file (.pluto) or PLTO binary. Returns a summary of all top-level declarations. The returned path should be used as the key for subsequent queries.")]
    async fn load_module(
        &self,
        Parameters(input): Parameters<LoadModuleInput>,
    ) -> Result<CallToolResult, McpError> {
        let canonical = canon(&input.path);

        // Detect binary vs source by magic bytes
        let first_bytes = std::fs::read(&canonical)
            .map_err(|e| mcp_err(format!("Cannot read file: {e}")))?;

        let stdlib_path = input.stdlib.as_ref().map(|s| PathBuf::from(s));

        let module = if first_bytes.len() >= 4 && &first_bytes[..4] == b"PLTO" {
            Module::open(&canonical).map_err(|e| mcp_internal(format!("Failed to load binary: {e}")))?
        } else {
            // Load in standalone mode to exclude sibling files
            Module::from_source_file_standalone(&canonical, stdlib_path.as_deref())
                .map_err(|e| mcp_internal(format!("Failed to analyze source: {e}")))?
        };

        // Build summary (only local declarations, not imports)
        let funcs = module.local_functions();
        let classes = module.local_classes();
        let enums = module.local_enums();
        let traits = module.local_traits();
        let errors = module.local_errors();
        let app = module.app();

        let mut declarations = Vec::new();
        for d in &funcs {
            declarations.push(serialize::decl_to_summary(d));
        }
        for d in &classes {
            declarations.push(serialize::decl_to_summary(d));
        }
        for d in &enums {
            declarations.push(serialize::decl_to_summary(d));
        }
        for d in &traits {
            declarations.push(serialize::decl_to_summary(d));
        }
        for d in &errors {
            declarations.push(serialize::decl_to_summary(d));
        }
        if let Some(ref a) = app {
            declarations.push(serialize::decl_to_summary(a));
        }

        let result = serialize::ModuleSummary {
            path: canonical.clone(),
            summary: serialize::DeclCounts {
                functions: funcs.len(),
                classes: classes.len(),
                enums: enums.len(),
                traits: traits.len(),
                errors: errors.len(),
                app: if app.is_some() { 1 } else { 0 },
            },
            declarations,
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;

        // Capture file mtime when loading
        let mtime = std::fs::metadata(&canonical)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());

        self.modules.write().await.insert(canonical, ModuleMetadata::new(module, mtime));

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 2: list_declarations ---
    #[tool(description = "List declarations in a loaded module. Optionally filter by kind: function, class, enum, trait, error, app.")]
    async fn list_declarations(
        &self,
        Parameters(input): Parameters<ListDeclarationsInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;
        let metadata = self.find_module(&modules, &input.path)?;
        let module = &metadata.module;

        let decls: Vec<serialize::DeclSummary> = match input.kind.as_deref() {
            Some("function") => module.local_functions().iter().map(serialize::decl_to_summary).collect(),
            Some("class") => module.local_classes().iter().map(serialize::decl_to_summary).collect(),
            Some("enum") => module.local_enums().iter().map(serialize::decl_to_summary).collect(),
            Some("trait") => module.local_traits().iter().map(serialize::decl_to_summary).collect(),
            Some("error") => module.local_errors().iter().map(serialize::decl_to_summary).collect(),
            Some("app") => module.app().iter().map(serialize::decl_to_summary).collect(),
            None => {
                let mut all = Vec::new();
                for d in module.local_functions() { all.push(serialize::decl_to_summary(&d)); }
                for d in module.local_classes() { all.push(serialize::decl_to_summary(&d)); }
                for d in module.local_enums() { all.push(serialize::decl_to_summary(&d)); }
                for d in module.local_traits() { all.push(serialize::decl_to_summary(&d)); }
                for d in module.local_errors() { all.push(serialize::decl_to_summary(&d)); }
                if let Some(a) = module.app() { all.push(serialize::decl_to_summary(&a)); }
                all
            }
            Some(other) => return Err(mcp_err(format!(
                "Unknown kind '{other}'. Valid: function, class, enum, trait, error, app"
            ))),
        };

        let json = serde_json::to_string_pretty(&decls)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 3: get_declaration ---
    #[tool(description = "Deep inspection of a single declaration by UUID or name. Returns params, types, error sets, methods, fields, and pretty-printed source text. If name lookup is ambiguous, returns a disambiguation list.")]
    async fn get_declaration(
        &self,
        Parameters(input): Parameters<GetDeclarationInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;
        let metadata = self.find_module(&modules, &input.path)?;
        let module = &metadata.module;

        if input.uuid.is_none() && input.name.is_none() {
            return Err(mcp_err("Either 'uuid' or 'name' must be provided"));
        }

        // UUID lookup
        if let Some(uuid_str) = &input.uuid {
            let id = uuid_str
                .parse::<Uuid>()
                .map_err(|_| mcp_err(format!("Invalid UUID: {uuid_str}")))?;
            let decl = module
                .get(id)
                .ok_or_else(|| mcp_err(format!("No declaration found with UUID {uuid_str}")))?;
            let json = self.inspect_decl(&decl, module)?;
            return Ok(CallToolResult::success(vec![Content::text(json)]));
        }

        // Name lookup
        let name = input.name.as_ref().unwrap();
        let matches = module.find(name);
        match matches.len() {
            0 => Err(mcp_err(format!("No declaration found with name '{name}'"))),
            1 => {
                let json = self.inspect_decl(&matches[0], module)?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            _ => {
                // Disambiguation
                let candidates: Vec<serialize::DisambiguationEntry> = matches
                    .iter()
                    .map(|d| serialize::DisambiguationEntry {
                        uuid: d.id().to_string(),
                        name: d.name().to_string(),
                        kind: serialize::decl_kind_to_string(d.kind()).to_string(),
                    })
                    .collect();
                let json = serde_json::to_string_pretty(&serde_json::json!({
                    "ambiguous": true,
                    "message": format!("Name '{name}' matches {} declarations. Specify a UUID to disambiguate.", matches.len()),
                    "candidates": candidates,
                }))
                .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
        }
    }

    // --- Tool 4: callers_of ---
    #[tool(description = "Find all call sites that invoke a given function across all loaded modules. Searches the entire project to find every location where the function is called.")]
    async fn callers_of(
        &self,
        Parameters(input): Parameters<CallersOfInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;

        let id = input
            .uuid
            .parse::<Uuid>()
            .map_err(|_| mcp_err(format!("Invalid UUID: {}", input.uuid)))?;

        let mut all_sites = Vec::new();

        // Search all loaded modules for callers
        for (module_path, metadata) in modules.iter() {
            let module = &metadata.module;
            let sites = module.callers_of(id);
            for site in sites {
                let func_id = module.find(&site.caller.name.node)
                    .first()
                    .map(|d| d.id().to_string());
                all_sites.push(serialize::CrossModuleXrefSiteInfo {
                    module_path: module_path.clone(),
                    function_name: site.caller.name.node.clone(),
                    function_uuid: func_id,
                    span: serialize::span_to_info(site.span),
                });
            }
        }

        let json = serde_json::to_string_pretty(&all_sites)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 4b: constructors_of ---
    #[tool(description = "Find all sites where a class is constructed via struct literal across all loaded modules.")]
    async fn constructors_of(
        &self,
        Parameters(input): Parameters<ConstructorsOfInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;

        let id = input
            .uuid
            .parse::<Uuid>()
            .map_err(|_| mcp_err(format!("Invalid UUID: {}", input.uuid)))?;

        let mut all_sites = Vec::new();

        // Search all loaded modules
        for (module_path, metadata) in modules.iter() {
            let module = &metadata.module;
            let sites = module.constructors_of(id);
            for site in sites {
                let func_id = module.find(&site.function.name.node)
                    .first()
                    .map(|d| d.id().to_string());
                all_sites.push(serialize::CrossModuleXrefSiteInfo {
                    module_path: module_path.clone(),
                    function_name: site.function.name.node.clone(),
                    function_uuid: func_id,
                    span: serialize::span_to_info(site.span),
                });
            }
        }

        let json = serde_json::to_string_pretty(&all_sites)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 4c: enum_usages_of ---
    #[tool(description = "Find all usages of an enum variant across all loaded modules.")]
    async fn enum_usages_of(
        &self,
        Parameters(input): Parameters<EnumUsagesOfInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;

        let id = input
            .uuid
            .parse::<Uuid>()
            .map_err(|_| mcp_err(format!("Invalid UUID: {}", input.uuid)))?;

        let mut all_sites = Vec::new();

        // Search all loaded modules
        for (module_path, metadata) in modules.iter() {
            let module = &metadata.module;
            let sites = module.enum_usages_of(id);
            for site in sites {
                let func_id = module.find(&site.function.name.node)
                    .first()
                    .map(|d| d.id().to_string());
                all_sites.push(serialize::CrossModuleXrefSiteInfo {
                    module_path: module_path.clone(),
                    function_name: site.function.name.node.clone(),
                    function_uuid: func_id,
                    span: serialize::span_to_info(site.span),
                });
            }
        }

        let json = serde_json::to_string_pretty(&all_sites)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 4d: raise_sites_of ---
    #[tool(description = "Find all sites where a given error is raised across all loaded modules.")]
    async fn raise_sites_of(
        &self,
        Parameters(input): Parameters<RaiseSitesOfInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;

        let id = input
            .uuid
            .parse::<Uuid>()
            .map_err(|_| mcp_err(format!("Invalid UUID: {}", input.uuid)))?;

        let mut all_sites = Vec::new();

        // Search all loaded modules
        for (module_path, metadata) in modules.iter() {
            let module = &metadata.module;
            let sites = module.raise_sites_of(id);
            for site in sites {
                let func_id = module.find(&site.function.name.node)
                    .first()
                    .map(|d| d.id().to_string());
                all_sites.push(serialize::CrossModuleXrefSiteInfo {
                    module_path: module_path.clone(),
                    function_name: site.function.name.node.clone(),
                    function_uuid: func_id,
                    span: serialize::span_to_info(site.span),
                });
            }
        }

        let json = serde_json::to_string_pretty(&all_sites)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 4e: usages_of (unified) ---
    #[tool(description = "Find all usages of a declaration across all loaded modules: calls, constructions, enum usages, and raise sites. Returns unified results with usage_kind and module_path.")]
    async fn usages_of(
        &self,
        Parameters(input): Parameters<UsagesOfInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;

        let id = input
            .uuid
            .parse::<Uuid>()
            .map_err(|_| mcp_err(format!("Invalid UUID: {}", input.uuid)))?;

        let mut results: Vec<serialize::UnifiedXrefInfo> = Vec::new();

        // Search all loaded modules
        for (module_path, metadata) in modules.iter() {
            let module = &metadata.module;
            // Collect call sites
            for site in module.callers_of(id) {
                let func_id = module.find(&site.caller.name.node).first().map(|d| d.id().to_string());
                results.push(serialize::UnifiedXrefInfo {
                    module_path: module_path.clone(),
                    usage_kind: "call".to_string(),
                    function_name: site.caller.name.node.clone(),
                    function_uuid: func_id,
                    span: serialize::span_to_info(site.span),
                });
            }
            // Collect constructor sites
            for site in module.constructors_of(id) {
                let func_id = module.find(&site.function.name.node).first().map(|d| d.id().to_string());
                results.push(serialize::UnifiedXrefInfo {
                    module_path: module_path.clone(),
                    usage_kind: "construct".to_string(),
                    function_name: site.function.name.node.clone(),
                    function_uuid: func_id,
                    span: serialize::span_to_info(site.span),
                });
            }
            // Collect enum usages
            for site in module.enum_usages_of(id) {
                let func_id = module.find(&site.function.name.node).first().map(|d| d.id().to_string());
                results.push(serialize::UnifiedXrefInfo {
                    module_path: module_path.clone(),
                    usage_kind: "enum_variant".to_string(),
                    function_name: site.function.name.node.clone(),
                    function_uuid: func_id,
                    span: serialize::span_to_info(site.span),
                });
            }
            // Collect raise sites
            for site in module.raise_sites_of(id) {
                let func_id = module.find(&site.function.name.node).first().map(|d| d.id().to_string());
                results.push(serialize::UnifiedXrefInfo {
                    module_path: module_path.clone(),
                    usage_kind: "raise".to_string(),
                    function_name: site.function.name.node.clone(),
                    function_uuid: func_id,
                    span: serialize::span_to_info(site.span),
                });
            }
        }

        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 4f: call_graph ---
    #[tool(description = "Build a call graph starting from a function UUID. Supports both directions: 'callees' (who this calls) and 'callers' (who calls this). Returns a tree structure with cycle detection.")]
    async fn call_graph(
        &self,
        Parameters(input): Parameters<CallGraphInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;

        let root_id = input
            .uuid
            .parse::<Uuid>()
            .map_err(|_| mcp_err(format!("Invalid UUID: {}", input.uuid)))?;

        let max_depth = input.max_depth.unwrap_or(5).min(20);
        let direction = input.direction.as_deref().unwrap_or("callees");

        if direction != "callees" && direction != "callers" {
            return Err(mcp_err(format!("Invalid direction '{}'. Must be 'callees' or 'callers'", direction)));
        }

        // Find the root function across all modules
        let mut root_name = None;
        let mut root_module_path = None;
        for (path, metadata) in modules.iter() {
            let module = &metadata.module;
            if let Some(decl) = module.get(root_id) {
                root_name = Some(decl.name().to_string());
                root_module_path = Some(path.clone());
                break;
            }
        }

        let root_name = root_name.ok_or_else(|| mcp_err(format!("No function found with UUID {}", input.uuid)))?;
        let root_module_path = root_module_path.unwrap();

        // Build the call graph
        let mut nodes = Vec::new();
        let mut visited = HashSet::new();
        self.build_call_graph_recursive(
            root_id,
            &root_name,
            &root_module_path,
            0,
            max_depth,
            direction,
            &modules,
            &mut visited,
            &mut nodes,
        ).await;

        let result = serialize::CallGraphResult {
            root_uuid: root_id.to_string(),
            root_name,
            direction: direction.to_string(),
            max_depth,
            nodes,
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 5: error_set ---
    #[tool(description = "Get error handling info for a function: whether it is fallible and its error set.")]
    async fn error_set(
        &self,
        Parameters(input): Parameters<ErrorSetInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;
        let metadata = self.find_module(&modules, &input.path)?;
        let module = &metadata.module;

        let id = input
            .uuid
            .parse::<Uuid>()
            .map_err(|_| mcp_err(format!("Invalid UUID: {}", input.uuid)))?;

        let decl = module
            .get(id)
            .ok_or_else(|| mcp_err(format!("No declaration found with UUID {}", input.uuid)))?;

        let result = serialize::ErrorsResult {
            function_name: decl.name().to_string(),
            is_fallible: module.is_fallible(id),
            error_set: module
                .error_set_of(id)
                .iter()
                .map(|e| serialize::ErrorRefInfo {
                    name: e.name.clone(),
                    uuid: e.id.map(|u| u.to_string()),
                })
                .collect(),
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 6: get_source ---
    #[tool(description = "Get source text from a loaded module, optionally at a specific byte range. If no range specified, returns the entire source.")]
    async fn get_source(
        &self,
        Parameters(input): Parameters<GetSourceInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;
        let metadata = self.find_module(&modules, &input.path)?;
        let module = &metadata.module;

        let src = module.source();
        let len = src.len();
        let start = input.start.unwrap_or(0);
        let end = input.end.unwrap_or(len);

        if start > len || end > len || start > end {
            return Err(mcp_err(format!(
                "Byte range [{start}..{end}) out of bounds (source length: {len})"
            )));
        }
        if !src.is_char_boundary(start) || !src.is_char_boundary(end) {
            return Err(mcp_err("Byte offset not on UTF-8 character boundary"));
        }

        let slice = &src[start..end];
        Ok(CallToolResult::success(vec![Content::text(slice)]))
    }

    // --- Tool 7: load_project ---
    #[tool(description = "Scan a directory for .pluto files and load all of them. Returns a summary of which files loaded successfully and which failed. Use list_modules and find_declaration to query the loaded project.")]
    async fn load_project(
        &self,
        Parameters(input): Parameters<LoadProjectInput>,
    ) -> Result<CallToolResult, McpError> {
        let root = canon(&input.path);
        let root_path = Path::new(&root);

        if !root_path.is_dir() {
            return Err(mcp_err(format!("Not a directory: {root}")));
        }

        let files = discover_pluto_files(root_path)
            .map_err(|e| mcp_internal(format!("Failed to scan directory: {e}")))?;

        let stdlib_path = input.stdlib.as_ref().map(|s| PathBuf::from(s));
        let mut load_errors = Vec::new();

        let mut modules = self.modules.write().await;

        for file in &files {
            let canonical = canon(&file.to_string_lossy());
            // Read file contents
            let source = match std::fs::read_to_string(&canonical) {
                Ok(s) => s,
                Err(e) => {
                    load_errors.push(serialize::LoadError {
                        path: canonical,
                        error: format!("Failed to read file: {}", e),
                    });
                    continue;
                }
            };

            // Parse without following imports (each file loaded independently)
            match Module::from_source(&source) {
                Ok(module) => {
                    // Capture file mtime when loading
                    let mtime = std::fs::metadata(&canonical)
                        .and_then(|m| m.modified())
                        .unwrap_or_else(|_| SystemTime::now());
                    modules.insert(canonical, ModuleMetadata::new(module, mtime));
                }
                Err(e) => {
                    load_errors.push(serialize::LoadError {
                        path: canonical,
                        error: e.to_string(),
                    });
                }
            }
        }

        // Build modules_loaded from the deduplicated modules HashMap
        let mut modules_loaded: Vec<serialize::ModuleBrief> = modules
            .iter()
            .map(|(path, metadata)| {
                let module = &metadata.module;
                let decl_count = module.local_functions().len()
                    + module.local_classes().len()
                    + module.local_enums().len()
                    + module.local_traits().len()
                    + module.local_errors().len()
                    + if module.app().is_some() { 1 } else { 0 };
                serialize::ModuleBrief {
                    path: path.clone(),
                    declarations: decl_count,
                }
            })
            .collect();
        modules_loaded.sort_by(|a, b| a.path.cmp(&b.path));

        *self.project_root.write().await = Some(root.clone());

        // Build dependency graph from loaded modules
        let mut dep_graph = self.dep_graph.write().await;
        dep_graph.name_to_path.clear();
        dep_graph.path_to_name.clear();
        dep_graph.dependencies.clear();

        // Build module name → path mapping
        // For each .pluto file, derive the module name from its path relative to project root
        for (path, _metadata) in modules.iter() {
            if let Ok(rel_path) = Path::new(path).strip_prefix(&root) {
                let module_name = derive_module_name(rel_path);
                dep_graph.name_to_path.insert(module_name.clone(), path.clone());
                dep_graph.path_to_name.insert(path.clone(), module_name);
            }
        }

        // Extract imports from each module and populate dependencies
        for (path, metadata) in modules.iter() {
            let module = &metadata.module;
            let program = module.program();
            let mut imported_names = Vec::new();

            for import in &program.imports {
                let import_path = &import.node.path;
                let module_name = import_path.iter()
                    .map(|s| s.node.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                imported_names.push(module_name);
            }

            if !imported_names.is_empty() {
                dep_graph.dependencies.insert(path.clone(), imported_names);
            }
        }

        // Detect circular imports (but don't fail - just report in output)
        let has_circular = detect_circular_imports(&dep_graph).is_err();

        // Build dependency graph info for output
        let mut dep_info_modules = Vec::new();
        for (path, deps) in &dep_graph.dependencies {
            if let Some(name) = dep_graph.path_to_name.get(path) {
                dep_info_modules.push(serialize::ModuleDependencyInfo {
                    path: path.clone(),
                    name: name.clone(),
                    imports: deps.clone(),
                });
            }
        }
        dep_info_modules.sort_by(|a, b| a.path.cmp(&b.path));

        let dependency_graph = if !dep_info_modules.is_empty() {
            Some(serialize::DependencyGraphInfo {
                module_count: dep_graph.path_to_name.len(),
                has_circular_imports: has_circular,
                modules: dep_info_modules,
            })
        } else {
            None
        };

        let result = serialize::ProjectSummary {
            project_root: root,
            files_found: files.len(),
            files_loaded: modules_loaded.len(),
            files_failed: load_errors.len(),
            modules: modules_loaded,
            errors: load_errors,
            dependency_graph,
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 8: list_modules ---
    #[tool(description = "List all currently loaded modules with declaration counts.")]
    async fn list_modules(
        &self,
        #[allow(unused_variables)]
        Parameters(_input): Parameters<ListModulesInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;

        let mut entries: Vec<serialize::ModuleListEntry> = modules
            .iter()
            .map(|(path, metadata)| {
                let module = &metadata.module;
                let funcs = module.local_functions().len();
                let classes = module.local_classes().len();
                let enums = module.local_enums().len();
                let traits = module.local_traits().len();
                let errors = module.local_errors().len();
                let app = if module.app().is_some() { 1 } else { 0 };
                serialize::ModuleListEntry {
                    path: path.clone(),
                    summary: serialize::DeclCounts {
                        functions: funcs,
                        classes,
                        enums,
                        traits,
                        errors,
                        app,
                    },
                }
            })
            .collect();

        entries.sort_by(|a, b| a.path.cmp(&b.path));

        let json = serde_json::to_string_pretty(&entries)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 9: find_declaration ---
    #[tool(description = "Search for a declaration by name across all loaded modules. Returns matches from every module.")]
    async fn find_declaration(
        &self,
        Parameters(input): Parameters<FindDeclarationInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;

        if modules.is_empty() {
            return Err(mcp_err("No modules loaded. Use load_module or load_project first."));
        }

        let kind_filter = match input.kind.as_deref() {
            Some("function") => Some(DeclKind::Function),
            Some("class") => Some(DeclKind::Class),
            Some("enum") => Some(DeclKind::Enum),
            Some("trait") => Some(DeclKind::Trait),
            Some("error") => Some(DeclKind::Error),
            Some("app") => Some(DeclKind::App),
            None => None,
            Some(other) => return Err(mcp_err(format!(
                "Unknown kind '{other}'. Valid: function, class, enum, trait, error, app"
            ))),
        };

        let mut results: Vec<serialize::CrossModuleMatch> = Vec::new();

        for (path, metadata) in modules.iter() {
            let module = &metadata.module;
            let matches = module.find(&input.name);
            for decl in matches {
                // Skip imported declarations (those with '.' in name)
                if decl.name().contains('.') {
                    continue;
                }
                if let Some(filter) = &kind_filter {
                    if decl.kind() != *filter {
                        continue;
                    }
                }
                results.push(serialize::CrossModuleMatch {
                    module_path: path.clone(),
                    uuid: decl.id().to_string(),
                    name: decl.name().to_string(),
                    kind: serialize::decl_kind_to_string(decl.kind()).to_string(),
                });
            }
        }

        results.sort_by(|a, b| a.module_path.cmp(&b.module_path).then(a.name.cmp(&b.name)));

        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 10: check ---
    #[tool(description = "Type-check a .pluto source file and return structured diagnostics (errors and warnings with spans including line:col). Does NOT produce a binary. Compiler errors are returned as structured JSON (not MCP errors).")]
    async fn check(
        &self,
        Parameters(input): Parameters<CheckInput>,
    ) -> Result<CallToolResult, McpError> {
        let canonical = canon(&input.path);
        let opts = service_types::CompileOptions {
            stdlib: input.stdlib.map(PathBuf::from),
            ..Default::default()
        };

        let service = self.service.read().await;
        let result = service.check(Path::new(&canonical), &opts);

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 11: compile ---
    #[tool(description = "Compile a .pluto source file to a native binary. Returns the output path on success or structured error diagnostics (with line:col) on failure.")]
    async fn compile(
        &self,
        Parameters(input): Parameters<CompileInput>,
    ) -> Result<CallToolResult, McpError> {
        // Validate path safety
        let validated_path = validate_write_path(&self.project_root, &input.path).await?;
        let canonical = validated_path.to_string_lossy().to_string();

        let output_path = match &input.output {
            Some(p) => PathBuf::from(p),
            None => {
                let dir = tempfile::tempdir()
                    .map_err(|e| mcp_internal(format!("Failed to create temp dir: {e}")))?;
                // Leak the tempdir so it isn't deleted when dropped
                let path = dir.path().join(format!("pluto_{}", uuid::Uuid::new_v4()));
                std::mem::forget(dir);
                path
            }
        };

        let opts = service_types::CompileOptions {
            stdlib: input.stdlib.map(PathBuf::from),
            ..Default::default()
        };

        let service = self.service.read().await;
        let result = service.compile(Path::new(&canonical), &output_path, &opts);

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 12: run ---
    #[tool(description = "Compile and execute a .pluto source file, capturing stdout/stderr. Default timeout: 10s, max: 60s. Returns compilation errors or execution results (stdout, stderr, exit code, timed_out).")]
    async fn run(
        &self,
        Parameters(input): Parameters<RunInput>,
    ) -> Result<CallToolResult, McpError> {
        // Validate path safety
        let validated_path = validate_write_path(&self.project_root, &input.path).await?;
        let canonical = validated_path.to_string_lossy().to_string();

        let opts = service_types::RunOptions {
            stdlib: input.stdlib.map(PathBuf::from),
            timeout_ms: Some(input.timeout_ms.unwrap_or(10_000).min(60_000)),
            cwd: input.cwd.map(PathBuf::from)
                .or_else(|| Path::new(&canonical).parent().map(|p| p.to_path_buf())),
        };

        let service = self.service.read().await;
        let result = service.run(Path::new(&canonical), &opts);

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 13: test ---
    #[tool(description = "Compile a .pluto source file in test mode and execute the test runner, capturing stdout/stderr. Default timeout: 30s, max: 60s. Returns compilation errors or test execution results.")]
    async fn test(
        &self,
        Parameters(input): Parameters<TestInput>,
    ) -> Result<CallToolResult, McpError> {
        // Validate path safety
        let validated_path = validate_write_path(&self.project_root, &input.path).await?;
        let canonical = validated_path.to_string_lossy().to_string();

        let opts = service_types::TestOptions {
            stdlib: input.stdlib.map(PathBuf::from),
            timeout_ms: Some(input.timeout_ms.unwrap_or(30_000).min(60_000)),
            cwd: input.cwd.map(PathBuf::from)
                .or_else(|| Path::new(&canonical).parent().map(|p| p.to_path_buf())),
        };

        let service = self.service.read().await;
        let result = service.test(Path::new(&canonical), &opts);

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 14: add_declaration ---
    #[tool(description = "Add a new top-level declaration to a .pluto source file. The file is created if it doesn't exist. Returns the UUID, name, and kind of the added declaration. Supports multiple declarations in a single source string — all will be added and their details returned as an array.")]
    async fn add_declaration(
        &self,
        Parameters(input): Parameters<AddDeclarationInput>,
    ) -> Result<CallToolResult, McpError> {
        // Validate path safety first
        let validated_path = validate_write_path(&self.project_root, &input.path).await?;

        // Create file if needed
        if !validated_path.exists() {
            if let Some(parent) = validated_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| mcp_internal(format!("Failed to create directories: {e}")))?;
            }
            std::fs::write(&validated_path, "")
                .map_err(|e| mcp_internal(format!("Failed to create file: {e}")))?;
        }

        let canonical = validated_path.to_string_lossy().to_string();
        let contents = std::fs::read_to_string(&canonical).unwrap_or_default();
        let module = Module::from_source(&contents)
            .map_err(|e| mcp_internal(format!("Failed to parse file: {e}")))?;

        let mut editor = module.edit();
        let ids = editor.add_many_from_source(&input.source)
            .map_err(|e| mcp_err(format!("Failed to add declaration: {e}")))?;

        let module = editor.commit();

        // Collect results for all added declarations
        let mut results: Vec<serialize::AddDeclResult> = Vec::new();
        for id in &ids {
            if let Some(decl) = module.get(*id) {
                results.push(serialize::AddDeclResult {
                    uuid: id.to_string(),
                    name: decl.name().to_string(),
                    kind: serialize::decl_kind_to_string(decl.kind()).to_string(),
                });
            }
        }

        std::fs::write(&canonical, module.source())
            .map_err(|e| mcp_internal(format!("Failed to write file: {e}")))?;

        // Capture file mtime after write
        let mtime = std::fs::metadata(&canonical)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());

        self.modules.write().await.insert(canonical, ModuleMetadata::new(module, mtime));

        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 15: replace_declaration ---
    #[tool(description = "Replace a top-level declaration in a .pluto source file with new source code. The replacement must be the same kind (function→function, class→class, etc.). Identifies the target by name.")]
    async fn replace_declaration(
        &self,
        Parameters(input): Parameters<ReplaceDeclarationInput>,
    ) -> Result<CallToolResult, McpError> {
        // Validate path safety
        let validated_path = validate_write_path(&self.project_root, &input.path).await?;
        let canonical = validated_path.to_string_lossy().to_string();

        let contents = std::fs::read_to_string(&canonical)
            .map_err(|e| mcp_err(format!("Cannot read file: {e}")))?;
        let module = Module::from_source(&contents)
            .map_err(|e| mcp_internal(format!("Failed to parse file: {e}")))?;

        let (id, kind) = find_decl_by_name(&module, &input.name)?;

        let mut editor = module.edit();
        editor.replace_from_source(id, &input.source)
            .map_err(|e| mcp_err(format!("Failed to replace declaration: {e}")))?;

        let module = editor.commit();

        std::fs::write(&canonical, module.source())
            .map_err(|e| mcp_internal(format!("Failed to write file: {e}")))?;

        // Capture file mtime after write
        let mtime = std::fs::metadata(&canonical)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());

        self.modules.write().await.insert(canonical, ModuleMetadata::new(module, mtime));

        let result = serialize::ReplaceDeclResult {
            uuid: id.to_string(),
            name: input.name,
            kind: kind.to_string(),
        };
        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 16: delete_declaration ---
    #[tool(description = "Delete a top-level declaration from a .pluto source file. Returns the deleted source text and any dangling references found.")]
    async fn delete_declaration(
        &self,
        Parameters(input): Parameters<DeleteDeclarationInput>,
    ) -> Result<CallToolResult, McpError> {
        // Validate path safety
        let validated_path = validate_write_path(&self.project_root, &input.path).await?;
        let canonical = validated_path.to_string_lossy().to_string();

        let contents = std::fs::read_to_string(&canonical)
            .map_err(|e| mcp_err(format!("Cannot read file: {e}")))?;
        let module = Module::from_source(&contents)
            .map_err(|e| mcp_internal(format!("Failed to parse file: {e}")))?;

        let (id, _kind) = find_decl_by_name(&module, &input.name)?;

        let mut editor = module.edit();
        let delete_result = editor.delete(id)
            .map_err(|e| mcp_err(format!("Failed to delete declaration: {e}")))?;

        let module = editor.commit();

        std::fs::write(&canonical, module.source())
            .map_err(|e| mcp_internal(format!("Failed to write file: {e}")))?;

        // Capture file mtime after write
        let mtime = std::fs::metadata(&canonical)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());

        self.modules.write().await.insert(canonical, ModuleMetadata::new(module, mtime));

        let dangling_refs = delete_result.dangling.iter().map(|d| {
            serialize::DanglingRefInfo {
                kind: dangling_ref_kind_str(d.kind),
                name: d.name.clone(),
                span: serialize::span_to_info(d.span),
            }
        }).collect();

        let result = serialize::DeleteDeclResult {
            deleted_source: delete_result.source,
            dangling_refs,
        };
        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 17: rename_declaration ---
    #[tool(description = "Rename a top-level declaration and update all references within the file. Returns the old name, new name, and UUID.")]
    async fn rename_declaration(
        &self,
        Parameters(input): Parameters<RenameDeclarationInput>,
    ) -> Result<CallToolResult, McpError> {
        // Validate path safety
        let validated_path = validate_write_path(&self.project_root, &input.path).await?;
        let canonical = validated_path.to_string_lossy().to_string();

        let contents = std::fs::read_to_string(&canonical)
            .map_err(|e| mcp_err(format!("Cannot read file: {e}")))?;
        let module = Module::from_source(&contents)
            .map_err(|e| mcp_internal(format!("Failed to parse file: {e}")))?;

        let (id, _kind) = find_decl_by_name(&module, &input.old_name)?;

        let mut editor = module.edit();
        editor.rename(id, &input.new_name)
            .map_err(|e| mcp_err(format!("Failed to rename declaration: {e}")))?;

        let module = editor.commit();

        std::fs::write(&canonical, module.source())
            .map_err(|e| mcp_internal(format!("Failed to write file: {e}")))?;

        // Capture file mtime after write
        let mtime = std::fs::metadata(&canonical)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());

        self.modules.write().await.insert(canonical, ModuleMetadata::new(module, mtime));

        let result = serialize::RenameDeclResult {
            old_name: input.old_name,
            new_name: input.new_name,
            uuid: id.to_string(),
        };
        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 18: add_method ---
    #[tool(description = "Add a method to a class in a .pluto source file. The method source must include a self parameter. Returns the UUID and name of the added method.")]
    async fn add_method(
        &self,
        Parameters(input): Parameters<AddMethodInput>,
    ) -> Result<CallToolResult, McpError> {
        // Validate path safety
        let validated_path = validate_write_path(&self.project_root, &input.path).await?;
        let canonical = validated_path.to_string_lossy().to_string();

        let contents = std::fs::read_to_string(&canonical)
            .map_err(|e| mcp_err(format!("Cannot read file: {e}")))?;
        let module = Module::from_source(&contents)
            .map_err(|e| mcp_internal(format!("Failed to parse file: {e}")))?;

        let class_id = find_class_by_name(&module, &input.class_name)?;

        let mut editor = module.edit();
        let method_id = editor.add_method_from_source(class_id, &input.source)
            .map_err(|e| mcp_err(format!("Failed to add method: {e}")))?;

        // Get method name from the editor's in-progress program before commit
        let method_name = editor.program().classes.iter()
            .flat_map(|c| c.node.methods.iter())
            .find(|m| m.node.id == method_id)
            .map(|m| m.node.name.node.clone())
            .unwrap_or_default();

        let module = editor.commit();

        std::fs::write(&canonical, module.source())
            .map_err(|e| mcp_internal(format!("Failed to write file: {e}")))?;

        // Capture file mtime after write
        let mtime = std::fs::metadata(&canonical)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());

        self.modules.write().await.insert(canonical, ModuleMetadata::new(module, mtime));

        let result = serialize::AddMethodResult {
            uuid: method_id.to_string(),
            name: method_name,
        };
        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 19: add_field ---
    #[tool(description = "Add a field to a class in a .pluto source file. Returns the UUID of the added field.")]
    async fn add_field(
        &self,
        Parameters(input): Parameters<AddFieldInput>,
    ) -> Result<CallToolResult, McpError> {
        // Validate path safety
        let validated_path = validate_write_path(&self.project_root, &input.path).await?;
        let canonical = validated_path.to_string_lossy().to_string();

        let contents = std::fs::read_to_string(&canonical)
            .map_err(|e| mcp_err(format!("Cannot read file: {e}")))?;
        let module = Module::from_source(&contents)
            .map_err(|e| mcp_internal(format!("Failed to parse file: {e}")))?;

        let class_id = find_class_by_name(&module, &input.class_name)?;

        let mut editor = module.edit();
        let field_id = editor.add_field(class_id, &input.field_name, &input.field_type)
            .map_err(|e| mcp_err(format!("Failed to add field: {e}")))?;

        let module = editor.commit();

        std::fs::write(&canonical, module.source())
            .map_err(|e| mcp_internal(format!("Failed to write file: {e}")))?;

        // Capture file mtime after write
        let mtime = std::fs::metadata(&canonical)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());

        self.modules.write().await.insert(canonical, ModuleMetadata::new(module, mtime));

        let result = serialize::AddFieldResult { uuid: field_id.to_string() };
        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 20: docs ---
    #[tool(description = "Get Pluto language reference documentation. Optionally filter by topic: types, operators, statements, declarations, strings, errors, closures, generics, modules, contracts, concurrency, gotchas. Returns markdown-formatted reference text.")]
    async fn docs(
        &self,
        Parameters(input): Parameters<DocsInput>,
    ) -> Result<CallToolResult, McpError> {
        let service = self.service.read().await;
        let text = service.language_docs(input.topic.as_deref())
            .map_err(|e| mcp_internal(format!("{e}")))?;
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    // --- Tool 21: stdlib_docs ---
    #[tool(description = "Get Pluto stdlib documentation. Without a module name, lists all available stdlib modules. With a module name (e.g. 'strings', 'fs', 'math'), returns all pub function signatures and descriptions from that module.")]
    async fn stdlib_docs(
        &self,
        Parameters(input): Parameters<StdlibDocsInput>,
    ) -> Result<CallToolResult, McpError> {
        let service = self.service.read().await;
        let text = service.stdlib_docs(input.module.as_deref())
            .map_err(|e| mcp_internal(format!("{e}")))?;
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    // --- Tool 22: reload_module ---
    #[tool(description = "Reload a module from disk, discarding the cached version. Useful when files are modified outside the MCP server.")]
    async fn reload_module(
        &self,
        Parameters(input): Parameters<ReloadModuleInput>,
    ) -> Result<CallToolResult, McpError> {
        let canonical = canon(&input.path);

        // Check if module is loaded
        {
            let modules = self.modules.read().await;
            if !modules.contains_key(&canonical) {
                return Err(mcp_err(format!("Module not loaded: '{}'", canonical)));
            }
        }

        // Reload from disk
        let first_bytes = std::fs::read(&canonical)
            .map_err(|e| mcp_err(format!("Cannot read file: {e}")))?;

        let module = if first_bytes.len() >= 4 && &first_bytes[..4] == b"PLTO" {
            Module::open(&canonical).map_err(|e| mcp_internal(format!("Failed to load binary: {e}")))?
        } else {
            Module::from_source_file(&canonical)
                .map_err(|e| mcp_internal(format!("Failed to analyze source: {e}")))?
        };

        // Capture file mtime
        let mtime = std::fs::metadata(&canonical)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());

        // Replace in cache
        self.modules.write().await.insert(canonical.clone(), ModuleMetadata::new(module, mtime));

        let result = serialize::ReloadResult {
            path: canonical,
            reloaded: true,
            message: "Module reloaded successfully".to_string(),
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 23: module_status ---
    #[tool(description = "Show status of all loaded modules, including whether they are stale (modified on disk since load).")]
    async fn module_status(
        &self,
        #[allow(unused_variables)]
        Parameters(_input): Parameters<ModuleStatusInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;

        let mut entries: Vec<serialize::ModuleStatusEntry> = modules
            .iter()
            .map(|(path, metadata)| {
                let is_stale = metadata.is_stale(Path::new(path));
                let loaded_at = metadata.loaded_at
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| format!("{}", d.as_secs()))
                    .unwrap_or_else(|_| "unknown".to_string());
                serialize::ModuleStatusEntry {
                    path: path.clone(),
                    is_stale,
                    loaded_at,
                }
            })
            .collect();

        entries.sort_by(|a, b| a.path.cmp(&b.path));

        let json = serde_json::to_string_pretty(&entries)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 24: sync_pt ---
    #[tool(description = "Sync human edits from a .pt text file back to a .pluto binary file, preserving UUIDs where declarations match by name.")]
    async fn sync_pt(
        &self,
        Parameters(input): Parameters<SyncPtInput>,
    ) -> Result<CallToolResult, McpError> {
        let pt_path = Path::new(&input.pt_path);
        if !pt_path.exists() {
            return Err(mcp_err(format!(
                ".pt file not found: {}",
                input.pt_path
            )));
        }

        // Determine .pluto path: use provided or default to same name with .pluto extension
        let pluto_path = match input.pluto_path {
            Some(p) => PathBuf::from(p),
            None => {
                let mut path = pt_path.to_path_buf();
                path.set_extension("pluto");
                path
            }
        };

        // Run sync operation
        let result = pluto::sync::sync_pt_to_pluto(&pt_path, &pluto_path).map_err(|e| {
            mcp_internal(format!("Sync failed: {e}"))
        })?;

        // Convert to serializable format
        let result_info = serialize::SyncResultInfo {
            added: result.added,
            removed: result.removed,
            modified: result.modified,
            unchanged: result.unchanged,
        };

        let json = serde_json::to_string_pretty(&result_info)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 25: pretty_print ---
    #[tool(description = "Pretty-print a loaded module or specific declaration as Pluto source text. Optionally include UUID hints.")]
    async fn pretty_print(
        &self,
        Parameters(input): Parameters<PrettyPrintInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;

        let metadata = modules
            .get(&input.path)
            .ok_or_else(|| mcp_err(format!("Module not loaded: {}", input.path)))?;

        let module = &metadata.module;
        let include_uuid_hints = input.include_uuid_hints.unwrap_or(false);

        // If a specific UUID is requested, find and print just that declaration
        if let Some(uuid_str) = input.uuid {
            let target_uuid = uuid::Uuid::parse_str(&uuid_str)
                .map_err(|_| mcp_err(format!("Invalid UUID: {uuid_str}")))?;

            // Search for the declaration with this UUID
            let program = module.program();

            // Check functions
            if let Some(func) = program.functions.iter().find(|f| f.node.id == target_uuid) {
                let text = pluto::pretty::pretty_print_function(&func.node, include_uuid_hints);
                return Ok(CallToolResult::success(vec![Content::text(text)]));
            }

            // Check classes
            if let Some(class) = program.classes.iter().find(|c| c.node.id == target_uuid) {
                let text = pluto::pretty::pretty_print_class(&class.node, include_uuid_hints);
                return Ok(CallToolResult::success(vec![Content::text(text)]));
            }

            // Check enums
            if let Some(enum_) = program.enums.iter().find(|e| e.node.id == target_uuid) {
                let text = pluto::pretty::pretty_print_enum(&enum_.node, include_uuid_hints);
                return Ok(CallToolResult::success(vec![Content::text(text)]));
            }

            // Check traits
            if let Some(trait_) = program.traits.iter().find(|t| t.node.id == target_uuid) {
                let text = pluto::pretty::pretty_print_trait(&trait_.node, include_uuid_hints);
                return Ok(CallToolResult::success(vec![Content::text(text)]));
            }

            // Check errors
            if let Some(error) = program.errors.iter().find(|e| e.node.id == target_uuid) {
                let text = pluto::pretty::pretty_print_error(&error.node, include_uuid_hints);
                return Ok(CallToolResult::success(vec![Content::text(text)]));
            }

            // Check app
            if let Some(app) = &program.app {
                if app.node.id == target_uuid {
                    let text = pluto::pretty::pretty_print_app(&app.node, include_uuid_hints);
                    return Ok(CallToolResult::success(vec![Content::text(text)]));
                }
            }

            return Err(mcp_err(format!(
                "Declaration with UUID {uuid_str} not found in module {}",
                input.path
            )));
        }

        // No specific UUID — print entire module
        let text = pluto::pretty::pretty_print(module.program(), include_uuid_hints);
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }
}

/// Recursively discover .pluto files in a directory, skipping hidden dirs and .git.
/// Deduplicates by directory: if multiple .pluto files exist in the same subdirectory
/// (a module directory), only the first is included (the compiler auto-merges siblings).
fn discover_pluto_files(root: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut files = Vec::new();
    walk_dir(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn walk_dir(
    dir: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), std::io::Error> {
    let entries = std::fs::read_dir(dir)?;
    let mut subdirs = Vec::new();
    let mut pluto_files = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden dirs and .git
        if name_str.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            subdirs.push(path);
        } else if path.extension().and_then(|e| e.to_str()) == Some("pluto") {
            pluto_files.push(path);
        }
    }

    // Add all .pluto files from this directory
    if !pluto_files.is_empty() {
        pluto_files.sort();
        files.extend(pluto_files);
    }

    for subdir in subdirs {
        walk_dir(&subdir, files)?;
    }
    Ok(())
}

// --- Dependency graph helpers ---

/// Derive a module name from a file path relative to the project root.
/// Examples:
///   - `main.pluto` → `main`
///   - `auth/user.pluto` → `auth` (directory module)
///   - `std/strings.pluto` → `std.strings`
fn derive_module_name(rel_path: &Path) -> String {
    let components: Vec<_> = rel_path
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(os_str) => os_str.to_str(),
            _ => None,
        })
        .collect();

    if components.is_empty() {
        return "main".to_string();
    }

    // If the path is just a .pluto file (e.g., `main.pluto`), use the stem
    if components.len() == 1 {
        if let Some(stem) = Path::new(components[0]).file_stem().and_then(|s| s.to_str()) {
            return stem.to_string();
        }
    }

    // For directory modules (e.g., `auth/user.pluto`), use the directory name
    if components.len() >= 2 {
        // Check if all components except the last are directories
        // Join all components except the file name with `.`
        let dir_parts: Vec<_> = components[..components.len() - 1].iter().copied().collect();
        let file_stem = Path::new(components.last().unwrap())
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("main");

        // If the file stem matches the parent directory name, it's a directory module
        if let Some(&last_dir) = dir_parts.last() {
            if file_stem == last_dir {
                return dir_parts.join(".");
            }
        }

        // Otherwise, it's a nested single-file module
        let mut parts = dir_parts;
        parts.push(file_stem);
        return parts.join(".");
    }

    "main".to_string()
}

/// Detect circular imports using DFS.
/// Returns Ok(()) if no cycles, Err(message) if a cycle is found.
fn detect_circular_imports(graph: &DependencyGraph) -> Result<(), String> {
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();

    for path in graph.dependencies.keys() {
        if !visited.contains(path) {
            if let Some(cycle_path) = has_cycle_dfs(path, graph, &mut visited, &mut rec_stack, &mut vec![]) {
                return Err(format!("Circular import: {}", cycle_path.join(" → ")));
            }
        }
    }

    Ok(())
}

/// DFS helper for cycle detection. Returns Some(cycle_path) if a cycle is found.
fn has_cycle_dfs<'a>(
    node: &'a str,
    graph: &'a DependencyGraph,
    visited: &mut HashSet<String>,
    rec_stack: &mut HashSet<String>,
    path: &mut Vec<String>,
) -> Option<Vec<String>> {
    visited.insert(node.to_string());
    rec_stack.insert(node.to_string());
    path.push(
        graph.path_to_name.get(node)
            .unwrap_or(&node.to_string())
            .clone()
    );

    if let Some(deps) = graph.dependencies.get(node) {
        for dep_name in deps {
            // Resolve dep_name to a path
            if let Some(dep_path) = graph.name_to_path.get(dep_name) {
                if rec_stack.contains(dep_path) {
                    // Cycle detected
                    path.push(dep_name.clone());
                    return Some(path.clone());
                }

                if !visited.contains(dep_path) {
                    if let Some(cycle) = has_cycle_dfs(dep_path, graph, visited, rec_stack, path) {
                        return Some(cycle);
                    }
                }
            }
        }
    }

    path.pop();
    rec_stack.remove(node);
    None
}

// --- Write tool helpers ---

/// Find a top-level declaration by name, returning its UUID and kind string.
fn find_decl_by_name(module: &Module, name: &str) -> Result<(Uuid, &'static str), McpError> {
    let matches = module.find(name);
    // Filter to top-level declarations only
    let top_level: Vec<_> = matches.iter()
        .filter(|d| matches!(d.kind(),
            DeclKind::Function | DeclKind::Class | DeclKind::Enum |
            DeclKind::Trait | DeclKind::Error | DeclKind::App))
        .collect();
    match top_level.len() {
        0 => Err(mcp_err(format!("No top-level declaration named '{name}' found"))),
        1 => Ok((top_level[0].id(), serialize::decl_kind_to_string(top_level[0].kind()))),
        _ => Err(mcp_err(format!("Ambiguous name '{name}' ({} top-level matches)", top_level.len()))),
    }
}

/// Find a class by name, returning its UUID.
fn find_class_by_name(module: &Module, name: &str) -> Result<Uuid, McpError> {
    let matches = module.find(name);
    for d in &matches {
        if d.kind() == DeclKind::Class {
            return Ok(d.id());
        }
    }
    Err(mcp_err(format!("No class named '{name}' found")))
}

/// Convert DanglingRefKind to a string label.
fn dangling_ref_kind_str(kind: DanglingRefKind) -> String {
    match kind {
        DanglingRefKind::Call => "call".to_string(),
        DanglingRefKind::StructLit => "struct_lit".to_string(),
        DanglingRefKind::EnumUsage => "enum_usage".to_string(),
        DanglingRefKind::Raise => "raise".to_string(),
        DanglingRefKind::MatchArm => "match_arm".to_string(),
        DanglingRefKind::TypeRef => "type_ref".to_string(),
    }
}

// --- Helper methods ---
impl PlutoMcp {
    /// Resolve a path, creating the file if it doesn't exist.
    fn resolve_or_create_path(&self, path: &str) -> Result<String, McpError> {
        let p = Path::new(path);
        if !p.exists() {
            // Create parent directories if needed
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| mcp_internal(format!("Failed to create directories: {e}")))?;
            }
            std::fs::write(p, "")
                .map_err(|e| mcp_internal(format!("Failed to create file: {e}")))?;
        }
        Ok(canon(path))
    }

    fn find_module<'a>(
        &self,
        modules: &'a HashMap<String, ModuleMetadata>,
        path: &str,
    ) -> Result<&'a ModuleMetadata, McpError> {
        // Try exact path first, then canonicalized
        if let Some(m) = modules.get(path) {
            return Ok(m);
        }
        let canonical = canon(path);
        modules
            .get(&canonical)
            .ok_or_else(|| mcp_err(format!(
                "Module not loaded: '{path}'. Use load_module first."
            )))
    }

    #[async_recursion::async_recursion]
    async fn build_call_graph_recursive(
        &self,
        func_id: Uuid,
        func_name: &str,
        module_path: &str,
        depth: usize,
        max_depth: usize,
        direction: &str,
        modules: &HashMap<String, ModuleMetadata>,
        visited: &mut HashSet<String>,
        nodes: &mut Vec<serialize::CallGraphNode>,
    ) {
        if depth > max_depth {
            return;
        }

        let node_key = format!("{}_{}_{}", func_id, module_path, depth);
        if visited.contains(&node_key) {
            return;
        }
        visited.insert(node_key);

        let mut children = Vec::new();

        if direction == "callees" {
            // Find all functions this function calls
            // We need to examine the function body to find Call expressions
            // For now, we'll use a simplified approach: find all callers_of in reverse
            // This is a limitation - proper callees would require AST traversal
            // For now, return empty children for callees direction
        } else {
            // direction == "callers"
            // Find all functions that call this function
            for (caller_module_path, metadata) in modules.iter() {
                let module = &metadata.module;
                let sites = module.callers_of(func_id);
                for site in sites {
                    let caller_name = &site.caller.name.node;
                    if let Some(caller_decl) = module.find(caller_name).first() {
                        let caller_id = caller_decl.id();
                        let child_key = format!("{}_{}_{}", caller_id, caller_module_path, depth + 1);
                        let is_cycle = visited.contains(&child_key);

                        children.push(serialize::CallGraphChild {
                            uuid: caller_id.to_string(),
                            name: caller_name.clone(),
                            module_path: caller_module_path.clone(),
                            is_cycle: if is_cycle { Some(true) } else { None },
                        });

                        // Recurse if not a cycle
                        if !is_cycle {
                            Box::pin(self.build_call_graph_recursive(
                                caller_id,
                                caller_name,
                                caller_module_path,
                                depth + 1,
                                max_depth,
                                direction,
                                modules,
                                visited,
                                nodes,
                            )).await;
                        }
                    }
                }
            }
        }

        nodes.push(serialize::CallGraphNode {
            uuid: func_id.to_string(),
            name: func_name.to_string(),
            module_path: module_path.to_string(),
            depth,
            children,
        });
    }

    fn inspect_decl(
        &self,
        decl: &pluto_sdk::DeclRef<'_>,
        module: &Module,
    ) -> Result<String, McpError> {
        let json = match decl.kind() {
            DeclKind::Function => {
                let func = decl.as_function().unwrap();
                serde_json::to_string_pretty(&serialize::function_detail(func, module))
            }
            DeclKind::Class => {
                let cls = decl.as_class().unwrap();
                serde_json::to_string_pretty(&serialize::class_detail(cls, module))
            }
            DeclKind::Enum => {
                let en = decl.as_enum().unwrap();
                serde_json::to_string_pretty(&serialize::enum_detail(en, module))
            }
            DeclKind::Trait => {
                let tr = decl.as_trait().unwrap();
                serde_json::to_string_pretty(&serialize::trait_detail(tr, module))
            }
            DeclKind::Error => {
                let err = decl.as_error().unwrap();
                serde_json::to_string_pretty(&serialize::error_decl_detail(err, module))
            }
            DeclKind::App => {
                let app = decl.as_app().unwrap();
                serde_json::to_string_pretty(&serialize::app_detail(app, module))
            }
            other => {
                serde_json::to_string_pretty(&serde_json::json!({
                    "name": decl.name(),
                    "uuid": decl.id().to_string(),
                    "kind": serialize::decl_kind_to_string(other),
                }))
            }
        };
        json.map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))
    }
}

#[tool_handler]
impl ServerHandler for PlutoMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "pluto-mcp".to_string(),
                title: None,
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "Pluto language MCP server. Load a .pluto source file with load_module, or scan a project directory with load_project. Then query declarations, types, error sets, and cross-references. Use add_declaration, replace_declaration, delete_declaration, rename_declaration, add_method, and add_field to edit source files. Use check to type-check, compile to build, run to execute, and test to run tests. Use docs to get language reference documentation and stdlib_docs to explore available stdlib modules and functions.".to_string()
            ),
        }
    }
}
