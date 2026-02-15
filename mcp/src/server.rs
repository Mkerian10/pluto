use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

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

use plutoc_sdk::Module;
use plutoc_sdk::decl::DeclKind;
use plutoc_sdk::editor::DanglingRefKind;

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

/// Execute a binary with a timeout, capturing stdout/stderr.
/// Returns (stdout, stderr, exit_code, timed_out).
async fn execute_with_timeout(
    binary: &Path,
    timeout: Duration,
    cwd: Option<&Path>,
) -> Result<(String, String, Option<i32>, bool), McpError> {
    let mut cmd = tokio::process::Command::new(binary);
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let mut child = cmd.spawn()
        .map_err(|e| mcp_internal(format!("Failed to execute binary: {e}")))?;

    match tokio::time::timeout(timeout, child.wait()).await {
        Ok(Ok(status)) => {
            // Process finished within timeout — read captured output
            let mut stdout_str = String::new();
            let mut stderr_str = String::new();
            if let Some(mut out) = child.stdout.take() {
                use tokio::io::AsyncReadExt;
                let _ = out.read_to_string(&mut stdout_str).await;
            }
            if let Some(mut err) = child.stderr.take() {
                use tokio::io::AsyncReadExt;
                let _ = err.read_to_string(&mut stderr_str).await;
            }
            Ok((stdout_str, stderr_str, status.code(), false))
        }
        Ok(Err(e)) => Err(mcp_internal(format!("Failed to wait for process: {e}"))),
        Err(_) => {
            // Timeout — kill the process
            let _ = child.kill().await;
            Ok((String::new(), String::new(), None, true))
        }
    }
}

