//! In-process compiler service implementation.
//!
//! This module provides `InProcessServer`, which implements the `CompilerService` trait
//! by calling compiler library functions directly. It maintains a module cache so that
//! loaded modules can be queried for declarations, cross-references, and more.

use super::types::*;
use super::CompilerService;
use crate::derived::DerivedInfo;
use crate::parser::ast::{
    AppDecl, ClassDecl, EnumDecl, ErrorDecl, Expr, Function, Program, Stmt, TraitDecl,
};
use crate::span::Spanned;
use crate::visit::{walk_expr, walk_stmt, Visitor};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use uuid::Uuid;

// ========== CachedModule ==========

/// A loaded and analyzed module cached in memory.
struct CachedModule {
    program: Program,
    source: String,
    derived: DerivedInfo,
    loaded_at: SystemTime,
    name: String,
}

// ========== InProcessServer ==========

/// In-process compiler service.
///
/// Calls compiler library functions directly and caches loaded modules for
/// subsequent queries (declarations, cross-references, etc.).
pub struct InProcessServer {
    modules: HashMap<PathBuf, CachedModule>,
}

impl InProcessServer {
    /// Create a new in-process server.
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }

    /// Get the cached module for a path, or return an error.
    fn get_module(&self, path: &Path) -> Result<&CachedModule, ServiceError> {
        // Try canonical path first, then as-is
        let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        self.modules
            .get(&canon)
            .or_else(|| self.modules.get(path))
            .ok_or_else(|| ServiceError::ModuleNotFound(path.to_path_buf()))
    }

    /// Canonicalize a path, falling back to the original if canonicalization fails.
    fn canon(path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    /// Build a ModuleSummary from a cached module.
    fn build_summary(path: &Path, cached: &CachedModule) -> ModuleSummary {
        ModuleSummary {
            path: path.to_path_buf(),
            name: cached.name.clone(),
            function_count: cached.program.functions.len(),
            class_count: cached.program.classes.len(),
            enum_count: cached.program.enums.len(),
            trait_count: cached.program.traits.len(),
            error_count: cached.program.errors.len(),
            app_count: if cached.program.app.is_some() { 1 } else { 0 },
        }
    }

    /// Build DeclSummary entries from a Program.
    fn collect_decl_summaries(program: &Program, filter: Option<DeclKind>) -> Vec<DeclSummary> {
        let mut results = Vec::new();

        if filter.is_none() || filter == Some(DeclKind::Function) {
            for f in &program.functions {
                results.push(DeclSummary {
                    uuid: f.node.id,
                    name: f.node.name.node.clone(),
                    kind: DeclKind::Function,
                });
            }
        }

        if filter.is_none() || filter == Some(DeclKind::Class) {
            for c in &program.classes {
                results.push(DeclSummary {
                    uuid: c.node.id,
                    name: c.node.name.node.clone(),
                    kind: DeclKind::Class,
                });
            }
        }

        if filter.is_none() || filter == Some(DeclKind::Enum) {
            for e in &program.enums {
                results.push(DeclSummary {
                    uuid: e.node.id,
                    name: e.node.name.node.clone(),
                    kind: DeclKind::Enum,
                });
            }
        }

        if filter.is_none() || filter == Some(DeclKind::Trait) {
            for t in &program.traits {
                results.push(DeclSummary {
                    uuid: t.node.id,
                    name: t.node.name.node.clone(),
                    kind: DeclKind::Trait,
                });
            }
        }

        if filter.is_none() || filter == Some(DeclKind::Error) {
            for e in &program.errors {
                results.push(DeclSummary {
                    uuid: e.node.id,
                    name: e.node.name.node.clone(),
                    kind: DeclKind::Error,
                });
            }
        }

        if filter.is_none() || filter == Some(DeclKind::App) {
            if let Some(app) = &program.app {
                results.push(DeclSummary {
                    uuid: app.node.id,
                    name: app.node.name.node.clone(),
                    kind: DeclKind::App,
                });
            }
        }

        results
    }

    /// Build a FunctionDetail from a Function AST node and DerivedInfo.
    fn build_function_detail(
        func: &Function,
        derived: &DerivedInfo,
        include_uuids: bool,
    ) -> FunctionDetail {
        let sig = derived.fn_signatures.get(&func.id);
        let error_set = derived.fn_error_sets.get(&func.id);

        let params = func
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let type_name = sig
                    .and_then(|s| s.param_types.get(i))
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                ParamInfo {
                    name: p.name.node.clone(),
                    type_name,
                }
            })
            .collect();

        let return_type = sig
            .map(|s| s.return_type.to_string())
            .unwrap_or_else(|| "void".to_string());

        let is_fallible = sig.map(|s| s.is_fallible).unwrap_or(false);

        let errors = error_set
            .map(|es| es.iter().map(|e| e.name.clone()).collect())
            .unwrap_or_default();

        FunctionDetail {
            uuid: func.id,
            name: func.name.node.clone(),
            params,
            return_type,
            is_fallible,
            error_set: errors,
            source: crate::pretty::pretty_print_function(func, include_uuids),
        }
    }

    /// Build a ClassDetail from a ClassDecl AST node and DerivedInfo.
    fn build_class_detail(
        cls: &ClassDecl,
        derived: &DerivedInfo,
        include_uuids: bool,
    ) -> ClassDetail {
        let class_info = derived.class_infos.get(&cls.id);

        let fields = if let Some(info) = class_info {
            info.fields
                .iter()
                .map(|f| FieldInfo {
                    name: f.name.clone(),
                    type_name: f.ty.to_string(),
                })
                .collect()
        } else {
            cls.fields
                .iter()
                .map(|f| FieldInfo {
                    name: f.name.node.clone(),
                    type_name: format!("{:?}", f.ty.node),
                })
                .collect()
        };

        let methods = if let Some(info) = class_info {
            info.methods
                .iter()
                .zip(cls.methods.iter())
                .map(|((name, sig), m)| MethodInfo {
                    uuid: m.node.id,
                    name: name.clone(),
                    params: sig
                        .param_types
                        .iter()
                        .skip(1) // skip self param
                        .zip(m.node.params.iter().skip(1))
                        .map(|(ty, p)| ParamInfo {
                            name: p.name.node.clone(),
                            type_name: ty.to_string(),
                        })
                        .collect(),
                    return_type: sig.return_type.to_string(),
                })
                .collect()
        } else {
            cls.methods
                .iter()
                .map(|m| MethodInfo {
                    uuid: m.node.id,
                    name: m.node.name.node.clone(),
                    params: m
                        .node
                        .params
                        .iter()
                        .skip(1) // skip self
                        .map(|p| ParamInfo {
                            name: p.name.node.clone(),
                            type_name: format!("{:?}", p.ty.node),
                        })
                        .collect(),
                    return_type: m
                        .node
                        .return_type
                        .as_ref()
                        .map(|t| format!("{:?}", t.node))
                        .unwrap_or_else(|| "void".to_string()),
                })
                .collect()
        };

        ClassDetail {
            uuid: cls.id,
            name: cls.name.node.clone(),
            fields,
            methods,
            source: crate::pretty::pretty_print_class(cls, include_uuids),
        }
    }

    /// Build an EnumDetail from an EnumDecl AST node and DerivedInfo.
    fn build_enum_detail(
        en: &EnumDecl,
        derived: &DerivedInfo,
        include_uuids: bool,
    ) -> EnumDetail {
        let enum_info = derived.enum_infos.get(&en.id);

        let variants = if let Some(info) = enum_info {
            info.variants
                .iter()
                .map(|v| VariantInfo {
                    name: v.name.clone(),
                    fields: v
                        .fields
                        .iter()
                        .map(|f| FieldInfo {
                            name: f.name.clone(),
                            type_name: f.ty.to_string(),
                        })
                        .collect(),
                })
                .collect()
        } else {
            en.variants
                .iter()
                .map(|v| VariantInfo {
                    name: v.name.node.clone(),
                    fields: v
                        .fields
                        .iter()
                        .map(|f| FieldInfo {
                            name: f.name.node.clone(),
                            type_name: format!("{:?}", f.ty.node),
                        })
                        .collect(),
                })
                .collect()
        };

        EnumDetail {
            uuid: en.id,
            name: en.name.node.clone(),
            variants,
            source: crate::pretty::pretty_print_enum(en, include_uuids),
        }
    }

    /// Build a TraitDetail from a TraitDecl AST node and DerivedInfo.
    fn build_trait_detail(
        tr: &TraitDecl,
        derived: &DerivedInfo,
        include_uuids: bool,
    ) -> TraitDetail {
        let trait_info = derived.trait_infos.get(&tr.id);

        let methods = if let Some(info) = trait_info {
            info.methods
                .iter()
                .map(|(name, sig)| MethodSignature {
                    name: name.clone(),
                    params: sig
                        .param_types
                        .iter()
                        .zip(
                            tr.methods
                                .iter()
                                .find(|m| &m.name.node == name)
                                .map(|m| m.params.iter().collect::<Vec<_>>())
                                .unwrap_or_default(),
                        )
                        .map(|(ty, p)| ParamInfo {
                            name: p.name.node.clone(),
                            type_name: ty.to_string(),
                        })
                        .collect(),
                    return_type: sig.return_type.to_string(),
                })
                .collect()
        } else {
            tr.methods
                .iter()
                .map(|m| MethodSignature {
                    name: m.name.node.clone(),
                    params: m
                        .params
                        .iter()
                        .map(|p| ParamInfo {
                            name: p.name.node.clone(),
                            type_name: format!("{:?}", p.ty.node),
                        })
                        .collect(),
                    return_type: m
                        .return_type
                        .as_ref()
                        .map(|t| format!("{:?}", t.node))
                        .unwrap_or_else(|| "void".to_string()),
                })
                .collect()
        };

        TraitDetail {
            uuid: tr.id,
            name: tr.name.node.clone(),
            methods,
            source: crate::pretty::pretty_print_trait(tr, include_uuids),
        }
    }

    /// Build an ErrorDetail from an ErrorDecl AST node.
    fn build_error_detail(err: &ErrorDecl, include_uuids: bool) -> ErrorDetail {
        ErrorDetail {
            uuid: err.id,
            name: err.name.node.clone(),
            source: crate::pretty::pretty_print_error(err, include_uuids),
        }
    }

    /// Build an AppDetail from an AppDecl AST node.
    fn build_app_detail(app: &AppDecl, include_uuids: bool) -> AppDetail {
        AppDetail {
            uuid: app.id,
            name: app.name.node.clone(),
            deps: app
                .inject_fields
                .iter()
                .map(|f| FieldInfo {
                    name: f.name.node.clone(),
                    type_name: format!("{:?}", f.ty.node),
                })
                .collect(),
            methods: app
                .methods
                .iter()
                .map(|m| MethodInfo {
                    uuid: m.node.id,
                    name: m.node.name.node.clone(),
                    params: m
                        .node
                        .params
                        .iter()
                        .skip(1) // skip self
                        .map(|p| ParamInfo {
                            name: p.name.node.clone(),
                            type_name: format!("{:?}", p.ty.node),
                        })
                        .collect(),
                    return_type: m
                        .node
                        .return_type
                        .as_ref()
                        .map(|t| format!("{:?}", t.node))
                        .unwrap_or_else(|| "void".to_string()),
                })
                .collect(),
            source: crate::pretty::pretty_print_app(app, include_uuids),
        }
    }
}

