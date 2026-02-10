use std::collections::HashMap;
use std::sync::Arc;

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

use crate::serialize;
use crate::tools::*;

#[derive(Clone)]
pub struct PlutoMcp {
    modules: Arc<RwLock<HashMap<String, Module>>>,
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

        let module = if first_bytes.len() >= 4 && &first_bytes[..4] == b"PLTO" {
            Module::open(&canonical).map_err(|e| mcp_internal(format!("Failed to load binary: {e}")))?
        } else {
            Module::from_source_file(&canonical)
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

    // --- Tool 3: inspect ---
    #[tool(description = "Deep inspection of a single declaration by UUID or name. Returns params, types, error sets, methods, fields, and pretty-printed source text. If name lookup is ambiguous, returns a disambiguation list.")]
    async fn inspect(
        &self,
        Parameters(input): Parameters<InspectInput>,
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

    // --- Tool 4: xrefs ---
    #[tool(description = "Cross-reference queries: who calls, constructs, uses, or raises a given declaration. Kind must be one of: callers, constructors, enum_usages, raise_sites. Note: callers_of only tracks Expr::Call targets, not method calls via dot syntax.")]
    async fn xrefs(
        &self,
        Parameters(input): Parameters<XrefsInput>,
    ) -> Result<CallToolResult, McpError> {
        let modules = self.modules.read().await;
        let module = self.find_module(&modules, &input.path)?;

        let id = input
            .uuid
            .parse::<Uuid>()
            .map_err(|_| mcp_err(format!("Invalid UUID: {}", input.uuid)))?;

        let sites: Vec<serialize::XrefSiteInfo> = match input.kind.as_str() {
            "callers" => module
                .callers_of(id)
                .iter()
                .map(|site| {
                    let func_id = module.find(&site.caller.name.node)
                        .first()
                        .map(|d| d.id().to_string());
                    serialize::XrefSiteInfo {
                        function_name: site.caller.name.node.clone(),
                        function_uuid: func_id,
                        span: serialize::span_to_info(site.span),
                    }
                })
                .collect(),
            "constructors" => module
                .constructors_of(id)
                .iter()
                .map(|site| {
                    let func_id = module.find(&site.function.name.node)
                        .first()
                        .map(|d| d.id().to_string());
                    serialize::XrefSiteInfo {
                        function_name: site.function.name.node.clone(),
                        function_uuid: func_id,
                        span: serialize::span_to_info(site.span),
                    }
                })
                .collect(),
            "enum_usages" => module
                .enum_usages_of(id)
                .iter()
                .map(|site| {
                    let func_id = module.find(&site.function.name.node)
                        .first()
                        .map(|d| d.id().to_string());
                    serialize::XrefSiteInfo {
                        function_name: site.function.name.node.clone(),
                        function_uuid: func_id,
                        span: serialize::span_to_info(site.span),
                    }
                })
                .collect(),
            "raise_sites" => module
                .raise_sites_of(id)
                .iter()
                .map(|site| {
                    let func_id = module.find(&site.function.name.node)
                        .first()
                        .map(|d| d.id().to_string());
                    serialize::XrefSiteInfo {
                        function_name: site.function.name.node.clone(),
                        function_uuid: func_id,
                        span: serialize::span_to_info(site.span),
                    }
                })
                .collect(),
            other => {
                return Err(mcp_err(format!(
                    "Unknown xref kind '{other}'. Valid: callers, constructors, enum_usages, raise_sites"
                )));
            }
        };

        let json = serde_json::to_string_pretty(&sites)
            .map_err(|e| mcp_internal(format!("JSON serialization failed: {e}")))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // --- Tool 5: errors ---
    #[tool(description = "Get error handling info for a function: whether it is fallible and its error set.")]
    async fn errors(
        &self,
        Parameters(input): Parameters<ErrorsInput>,
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

    // --- Tool 6: source ---
    #[tool(description = "Get source text from a loaded module, optionally at a specific byte range. If no range specified, returns the entire source.")]
    async fn source(
        &self,
        Parameters(input): Parameters<SourceInput>,
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
}

// --- Helper methods ---
impl PlutoMcp {
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
                serde_json::to_string_pretty(&serialize::class_detail(cls))
            }
            DeclKind::Enum => {
                let en = decl.as_enum().unwrap();
                serde_json::to_string_pretty(&serialize::enum_detail(en))
            }
            DeclKind::Trait => {
                let tr = decl.as_trait().unwrap();
                serde_json::to_string_pretty(&serialize::trait_detail(tr))
            }
            DeclKind::Error => {
                let err = decl.as_error().unwrap();
                serde_json::to_string_pretty(&serialize::error_decl_detail(err))
            }
            DeclKind::App => {
                let app = decl.as_app().unwrap();
                serde_json::to_string_pretty(&serialize::app_detail(app))
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
                "Pluto language MCP server. Load a .pluto source file with load_module, then query declarations, types, error sets, and cross-references.".to_string()
            ),
        }
    }
}