#[derive(Clone)]
pub struct PlutoMcp {
    modules: Arc<RwLock<HashMap<String, Module>>>,
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

#[tool_router]
impl PlutoMcp {
    pub fn new() -> Self {
        Self {
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
            Module::from_source_file_with_stdlib(&canonical, stdlib_path.as_deref())
                .map_err(|e| mcp_internal(format!("Failed to analyze source: {e}")))?
        };

        // Build summary
        let funcs = module.functions();
        let classes = module.classes();
        let enums = module.enums();
        let traits = module.traits();
        let errors = module.errors();
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

        self.modules.write().await.insert(canonical, module);

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 2: list_declarations ---
    #[tool(description = "List declarations in a loaded module. Optionally filter by kind: function, class, enum, trait, error, app.")]
    async fn list_declarations(
        &self,
        Parameters(input): Parameters<ListDeclarationsInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;
        let module = self.find_module(&modules, &input.path)?;

        let decls: Vec<serialize::DeclSummary> = match input.kind.as_deref() {
            Some("function") => module.functions().iter().map(serialize::decl_to_summary).collect(),
            Some("class") => module.classes().iter().map(serialize::decl_to_summary).collect(),
            Some("enum") => module.enums().iter().map(serialize::decl_to_summary).collect(),
            Some("trait") => module.traits().iter().map(serialize::decl_to_summary).collect(),
            Some("error") => module.errors().iter().map(serialize::decl_to_summary).collect(),
            Some("app") => module.app().iter().map(serialize::decl_to_summary).collect(),
            None => {
                let mut all = Vec::new();
                for d in module.functions() { all.push(serialize::decl_to_summary(&d)); }
                for d in module.classes() { all.push(serialize::decl_to_summary(&d)); }
                for d in module.enums() { all.push(serialize::decl_to_summary(&d)); }
                for d in module.traits() { all.push(serialize::decl_to_summary(&d)); }
                for d in module.errors() { all.push(serialize::decl_to_summary(&d)); }
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
        let module = self.find_module(&modules, &input.path)?;

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
        for (module_path, module) in modules.iter() {
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
        for (module_path, module) in modules.iter() {
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
        for (module_path, module) in modules.iter() {
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
        for (module_path, module) in modules.iter() {
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
        for (module_path, module) in modules.iter() {
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

    // --- Tool 5: error_set ---
    #[tool(description = "Get error handling info for a function: whether it is fallible and its error set.")]
    async fn error_set(
        &self,
        Parameters(input): Parameters<ErrorSetInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;
        let module = self.find_module(&modules, &input.path)?;

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
        let module = self.find_module(&modules, &input.path)?;

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
        let mut modules_loaded = Vec::new();
        let mut load_errors = Vec::new();

        let mut modules = self.modules.write().await;

        for file in &files {
            let canonical = canon(&file.to_string_lossy());
            match Module::from_source_file_with_stdlib(&canonical, stdlib_path.as_deref()) {
                Ok(module) => {
                    let decl_count = module.functions().len()
                        + module.classes().len()
                        + module.enums().len()
                        + module.traits().len()
                        + module.errors().len()
                        + if module.app().is_some() { 1 } else { 0 };
                    modules_loaded.push(serialize::ModuleBrief {
                        path: canonical.clone(),
                        declarations: decl_count,
                    });
                    modules.insert(canonical, module);
                }
                Err(e) => {
                    load_errors.push(serialize::LoadError {
                        path: canonical,
                        error: e.to_string(),
                    });
                }
            }
        }

        *self.project_root.write().await = Some(root.clone());

        // Build dependency graph from loaded modules
        let mut dep_graph = self.dep_graph.write().await;
        dep_graph.name_to_path.clear();
        dep_graph.path_to_name.clear();
        dep_graph.dependencies.clear();

        // Build module name → path mapping
        // For each .pluto file, derive the module name from its path relative to project root
        for (path, _module) in modules.iter() {
            if let Ok(rel_path) = Path::new(path).strip_prefix(&root) {
                let module_name = derive_module_name(rel_path);
                dep_graph.name_to_path.insert(module_name.clone(), path.clone());
                dep_graph.path_to_name.insert(path.clone(), module_name);
            }
        }

        // Extract imports from each module and populate dependencies
        for (path, module) in modules.iter() {
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
            .map(|(path, module)| {
                let funcs = module.functions().len();
                let classes = module.classes().len();
                let enums = module.enums().len();
                let traits = module.traits().len();
                let errors = module.errors().len();
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

        for (path, module) in modules.iter() {
            let matches = module.find(&input.name);
            for decl in matches {
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
        let entry_path = Path::new(&canonical);
        let stdlib_path = input.stdlib.as_ref().map(|s| PathBuf::from(s));

        // Read source for line:col enrichment
        let source_text = std::fs::read_to_string(&canonical).ok();

        let result = plutoc::analyze_file_with_warnings(
            entry_path,
            stdlib_path.as_deref(),
        );

        let check_result = match result {
            Ok((_program, _source, _derived, warnings)) => {
                serialize::CheckResult {
                    success: true,
                    path: canonical,
                    errors: vec![],
                    warnings: warnings.iter().map(|w| serialize::compile_warning_to_diagnostic(w, source_text.as_deref())).collect(),
                }
            }
            Err(err) => {
                serialize::CheckResult {
                    success: false,
                    path: canonical,
                    errors: vec![serialize::compile_error_to_diagnostic(&err, source_text.as_deref())],
                    warnings: vec![],
                }
            }
        };

        let json = serde_json::to_string_pretty(&check_result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 11: compile ---
    #[tool(description = "Compile a .pluto source file to a native binary. Returns the output path on success or structured error diagnostics (with line:col) on failure.")]
    async fn compile(
        &self,
        Parameters(input): Parameters<CompileInput>,
    ) -> Result<CallToolResult, McpError> {
        let canonical = canon(&input.path);
        let entry_path = Path::new(&canonical);
        let stdlib_path = input.stdlib.as_ref().map(|s| PathBuf::from(s));

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

        // Read source for line:col enrichment
        let source_text = std::fs::read_to_string(&canonical).ok();

        let result = plutoc::compile_file_with_stdlib(
            entry_path,
            &output_path,
            stdlib_path.as_deref(),
        );

        let compile_result = match result {
            Ok(()) => {
                serialize::CompileResult {
                    success: true,
                    path: canonical,
                    output: Some(output_path.display().to_string()),
                    errors: vec![],
                }
            }
            Err(err) => {
                serialize::CompileResult {
                    success: false,
                    path: canonical,
                    output: None,
                    errors: vec![serialize::compile_error_to_diagnostic(&err, source_text.as_deref())],
                }
            }
        };

        let json = serde_json::to_string_pretty(&compile_result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 12: run ---
    #[tool(description = "Compile and execute a .pluto source file, capturing stdout/stderr. Default timeout: 10s, max: 60s. Returns compilation errors or execution results (stdout, stderr, exit code, timed_out).")]
    async fn run(
        &self,
        Parameters(input): Parameters<RunInput>,
    ) -> Result<CallToolResult, McpError> {
        let canonical = canon(&input.path);
        let entry_path = Path::new(&canonical);
        let stdlib_path = input.stdlib.as_ref().map(|s| PathBuf::from(s));
        let timeout_ms = input.timeout_ms.unwrap_or(10_000).min(60_000);
        let timeout = Duration::from_millis(timeout_ms);

        // Determine working directory: explicit cwd > source file's parent
        let cwd = input.cwd.as_ref()
            .map(|c| PathBuf::from(c))
            .or_else(|| entry_path.parent().map(|p| p.to_path_buf()));

        // Read source for line:col enrichment
        let source_text = std::fs::read_to_string(&canonical).ok();

        // Compile to temp binary
        let tmp_dir = tempfile::tempdir()
            .map_err(|e| mcp_internal(format!("Failed to create temp dir: {e}")))?;
        let binary_path = tmp_dir.path().join(format!("pluto_{}", uuid::Uuid::new_v4()));

        let compile_result = plutoc::compile_file_with_stdlib(
            entry_path,
            &binary_path,
            stdlib_path.as_deref(),
        );

        if let Err(err) = compile_result {
            let run_result = serialize::RunResult {
                success: false,
                path: canonical,
                compilation_errors: vec![serialize::compile_error_to_diagnostic(&err, source_text.as_deref())],
                stdout: None,
                stderr: None,
                exit_code: None,
                timed_out: false,
            };
            let json = serde_json::to_string_pretty(&run_result)
                .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
            return Ok(CallToolResult::success(vec![Content::text(json)]));
        }

        // Execute
        let (stdout, stderr, exit_code, timed_out) =
            execute_with_timeout(&binary_path, timeout, cwd.as_deref()).await?;

        let run_result = serialize::RunResult {
            success: !timed_out && exit_code == Some(0),
            path: canonical,
            compilation_errors: vec![],
            stdout: Some(stdout),
            stderr: Some(stderr),
            exit_code,
            timed_out,
        };

        let json = serde_json::to_string_pretty(&run_result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
        // tmp_dir dropped here, cleans up binary
    }

    // --- Tool 13: test ---
    #[tool(description = "Compile a .pluto source file in test mode and execute the test runner, capturing stdout/stderr. Default timeout: 30s, max: 60s. Returns compilation errors or test execution results.")]
    async fn test(
        &self,
        Parameters(input): Parameters<TestInput>,
    ) -> Result<CallToolResult, McpError> {
        let canonical = canon(&input.path);
        let entry_path = Path::new(&canonical);
        let stdlib_path = input.stdlib.as_ref().map(|s| PathBuf::from(s));
        let timeout_ms = input.timeout_ms.unwrap_or(30_000).min(60_000);
        let timeout = Duration::from_millis(timeout_ms);

        // Determine working directory: explicit cwd > source file's parent
        let cwd = input.cwd.as_ref()
            .map(|c| PathBuf::from(c))
            .or_else(|| entry_path.parent().map(|p| p.to_path_buf()));

        // Read source for line:col enrichment
        let source_text = std::fs::read_to_string(&canonical).ok();

        // Compile in test mode to temp binary
        let tmp_dir = tempfile::tempdir()
            .map_err(|e| mcp_internal(format!("Failed to create temp dir: {e}")))?;
        let binary_path = tmp_dir.path().join(format!("pluto_test_{}", uuid::Uuid::new_v4()));

        let compile_result = plutoc::compile_file_for_tests(
            entry_path,
            &binary_path,
            stdlib_path.as_deref(),
            false,
        );

        if let Err(err) = compile_result {
            let test_result = serialize::TestResult {
                success: false,
                path: canonical,
                compilation_errors: vec![serialize::compile_error_to_diagnostic(&err, source_text.as_deref())],
                stdout: None,
                stderr: None,
                exit_code: None,
                timed_out: false,
            };
            let json = serde_json::to_string_pretty(&test_result)
                .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
            return Ok(CallToolResult::success(vec![Content::text(json)]));
        }

        // Execute test runner
        let (stdout, stderr, exit_code, timed_out) =
            execute_with_timeout(&binary_path, timeout, cwd.as_deref()).await?;

        let test_result = serialize::TestResult {
            success: !timed_out && exit_code == Some(0),
            path: canonical,
            compilation_errors: vec![],
            stdout: Some(stdout),
            stderr: Some(stderr),
            exit_code,
            timed_out,
        };

        let json = serde_json::to_string_pretty(&test_result)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
        // tmp_dir dropped here, cleans up binary
    }

    // --- Tool 14: add_declaration ---
    #[tool(description = "Add a new top-level declaration to a .pluto source file. The file is created if it doesn't exist. Returns the UUID, name, and kind of the added declaration. Supports multiple declarations in a single source string — all will be added and their details returned as an array.")]
    async fn add_declaration(
        &self,
        Parameters(input): Parameters<AddDeclarationInput>,
    ) -> Result<CallToolResult, McpError> {
        let canonical = self.resolve_or_create_path(&input.path)?;

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

        self.modules.write().await.insert(canonical, module);

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
        let canonical = canon(&input.path);

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

        self.modules.write().await.insert(canonical, module);

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
        let canonical = canon(&input.path);

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

        self.modules.write().await.insert(canonical, module);

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
        let canonical = canon(&input.path);

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

        self.modules.write().await.insert(canonical, module);

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
        let canonical = canon(&input.path);

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

        self.modules.write().await.insert(canonical, module);

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
        let canonical = canon(&input.path);

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

        self.modules.write().await.insert(canonical, module);

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
        let text = crate::docs::get_docs(input.topic.as_deref());
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    // --- Tool 21: stdlib_docs ---
    #[tool(description = "Get Pluto stdlib documentation. Without a module name, lists all available stdlib modules. With a module name (e.g. 'strings', 'fs', 'math'), returns all pub function signatures and descriptions from that module.")]
    async fn stdlib_docs(
        &self,
        Parameters(input): Parameters<StdlibDocsInput>,
    ) -> Result<CallToolResult, McpError> {
        let text = crate::docs::get_stdlib_docs(input.module.as_deref())
            .map_err(|e| mcp_err(e))?;
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }
}

/// Recursively discover .pluto files in a directory, skipping hidden dirs and .git.
/// Deduplicates by directory: if multiple .pluto files exist in the same subdirectory
/// (a module directory), only the first is included (the compiler auto-merges siblings).
fn discover_pluto_files(root: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut files = Vec::new();
    let mut seen_dirs = std::collections::HashSet::new();
    walk_dir(root, &mut files, &mut seen_dirs)?;
    files.sort();
    Ok(files)
}

fn walk_dir(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    seen_dirs: &mut std::collections::HashSet<PathBuf>,
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

    // For .pluto files in this directory: if the parent is already tracked
    // as a module directory (by a sibling), skip duplicates
    if !pluto_files.is_empty() {
        let parent = dir.to_path_buf();
        if seen_dirs.insert(parent) {
            // First time seeing this dir — take the first .pluto file
            pluto_files.sort();
            files.push(pluto_files[0].clone());
        }
        // else: already loaded a file from this dir, skip all
    }

    for subdir in subdirs {
        walk_dir(&subdir, files, seen_dirs)?;
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
        modules: &'a HashMap<String, Module>,
        path: &str,
    ) -> Result<&'a Module, McpError> {
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

    fn inspect_decl(
        &self,
        decl: &plutoc_sdk::DeclRef<'_>,
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