impl Default for InProcessServer {
    fn default() -> Self {
        Self::new()
    }
}

// ========== XrefCollector ==========

/// Visitor that collects cross-reference sites matching a target UUID.
struct XrefCollector {
    target_id: Uuid,
    /// Which kinds of references to look for.
    look_for_calls: bool,
    look_for_structs: bool,
    look_for_enums: bool,
    look_for_raises: bool,
    /// Collected sites with (span_start, span_end).
    sites: Vec<(usize, usize)>,
}

impl XrefCollector {
    fn for_calls(id: Uuid) -> Self {
        Self {
            target_id: id,
            look_for_calls: true,
            look_for_structs: false,
            look_for_enums: false,
            look_for_raises: false,
            sites: Vec::new(),
        }
    }

    fn for_structs(id: Uuid) -> Self {
        Self {
            target_id: id,
            look_for_calls: false,
            look_for_structs: true,
            look_for_enums: false,
            look_for_raises: false,
            sites: Vec::new(),
        }
    }

    fn for_enums(id: Uuid) -> Self {
        Self {
            target_id: id,
            look_for_calls: false,
            look_for_structs: false,
            look_for_enums: true,
            look_for_raises: false,
            sites: Vec::new(),
        }
    }

    fn for_raises(id: Uuid) -> Self {
        Self {
            target_id: id,
            look_for_calls: false,
            look_for_structs: false,
            look_for_enums: false,
            look_for_raises: true,
            sites: Vec::new(),
        }
    }

    fn for_all(id: Uuid) -> Self {
        Self {
            target_id: id,
            look_for_calls: true,
            look_for_structs: true,
            look_for_enums: true,
            look_for_raises: true,
            sites: Vec::new(),
        }
    }

    fn into_xref_sites(self, module_path: &Path, source: &str) -> Vec<XrefSite> {
        self.sites
            .into_iter()
            .map(|(start, end)| XrefSite {
                module_path: module_path.to_path_buf(),
                span: DiagnosticSpan::from_offset(start, end, source),
                context: None,
            })
            .collect()
    }
}

impl Visitor for XrefCollector {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        match &expr.node {
            Expr::Call { target_id, .. } if self.look_for_calls => {
                if *target_id == Some(self.target_id) {
                    self.sites.push((expr.span.start, expr.span.end));
                }
            }
            Expr::StructLit { target_id, .. } if self.look_for_structs => {
                if *target_id == Some(self.target_id) {
                    self.sites.push((expr.span.start, expr.span.end));
                }
            }
            Expr::EnumUnit {
                enum_id,
                variant_id,
                ..
            }
            | Expr::EnumData {
                enum_id,
                variant_id,
                ..
            } if self.look_for_enums => {
                if *enum_id == Some(self.target_id) || *variant_id == Some(self.target_id) {
                    self.sites.push((expr.span.start, expr.span.end));
                }
            }
            _ => {}
        }
        // Always recurse into children
        walk_expr(self, expr);
    }

    fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
        match &stmt.node {
            Stmt::Raise { error_id, .. } if self.look_for_raises => {
                if *error_id == Some(self.target_id) {
                    self.sites.push((stmt.span.start, stmt.span.end));
                }
            }
            Stmt::Match { arms, .. } if self.look_for_enums => {
                for arm in arms {
                    if arm.enum_id == Some(self.target_id)
                        || arm.variant_id == Some(self.target_id)
                    {
                        self.sites
                            .push((arm.enum_name.span.start, arm.variant_name.span.end));
                    }
                }
            }
            _ => {}
        }
        // Always recurse into children
        walk_stmt(self, stmt);
    }
}

/// Helper to run an XrefCollector across a program.
fn collect_xrefs(program: &Program, collector: &mut XrefCollector) {
    // Walk all top-level functions
    for func in &program.functions {
        collector.visit_block(&func.node.body);
    }
    // Walk all class methods
    for cls in &program.classes {
        for method in &cls.node.methods {
            collector.visit_block(&method.node.body);
        }
    }
    // Walk app methods
    if let Some(app) = &program.app {
        for method in &app.node.methods {
            collector.visit_block(&method.node.body);
        }
    }
    // Walk stage methods
    for stage in &program.stages {
        for method in &stage.node.methods {
            collector.visit_block(&method.node.body);
        }
    }
    // Walk trait default method bodies
    for tr in &program.traits {
        for method in &tr.node.methods {
            if let Some(body) = &method.body {
                collector.visit_block(body);
            }
        }
    }
}

// ========== DiagnosticSpan helper ==========

impl DiagnosticSpan {
    /// Create a DiagnosticSpan from byte offsets and source text.
    fn from_offset(start: usize, end: usize, source: &str) -> Self {
        let line = source[..start.min(source.len())]
            .chars()
            .filter(|c| *c == '\n')
            .count()
            + 1;
        let col_start = source[..start.min(source.len())]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let column = source[col_start..start.min(source.len())].chars().count() + 1;

        Self {
            start,
            end,
            line: Some(line),
            column: Some(column),
        }
    }
}

// ========== CompilerService Implementation ==========

impl CompilerService for InProcessServer {
    // ===== Module Management =====

    fn load_module(
        &mut self,
        path: &Path,
        opts: &LoadOptions,
    ) -> Result<ModuleSummary, ServiceError> {
        if !path.exists() {
            return Err(ServiceError::ModuleNotFound(path.to_path_buf()));
        }

        let canon = Self::canon(path);

        match crate::analyze_file(&canon, opts.stdlib.as_deref()) {
            Ok((program, source, derived)) => {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let cached = CachedModule {
                    program,
                    source,
                    derived,
                    loaded_at: SystemTime::now(),
                    name,
                };

                let summary = Self::build_summary(&canon, &cached);
                self.modules.insert(canon, cached);
                Ok(summary)
            }
            Err(e) => Err(ServiceError::from(e)),
        }
    }

    fn load_project(
        &mut self,
        root: &Path,
        opts: &LoadOptions,
    ) -> Result<ProjectSummary, ServiceError> {
        if !root.is_dir() {
            return Err(ServiceError::InvalidPath(format!(
                "{} is not a directory",
                root.display()
            )));
        }

        let mut loaded = Vec::new();
        let mut failed = Vec::new();

        fn visit_dirs(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
            if dir.is_dir() {
                for entry in std::fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if !name.starts_with('.') {
                            visit_dirs(&path, files)?;
                        }
                    } else if path.extension().and_then(|e| e.to_str()) == Some("pluto") {
                        files.push(path);
                    }
                }
            }
            Ok(())
        }

        let mut pluto_files = Vec::new();
        visit_dirs(root, &mut pluto_files).map_err(ServiceError::Io)?;

        for file in pluto_files {
            match self.load_module(&file, opts) {
                Ok(_) => loaded.push(file),
                Err(e) => failed.push((file, e.to_string())),
            }
        }

        Ok(ProjectSummary {
            root: root.to_path_buf(),
            loaded,
            failed,
        })
    }

    fn list_modules(&self) -> Vec<ModuleInfo> {
        self.modules
            .iter()
            .map(|(path, cached)| ModuleInfo {
                path: path.clone(),
                name: cached.name.clone(),
                loaded_at: cached.loaded_at,
            })
            .collect()
    }

    fn reload_module(
        &mut self,
        path: &Path,
        opts: &LoadOptions,
    ) -> Result<ModuleSummary, ServiceError> {
        // Remove the old cached version (if any) and reload
        let canon = Self::canon(path);
        self.modules.remove(&canon);
        self.load_module(path, opts)
    }

    fn module_status(&self) -> Vec<ModuleStatus> {
        self.modules
            .iter()
            .map(|(path, cached)| {
                let is_stale = std::fs::metadata(path)
                    .and_then(|m| m.modified())
                    .map(|mtime| mtime > cached.loaded_at)
                    .unwrap_or(false);

                ModuleStatus {
                    path: path.clone(),
                    name: cached.name.clone(),
                    loaded_at: cached.loaded_at,
                    is_stale,
                }
            })
            .collect()
    }

    // ===== Declaration Inspection =====

    fn list_declarations(
        &self,
        path: &Path,
        filter: Option<DeclKind>,
    ) -> Result<Vec<DeclSummary>, ServiceError> {
        let cached = self.get_module(path)?;
        Ok(Self::collect_decl_summaries(&cached.program, filter))
    }

    fn get_declaration(&self, path: &Path, id: Uuid) -> Result<DeclDetail, ServiceError> {
        let cached = self.get_module(path)?;
        let program = &cached.program;
        let derived = &cached.derived;

        // Search functions
        if let Some(f) = program.functions.iter().find(|f| f.node.id == id) {
            return Ok(DeclDetail::Function(Self::build_function_detail(
                &f.node, derived, false,
            )));
        }

        // Search classes
        if let Some(c) = program.classes.iter().find(|c| c.node.id == id) {
            return Ok(DeclDetail::Class(Self::build_class_detail(
                &c.node, derived, false,
            )));
        }

        // Search enums
        if let Some(e) = program.enums.iter().find(|e| e.node.id == id) {
            return Ok(DeclDetail::Enum(Self::build_enum_detail(
                &e.node, derived, false,
            )));
        }

        // Search traits
        if let Some(t) = program.traits.iter().find(|t| t.node.id == id) {
            return Ok(DeclDetail::Trait(Self::build_trait_detail(
                &t.node, derived, false,
            )));
        }

        // Search errors
        if let Some(e) = program.errors.iter().find(|e| e.node.id == id) {
            return Ok(DeclDetail::Error(Self::build_error_detail(&e.node, false)));
        }

        // Search app
        if let Some(app) = &program.app {
            if app.node.id == id {
                return Ok(DeclDetail::App(Self::build_app_detail(&app.node, false)));
            }
        }

        Err(ServiceError::DeclarationNotFound(id))
    }

    fn find_declaration(&self, name: &str, filter: Option<DeclKind>) -> Vec<DeclMatch> {
        let mut results = Vec::new();

        for (path, cached) in &self.modules {
            let program = &cached.program;

            if filter.is_none() || filter == Some(DeclKind::Function) {
                for f in &program.functions {
                    if f.node.name.node == name {
                        results.push(DeclMatch {
                            uuid: f.node.id,
                            name: f.node.name.node.clone(),
                            kind: DeclKind::Function,
                            module_path: path.clone(),
                        });
                    }
                }
            }

            if filter.is_none() || filter == Some(DeclKind::Class) {
                for c in &program.classes {
                    if c.node.name.node == name {
                        results.push(DeclMatch {
                            uuid: c.node.id,
                            name: c.node.name.node.clone(),
                            kind: DeclKind::Class,
                            module_path: path.clone(),
                        });
                    }
                }
            }

            if filter.is_none() || filter == Some(DeclKind::Enum) {
                for e in &program.enums {
                    if e.node.name.node == name {
                        results.push(DeclMatch {
                            uuid: e.node.id,
                            name: e.node.name.node.clone(),
                            kind: DeclKind::Enum,
                            module_path: path.clone(),
                        });
                    }
                }
            }

            if filter.is_none() || filter == Some(DeclKind::Trait) {
                for t in &program.traits {
                    if t.node.name.node == name {
                        results.push(DeclMatch {
                            uuid: t.node.id,
                            name: t.node.name.node.clone(),
                            kind: DeclKind::Trait,
                            module_path: path.clone(),
                        });
                    }
                }
            }

            if filter.is_none() || filter == Some(DeclKind::Error) {
                for e in &program.errors {
                    if e.node.name.node == name {
                        results.push(DeclMatch {
                            uuid: e.node.id,
                            name: e.node.name.node.clone(),
                            kind: DeclKind::Error,
                            module_path: path.clone(),
                        });
                    }
                }
            }

            if filter.is_none() || filter == Some(DeclKind::App) {
                if let Some(app) = &program.app {
                    if app.node.name.node == name {
                        results.push(DeclMatch {
                            uuid: app.node.id,
                            name: app.node.name.node.clone(),
                            kind: DeclKind::App,
                            module_path: path.clone(),
                        });
                    }
                }
            }
        }

        results
    }

    // ===== Cross-References & Analysis =====

    fn callers_of(&self, id: Uuid) -> Vec<XrefSite> {
        let mut all_sites = Vec::new();
        for (path, cached) in &self.modules {
            let mut collector = XrefCollector::for_calls(id);
            collect_xrefs(&cached.program, &mut collector);
            all_sites.extend(collector.into_xref_sites(path, &cached.source));
        }
        all_sites
    }

    fn constructors_of(&self, id: Uuid) -> Vec<XrefSite> {
        let mut all_sites = Vec::new();
        for (path, cached) in &self.modules {
            let mut collector = XrefCollector::for_structs(id);
            collect_xrefs(&cached.program, &mut collector);
            all_sites.extend(collector.into_xref_sites(path, &cached.source));
        }
        all_sites
    }

    fn enum_usages_of(&self, id: Uuid) -> Vec<XrefSite> {
        let mut all_sites = Vec::new();
        for (path, cached) in &self.modules {
            let mut collector = XrefCollector::for_enums(id);
            collect_xrefs(&cached.program, &mut collector);
            all_sites.extend(collector.into_xref_sites(path, &cached.source));
        }
        all_sites
    }

    fn raise_sites_of(&self, id: Uuid) -> Vec<XrefSite> {
        let mut all_sites = Vec::new();
        for (path, cached) in &self.modules {
            let mut collector = XrefCollector::for_raises(id);
            collect_xrefs(&cached.program, &mut collector);
            all_sites.extend(collector.into_xref_sites(path, &cached.source));
        }
        all_sites
    }

    fn usages_of(&self, id: Uuid) -> Vec<XrefSite> {
        let mut all_sites = Vec::new();
        for (path, cached) in &self.modules {
            let mut collector = XrefCollector::for_all(id);
            collect_xrefs(&cached.program, &mut collector);
            all_sites.extend(collector.into_xref_sites(path, &cached.source));
        }
        all_sites
    }

    fn call_graph(
        &self,
        id: Uuid,
        _opts: &CallGraphOptions,
    ) -> Result<CallGraphResult, ServiceError> {
        // Find the function declaration
        let (root_name, root_path) = self
            .modules
            .iter()
            .find_map(|(path, cached)| {
                cached
                    .program
                    .functions
                    .iter()
                    .find(|f| f.node.id == id)
                    .map(|f| (f.node.name.node.clone(), path.clone()))
                    .or_else(|| {
                        // Search class methods too
                        cached.program.classes.iter().find_map(|c| {
                            c.node
                                .methods
                                .iter()
                                .find(|m| m.node.id == id)
                                .map(|m| (m.node.name.node.clone(), path.clone()))
                        })
                    })
            })
            .ok_or(ServiceError::DeclarationNotFound(id))?;

        let root = CallGraphNode {
            uuid: id,
            name: root_name,
            module_path: root_path,
            children: vec![], // TODO: recursive traversal
            is_cycle: false,
        };

        Ok(CallGraphResult { root })
    }

    fn error_set(&self, path: &Path, id: Uuid) -> Result<ErrorSetInfo, ServiceError> {
        let cached = self.get_module(path)?;

        let is_fallible = cached
            .derived
            .fn_signatures
            .get(&id)
            .map(|s| s.is_fallible)
            .unwrap_or(false);

        let errors = cached
            .derived
            .fn_error_sets
            .get(&id)
            .map(|es| es.iter().map(|e| e.name.clone()).collect())
            .unwrap_or_default();

        Ok(ErrorSetInfo {
            is_fallible,
            errors,
        })
    }

    // ===== Source Access =====

    fn get_source(&self, path: &Path, range: Option<ByteRange>) -> Result<String, ServiceError> {
        let source = std::fs::read_to_string(path).map_err(ServiceError::Io)?;

        match range {
            Some(ByteRange { start, end }) => {
                if end > source.len() {
                    return Err(ServiceError::InvalidParameter(format!(
                        "Range {}-{} exceeds source length {}",
                        start,
                        end,
                        source.len()
                    )));
                }
                Ok(source[start..end].to_string())
            }
            None => Ok(source),
        }
    }

    fn pretty_print(
        &self,
        path: &Path,
        id: Option<Uuid>,
        include_uuids: bool,
    ) -> Result<String, ServiceError> {
        let cached = self.get_module(path)?;

        match id {
            None => Ok(crate::pretty::pretty_print(
                &cached.program,
                include_uuids,
            )),
            Some(uuid) => {
                // Find the specific declaration and pretty-print it
                let program = &cached.program;

                if let Some(f) = program.functions.iter().find(|f| f.node.id == uuid) {
                    return Ok(crate::pretty::pretty_print_function(
                        &f.node,
                        include_uuids,
                    ));
                }
                if let Some(c) = program.classes.iter().find(|c| c.node.id == uuid) {
                    return Ok(crate::pretty::pretty_print_class(&c.node, include_uuids));
                }
                if let Some(e) = program.enums.iter().find(|e| e.node.id == uuid) {
                    return Ok(crate::pretty::pretty_print_enum(&e.node, include_uuids));
                }
                if let Some(t) = program.traits.iter().find(|t| t.node.id == uuid) {
                    return Ok(crate::pretty::pretty_print_trait(&t.node, include_uuids));
                }
                if let Some(e) = program.errors.iter().find(|e| e.node.id == uuid) {
                    return Ok(crate::pretty::pretty_print_error(&e.node, include_uuids));
                }
                if let Some(app) = &program.app {
                    if app.node.id == uuid {
                        return Ok(crate::pretty::pretty_print_app(&app.node, include_uuids));
                    }
                }

                Err(ServiceError::DeclarationNotFound(uuid))
            }
        }
    }

    // ===== Compilation & Execution =====

    fn check(&self, path: &Path, opts: &CompileOptions) -> CheckResult {
        match crate::analyze_file_with_warnings(path, opts.stdlib.as_deref()) {
            Ok((_program, _source, _derived, warnings)) => CheckResult {
                success: true,
                path: path.to_path_buf(),
                errors: vec![],
                warnings: warnings
                    .into_iter()
                    .map(|w| Diagnostic::from_compile_warning(&w, None))
                    .collect(),
            },
            Err(err) => CheckResult {
                success: false,
                path: path.to_path_buf(),
                errors: vec![Diagnostic::from_compile_error(&err, None)],
                warnings: vec![],
            },
        }
    }

    fn compile(&self, path: &Path, output: &Path, opts: &CompileOptions) -> CompileResult {
        match crate::compile_file_with_options(path, output, opts.stdlib.as_deref(), opts.gc) {
            Ok(()) => CompileResult {
                success: true,
                path: path.to_path_buf(),
                output: Some(output.to_path_buf()),
                errors: vec![],
                warnings: vec![],
            },
            Err(err) => CompileResult {
                success: false,
                path: path.to_path_buf(),
                output: None,
                errors: vec![Diagnostic::from_compile_error(&err, None)],
                warnings: vec![],
            },
        }
    }

    fn run(&self, path: &Path, opts: &RunOptions) -> RunResult {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        let temp_dir = std::env::temp_dir();
        let output = temp_dir.join(format!("pluto_run_{}", uuid::Uuid::new_v4()));

        match crate::compile_file_with_options(
            path,
            &output,
            opts.stdlib.as_deref(),
            crate::GcBackend::MarkSweep,
        ) {
            Ok(()) => {
                let mut cmd = Command::new(&output);
                cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
                if let Some(dir) = &opts.cwd {
                    cmd.current_dir(dir);
                }

                let timeout = Duration::from_millis(opts.timeout_ms.unwrap_or(10000));
                let start = Instant::now();

                match cmd.spawn() {
                    Ok(mut child) => {
                        let result = loop {
                            if start.elapsed() >= timeout {
                                let _ = child.kill();
                                break Ok((String::new(), String::new(), None, true));
                            }

                            match child.try_wait() {
                                Ok(Some(status)) => {
                                    let out = child
                                        .wait_with_output()
                                        .unwrap_or_else(|_| std::process::Output {
                                            status,
                                            stdout: vec![],
                                            stderr: vec![],
                                        });
                                    break Ok((
                                        String::from_utf8_lossy(&out.stdout).to_string(),
                                        String::from_utf8_lossy(&out.stderr).to_string(),
                                        status.code(),
                                        false,
                                    ));
                                }
                                Ok(None) => {
                                    std::thread::sleep(Duration::from_millis(50));
                                }
                                Err(e) => {
                                    break Err(e.to_string());
                                }
                            }
                        };

                        let _ = std::fs::remove_file(&output);

                        match result {
                            Ok((stdout, stderr, exit_code, timed_out)) => RunResult {
                                success: exit_code == Some(0),
                                path: path.to_path_buf(),
                                stdout,
                                stderr,
                                exit_code,
                                timed_out,
                                compile_errors: vec![],
                            },
                            Err(e) => RunResult {
                                success: false,
                                path: path.to_path_buf(),
                                stdout: String::new(),
                                stderr: e,
                                exit_code: None,
                                timed_out: false,
                                compile_errors: vec![],
                            },
                        }
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(&output);
                        RunResult {
                            success: false,
                            path: path.to_path_buf(),
                            stdout: String::new(),
                            stderr: e.to_string(),
                            exit_code: None,
                            timed_out: false,
                            compile_errors: vec![],
                        }
                    }
                }
            }
            Err(err) => RunResult {
                success: false,
                path: path.to_path_buf(),
                stdout: String::new(),
                stderr: String::new(),
                exit_code: None,
                timed_out: false,
                compile_errors: vec![Diagnostic::from_compile_error(&err, None)],
            },
        }
    }

    fn test(&self, path: &Path, opts: &TestOptions) -> TestResult {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        let temp_dir = std::env::temp_dir();
        let output = temp_dir.join(format!("pluto_test_{}", uuid::Uuid::new_v4()));

        // Compile in test mode
        match crate::compile_file_for_tests(path, &output, opts.stdlib.as_deref(), false) {
            Ok(()) => {
                let mut cmd = Command::new(&output);
                cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
                if let Some(dir) = &opts.cwd {
                    cmd.current_dir(dir);
                }

                let timeout = Duration::from_millis(opts.timeout_ms.unwrap_or(30000));
                let start = Instant::now();

                match cmd.spawn() {
                    Ok(mut child) => {
                        let result = loop {
                            if start.elapsed() >= timeout {
                                let _ = child.kill();
                                break Ok((String::new(), String::new(), None, true));
                            }

                            match child.try_wait() {
                                Ok(Some(status)) => {
                                    let out = child
                                        .wait_with_output()
                                        .unwrap_or_else(|_| std::process::Output {
                                            status,
                                            stdout: vec![],
                                            stderr: vec![],
                                        });
                                    break Ok((
                                        String::from_utf8_lossy(&out.stdout).to_string(),
                                        String::from_utf8_lossy(&out.stderr).to_string(),
                                        status.code(),
                                        false,
                                    ));
                                }
                                Ok(None) => {
                                    std::thread::sleep(Duration::from_millis(50));
                                }
                                Err(e) => {
                                    break Err(e.to_string());
                                }
                            }
                        };

                        let _ = std::fs::remove_file(&output);

                        match result {
                            Ok((stdout, stderr, exit_code, timed_out)) => TestResult {
                                success: exit_code == Some(0),
                                path: path.to_path_buf(),
                                stdout,
                                stderr,
                                exit_code,
                                timed_out,
                                compile_errors: vec![],
                            },
                            Err(e) => TestResult {
                                success: false,
                                path: path.to_path_buf(),
                                stdout: String::new(),
                                stderr: e,
                                exit_code: None,
                                timed_out: false,
                                compile_errors: vec![],
                            },
                        }
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(&output);
                        TestResult {
                            success: false,
                            path: path.to_path_buf(),
                            stdout: String::new(),
                            stderr: e.to_string(),
                            exit_code: None,
                            timed_out: false,
                            compile_errors: vec![],
                        }
                    }
                }
            }
            Err(err) => TestResult {
                success: false,
                path: path.to_path_buf(),
                stdout: String::new(),
                stderr: String::new(),
                exit_code: None,
                timed_out: false,
                compile_errors: vec![Diagnostic::from_compile_error(&err, None)],
            },
        }
    }

    // ===== Editing Operations (deferred) =====

    fn add_declaration(
        &mut self,
        _path: &Path,
        _source: &str,
    ) -> Result<EditResult, ServiceError> {
        Err(ServiceError::Internal(
            "add_declaration not yet implemented".to_string(),
        ))
    }

    fn replace_declaration(
        &mut self,
        _path: &Path,
        _name: &str,
        _source: &str,
    ) -> Result<EditResult, ServiceError> {
        Err(ServiceError::Internal(
            "replace_declaration not yet implemented".to_string(),
        ))
    }

    fn delete_declaration(
        &mut self,
        _path: &Path,
        _name: &str,
    ) -> Result<DeleteResult, ServiceError> {
        Err(ServiceError::Internal(
            "delete_declaration not yet implemented".to_string(),
        ))
    }

    fn rename_declaration(
        &mut self,
        _path: &Path,
        _old_name: &str,
        _new_name: &str,
    ) -> Result<EditResult, ServiceError> {
        Err(ServiceError::Internal(
            "rename_declaration not yet implemented".to_string(),
        ))
    }

    fn add_method(
        &mut self,
        _path: &Path,
        _class_name: &str,
        _source: &str,
    ) -> Result<EditResult, ServiceError> {
        Err(ServiceError::Internal(
            "add_method not yet implemented".to_string(),
        ))
    }

    fn add_field(
        &mut self,
        _path: &Path,
        _class_name: &str,
        _field_name: &str,
        _field_type: &str,
    ) -> Result<EditResult, ServiceError> {
        Err(ServiceError::Internal(
            "add_field not yet implemented".to_string(),
        ))
    }

    // ===== Format & Sync =====

    fn sync_pt(&mut self, _pt_path: &Path, _pluto_path: &Path) -> Result<SyncResult, ServiceError> {
        Err(ServiceError::Internal(
            "sync_pt not yet implemented".to_string(),
        ))
    }

    fn analyze_and_update(&self, path: &Path, opts: &LoadOptions) -> Result<(), ServiceError> {
        crate::analyze_and_update(path, opts.stdlib.as_deref())
            .map_err(|e| ServiceError::CompilationFailed(e.to_string()))
    }

    // ===== Documentation =====

    fn language_docs(&self, topic: Option<&str>) -> Result<String, ServiceError> {
        Ok(crate::docs::get_docs(topic))
    }

    fn stdlib_docs(&self, module: Option<&str>) -> Result<String, ServiceError> {
        crate::docs::get_stdlib_docs(module).map_err(|e| ServiceError::Internal(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_server() {
        let server = InProcessServer::new();
        assert_eq!(server.list_modules().len(), 0);
    }

    #[test]
    fn test_module_status_empty() {
        let server = InProcessServer::new();
        assert_eq!(server.module_status().len(), 0);
    }

    #[test]
    fn test_find_declaration_empty() {
        let server = InProcessServer::new();
        assert_eq!(server.find_declaration("foo", None).len(), 0);
    }

    #[test]
    fn test_callers_of_empty() {
        let server = InProcessServer::new();
        assert_eq!(server.callers_of(Uuid::new_v4()).len(), 0);
    }

    #[test]
    fn test_usages_of_empty() {
        let server = InProcessServer::new();
        assert_eq!(server.usages_of(Uuid::new_v4()).len(), 0);
    }
}
