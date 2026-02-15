use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::diagnostics::CompileError;
use crate::lexer;
use crate::manifest::{DependencyScope, PackageGraph};
use crate::parser::ast::*;
use crate::parser::Parser;
use crate::span::{Span, Spanned};
use crate::visit::{
    walk_app_mut, walk_block_mut, walk_class_mut, walk_enum_mut, walk_error_mut, walk_expr_mut,
    walk_extern_fn_mut, walk_function_mut, walk_program_mut, walk_stage_mut, walk_stmt_mut,
    walk_system_mut, walk_trait_mut, walk_type_expr_mut, VisitMut,
};

/// Maps file_id -> (path, source_text).
#[derive(Default)]
pub struct SourceMap {
    pub files: Vec<(PathBuf, String)>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_file(&mut self, path: PathBuf, source: String) -> u32 {
        let id = self.files.len() as u32;
        self.files.push((path, source));
        id
    }

    pub fn get_source(&self, file_id: u32) -> Option<(&Path, &str)> {
        self.files.get(file_id as usize).map(|(p, s)| (p.as_path(), s.as_str()))
    }
}

/// Tracks whether an import came from a local module or a package dependency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImportOrigin {
    Local,
    PackageDep,
}

/// Result of module resolution before flattening.
pub struct ModuleGraph {
    pub root: Program,
    pub imports: Vec<(String, Program, ImportOrigin)>,
    pub source_map: SourceMap,
}

/// Visitor that stamps all spans in an AST with a specific file_id.
/// Applied after parsing to attribute every span to its source file.
struct FileIdSetter {
    file_id: u32,
}

impl VisitMut for FileIdSetter {
    fn visit_function_mut(&mut self, func: &mut Spanned<Function>) {
        func.span.file_id = self.file_id;
        func.node.name.span.file_id = self.file_id;
        walk_function_mut(self, func);
    }

    fn visit_class_mut(&mut self, class: &mut Spanned<ClassDecl>) {
        class.span.file_id = self.file_id;
        class.node.name.span.file_id = self.file_id;
        for field in &mut class.node.fields {
            field.name.span.file_id = self.file_id;
        }
        walk_class_mut(self, class);
    }

    fn visit_trait_mut(&mut self, trait_decl: &mut Spanned<TraitDecl>) {
        trait_decl.span.file_id = self.file_id;
        trait_decl.node.name.span.file_id = self.file_id;
        walk_trait_mut(self, trait_decl);
    }

    fn visit_enum_mut(&mut self, enum_decl: &mut Spanned<EnumDecl>) {
        enum_decl.span.file_id = self.file_id;
        enum_decl.node.name.span.file_id = self.file_id;
        walk_enum_mut(self, enum_decl);
    }

    fn visit_error_mut(&mut self, error_decl: &mut Spanned<ErrorDecl>) {
        error_decl.span.file_id = self.file_id;
        error_decl.node.name.span.file_id = self.file_id;
        walk_error_mut(self, error_decl);
    }

    fn visit_app_mut(&mut self, app: &mut Spanned<AppDecl>) {
        app.span.file_id = self.file_id;
        app.node.name.span.file_id = self.file_id;
        walk_app_mut(self, app);
    }

    fn visit_stage_mut(&mut self, stage: &mut Spanned<StageDecl>) {
        stage.span.file_id = self.file_id;
        stage.node.name.span.file_id = self.file_id;
        walk_stage_mut(self, stage);
    }

    fn visit_system_mut(&mut self, system: &mut Spanned<SystemDecl>) {
        system.span.file_id = self.file_id;
        system.node.name.span.file_id = self.file_id;
        walk_system_mut(self, system);
    }

    fn visit_extern_fn_mut(&mut self, extern_fn: &mut Spanned<ExternFnDecl>) {
        extern_fn.span.file_id = self.file_id;
        extern_fn.node.name.span.file_id = self.file_id;
        walk_extern_fn_mut(self, extern_fn);
    }

    fn visit_import_mut(&mut self, import: &mut Spanned<ImportDecl>) {
        import.span.file_id = self.file_id;
    }

    fn visit_block_mut(&mut self, block: &mut Spanned<Block>) {
        block.span.file_id = self.file_id;
        walk_block_mut(self, block);
    }

    fn visit_stmt_mut(&mut self, stmt: &mut Spanned<Stmt>) {
        stmt.span.file_id = self.file_id;
        walk_stmt_mut(self, stmt);
    }

    fn visit_expr_mut(&mut self, expr: &mut Spanned<Expr>) {
        expr.span.file_id = self.file_id;
        walk_expr_mut(self, expr);
    }

    fn visit_type_expr_mut(&mut self, te: &mut Spanned<TypeExpr>) {
        te.span.file_id = self.file_id;
        walk_type_expr_mut(self, te);
    }
}

/// Stamp all spans in a Program with the given file_id.
fn set_program_file_id(program: &mut Program, file_id: u32) {
    let mut setter = FileIdSetter { file_id };
    walk_program_mut(&mut setter, program);
}

/// Load and parse a single .pluto file, assigning spans with the given file_id.
fn load_and_parse(path: &Path, source_map: &mut SourceMap) -> Result<(Program, u32), CompileError> {
    let source = std::fs::read_to_string(path).map_err(|e| {
        CompileError::codegen(format!("could not read '{}': {e}", path.display()))
    })?;
    let file_id = source_map.add_file(path.to_path_buf(), source.clone());
    let tokens = lexer::lex(&source)?;
    let mut parser = Parser::new_with_path(&tokens, &source, path.display().to_string());
    let mut program = parser.parse_program()?;
    set_program_file_id(&mut program, file_id);
    Ok((program, file_id))
}

/// Load all .pluto files in a directory and merge into one Program.
/// If `mod.pluto` exists, only that file is loaded; otherwise all .pluto files are auto-merged.
/// Sub-imports within loaded files are recursively resolved and flattened into the result.
fn load_directory_module(
    dir: &Path,
    source_map: &mut SourceMap,
    visited: &mut HashSet<PathBuf>,
    effective_stdlib: Option<&Path>,
    current_deps: &DependencyScope,
    pkg_graph: &PackageGraph,
    parent_origin: ImportOrigin,
) -> Result<Program, CompileError> {
    // Directory cycle detection with closure cleanup pattern
    let canonical_dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    if visited.contains(&canonical_dir) {
        return Err(CompileError::codegen(format!(
            "circular import detected: '{}'", dir.display()
        )));
    }
    visited.insert(canonical_dir.clone());
    let result = (|| {
        let mut merged = Program {
            imports: Vec::new(),
            functions: Vec::new(),
            extern_fns: Vec::new(),
            classes: Vec::new(),
            traits: Vec::new(),
            enums: Vec::new(),
            app: None,
            stages: Vec::new(),
            system: None,
            errors: Vec::new(),
            test_info: Vec::new(),
            tests: None,
            fallible_extern_fns: Vec::new(),
        };

        let entries = std::fs::read_dir(dir).map_err(|e| {
            CompileError::codegen(format!("could not read directory '{}': {e}", dir.display()))
        })?;

        let mut pluto_files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "pluto"))
            .collect();
        pluto_files.sort();

        for file_path in pluto_files {
            let (program, _file_id) = load_and_parse(&file_path, source_map)?;
            merged.functions.extend(program.functions);
            merged.extern_fns.extend(program.extern_fns);
            merged.classes.extend(program.classes);
            merged.traits.extend(program.traits);
            merged.enums.extend(program.enums);
            if let Some(app_decl) = program.app {
                if merged.app.is_some() {
                    return Err(CompileError::codegen(format!(
                        "multiple app declarations in module directory '{}'",
                        dir.display()
                    )));
                }
                merged.app = Some(app_decl);
            }
            if let Some(system_decl) = program.system {
                if merged.system.is_some() {
                    return Err(CompileError::codegen(format!(
                        "multiple system declarations in module directory '{}'",
                        dir.display()
                    )));
                }
                merged.system = Some(system_decl);
            }
            merged.stages.extend(program.stages);
            merged.errors.extend(program.errors);
            merged.test_info.extend(program.test_info);
            if let Some(tests_decl) = program.tests {
                if merged.tests.is_some() {
                    return Err(CompileError::codegen(format!(
                        "multiple tests declarations in module directory '{}'",
                        dir.display()
                    )));
                }
                merged.tests = Some(tests_decl);
            }
            merged.imports.extend(program.imports);
        }

        resolve_module_imports(&mut merged, dir, source_map, visited, effective_stdlib, current_deps, pkg_graph, parent_origin)?;

        Ok(merged)
    })();
    visited.remove(&canonical_dir);
    result
}

/// Resolve a multi-segment import path to a module, with recursive sub-import resolution.
#[allow(clippy::too_many_arguments)]
fn resolve_module_path(
    segments: &[Spanned<String>],
    base_dir: &Path,
    source_map: &mut SourceMap,
    import_span: Span,
    visited: &mut HashSet<PathBuf>,
    effective_stdlib: Option<&Path>,
    current_deps: &DependencyScope,
    pkg_graph: &PackageGraph,
    parent_origin: ImportOrigin,
) -> Result<Program, CompileError> {
    let mut current_dir = base_dir.to_path_buf();

    // Walk intermediate segments (all but the last)
    for segment in &segments[..segments.len() - 1] {
        let next_dir = current_dir.join(&segment.node);
        if !next_dir.is_dir() {
            return Err(CompileError::syntax(
                format!("cannot find module path: '{}' is not a directory", next_dir.display()),
                import_span,
            ));
        }
        current_dir = next_dir;
    }

    // Resolve final segment
    let final_seg = &segments[segments.len() - 1];
    let file_path = current_dir.join(format!("{}.pluto", final_seg.node));
    let dir_path = current_dir.join(&final_seg.node);

    if file_path.is_file() {
        let canonical = file_path.canonicalize().unwrap_or_else(|_| file_path.clone());
        if visited.contains(&canonical) {
            return Err(CompileError::codegen(format!(
                "circular import detected: '{}'",
                file_path.display()
            )));
        }
        visited.insert(canonical.clone());
        let (mut module_prog, _) = load_and_parse(&file_path, source_map)?;
        resolve_module_imports(&mut module_prog, &current_dir, source_map, visited, effective_stdlib, current_deps, pkg_graph, parent_origin)?;
        visited.remove(&canonical);
        Ok(module_prog)
    } else if dir_path.is_dir() {
        load_directory_module(&dir_path, source_map, visited, effective_stdlib, current_deps, pkg_graph, parent_origin)
    } else {
        let full_path: Vec<&str> = segments.iter().map(|s| s.node.as_str()).collect();
        Err(CompileError::syntax(
            format!("cannot find module '{}': no directory or file found", full_path.join(".")),
            final_seg.span,
        ))
    }
}

/// Resolve all imports within a module's Program, flattening sub-imports into it.
/// This is the core recursive function: for each import in `program`, resolve the sub-module,
/// then flatten its items into `program` with prefixed names.
#[allow(clippy::too_many_arguments)]
fn resolve_module_imports(
    program: &mut Program,
    module_dir: &Path,
    source_map: &mut SourceMap,
    visited: &mut HashSet<PathBuf>,
    effective_stdlib: Option<&Path>,
    current_deps: &DependencyScope,
    pkg_graph: &PackageGraph,
    parent_origin: ImportOrigin,
) -> Result<(), CompileError> {
    if program.imports.is_empty() {
        return Ok(());
    }

    let imports_to_resolve: Vec<Spanned<ImportDecl>> = std::mem::take(&mut program.imports);
    let mut imported_names: HashMap<String, String> = HashMap::new();
    let mut resolved_imports: Vec<(String, Program, ImportOrigin)> = Vec::new();

    for import in &imports_to_resolve {
        let binding_name = import.node.binding_name().to_string();
        let full_path = import.node.full_path();

        // Duplicate import handling: allow exact duplicates, error on conflicts
        if let Some(prev_path) = imported_names.get(&binding_name) {
            if *prev_path == full_path {
                continue; // Exact duplicate — deduplicate silently
            } else {
                return Err(CompileError::syntax(
                    format!("conflicting import binding '{}': imports '{}' and '{}'", binding_name, prev_path, full_path),
                    import.span,
                ));
            }
        }
        imported_names.insert(binding_name.clone(), full_path.clone());

        let first_segment = &import.node.path[0].node;

        if import.node.path.len() == 1 {
            // Single-segment import
            let is_dep = current_deps.contains_key(first_segment);
            let dir_path = module_dir.join(first_segment);
            let file_path_candidate = module_dir.join(format!("{}.pluto", first_segment));
            let is_local = dir_path.is_dir() || file_path_candidate.is_file();

            if is_dep && is_local {
                return Err(CompileError::syntax(
                    format!("import '{}' is ambiguous: declared as dependency and also exists locally", first_segment),
                    import.node.path[0].span,
                ));
            }

            if is_dep {
                let dep_path = &current_deps[first_segment];
                let dep_canonical = dep_path.canonicalize().map_err(|e| {
                    CompileError::codegen(format!("cannot resolve dep path '{}': {e}", dep_path.display()))
                })?;
                let dep_scope = pkg_graph.deps_for(&dep_canonical);
                let module_prog = load_directory_module(dep_path, source_map, visited, effective_stdlib, dep_scope, pkg_graph, ImportOrigin::PackageDep)?;
                resolved_imports.push((binding_name, module_prog, ImportOrigin::PackageDep));
            } else if dir_path.is_dir() {
                let origin = if parent_origin == ImportOrigin::PackageDep { ImportOrigin::PackageDep } else { ImportOrigin::Local };
                let module_prog = load_directory_module(&dir_path, source_map, visited, effective_stdlib, current_deps, pkg_graph, parent_origin)?;
                resolved_imports.push((binding_name, module_prog, origin));
            } else if file_path_candidate.is_file() {
                let canonical = file_path_candidate.canonicalize().unwrap_or_else(|_| file_path_candidate.clone());
                if visited.contains(&canonical) {
                    return Err(CompileError::codegen(format!(
                        "circular import detected: '{}'",
                        file_path_candidate.display()
                    )));
                }
                visited.insert(canonical.clone());
                let (mut module_prog, _) = load_and_parse(&file_path_candidate, source_map)?;
                resolve_module_imports(&mut module_prog, module_dir, source_map, visited, effective_stdlib, current_deps, pkg_graph, parent_origin)?;
                visited.remove(&canonical);
                let origin = if parent_origin == ImportOrigin::PackageDep { ImportOrigin::PackageDep } else { ImportOrigin::Local };
                resolved_imports.push((binding_name, module_prog, origin));
            } else {
                return Err(CompileError::syntax(
                    format!("cannot find module '{}': no directory or file found", full_path),
                    import.node.path[0].span,
                ));
            }
        } else if first_segment == "std" {
            // Stdlib import
            match effective_stdlib {
                Some(root) => {
                    let remaining = &import.node.path[1..];
                    let module_prog = resolve_module_path(remaining, root, source_map, import.span, visited, effective_stdlib, current_deps, pkg_graph, parent_origin)?;
                    let origin = if parent_origin == ImportOrigin::PackageDep { ImportOrigin::PackageDep } else { ImportOrigin::Local };
                    resolved_imports.push((binding_name, module_prog, origin));
                }
                None => {
                    return Err(CompileError::syntax(
                        format!(
                            "cannot import '{}': no stdlib root found (tried --stdlib flag, PLUTO_STDLIB env var, and ./stdlib relative to entry file)",
                            full_path
                        ),
                        import.span,
                    ));
                }
            }
        } else {
            // Multi-segment import
            let is_dep = current_deps.contains_key(first_segment);
            let dir_path = module_dir.join(first_segment);
            let is_local = dir_path.is_dir();

            if is_dep && is_local {
                return Err(CompileError::syntax(
                    format!("import '{}' is ambiguous: declared as dependency and also exists locally", full_path),
                    import.node.path[0].span,
                ));
            }

            if is_dep {
                // Resolve remaining segments from dep path
                let dep_path = &current_deps[first_segment];
                let dep_canonical = dep_path.canonicalize().map_err(|e| {
                    CompileError::codegen(format!("cannot resolve dep path '{}': {e}", dep_path.display()))
                })?;
                let dep_scope = pkg_graph.deps_for(&dep_canonical);
                let remaining = &import.node.path[1..];
                let module_prog = resolve_module_path(remaining, dep_path, source_map, import.span, visited, effective_stdlib, dep_scope, pkg_graph, ImportOrigin::PackageDep)?;
                resolved_imports.push((binding_name, module_prog, ImportOrigin::PackageDep));
            } else {
                // Multi-segment import from project
                let origin = if parent_origin == ImportOrigin::PackageDep { ImportOrigin::PackageDep } else { ImportOrigin::Local };
                let module_prog = resolve_module_path(&import.node.path, module_dir, source_map, import.span, visited, effective_stdlib, current_deps, pkg_graph, parent_origin)?;
                resolved_imports.push((binding_name, module_prog, origin));
            }
        }
    }

    // Flatten resolved imports into the program
    flatten_into_program(program, resolved_imports)?;

    Ok(())
}

/// Format a module-prefixed name: "module.name".
fn prefix_name(module_name: &str, name: &str) -> String {
    format!("{}.{}", module_name, name)
}

/// Validate that imported modules don't contain app or extern_rust declarations.
fn validate_imported_modules(imports: &[(String, Program, ImportOrigin)]) -> Result<(), CompileError> {
    for (module_name, module_prog, _origin) in imports {
        if module_prog.app.is_some() {
            return Err(CompileError::codegen(format!(
                "app declarations are not allowed in imported modules (found in '{}')",
                module_name
            )));
        }
    }
    Ok(())
}

/// Add all items from a module into the target program with prefixed names.
/// Handles functions, classes, traits, enums, errors, and extern fns (deduplicated).
fn add_prefixed_items(
    target: &mut Program,
    module_name: &str,
    module_prog: &Program,
) -> Result<(), CompileError> {
    // Functions
    for func in &module_prog.functions {
        let mut prefixed_func = func.clone();
        prefixed_func.node.name.node = prefix_name(module_name, &func.node.name.node);
        prefix_function_types(&mut prefixed_func.node, module_name, module_prog);
        target.functions.push(prefixed_func);
    }

    // Classes
    for class in &module_prog.classes {
        let mut prefixed_class = class.clone();
        prefixed_class.node.name.node = prefix_name(module_name, &class.node.name.node);
        for field in &mut prefixed_class.node.fields {
            prefix_type_expr(&mut field.ty.node, module_name, module_prog);
        }
        for method in &mut prefixed_class.node.methods {
            prefix_function_types(&mut method.node, module_name, module_prog);
        }
        for trait_name in &mut prefixed_class.node.impl_traits {
            if module_prog.traits.iter().any(|t| t.node.name.node == trait_name.node) {
                trait_name.node = prefix_name(module_name, &trait_name.node);
            }
        }
        target.classes.push(prefixed_class);
    }

    // Traits
    for tr in &module_prog.traits {
        let mut prefixed_trait = tr.clone();
        prefixed_trait.node.name.node = prefix_name(module_name, &tr.node.name.node);
        for method in &mut prefixed_trait.node.methods {
            for param in &mut method.params {
                prefix_type_expr(&mut param.ty.node, module_name, module_prog);
            }
            if let Some(ret) = &mut method.return_type {
                prefix_type_expr(&mut ret.node, module_name, module_prog);
            }
        }
        target.traits.push(prefixed_trait);
    }

    // Enums
    for enum_decl in &module_prog.enums {
        let mut prefixed_enum = enum_decl.clone();
        prefixed_enum.node.name.node = prefix_name(module_name, &enum_decl.node.name.node);
        for variant in &mut prefixed_enum.node.variants {
            for field in &mut variant.fields {
                prefix_type_expr(&mut field.ty.node, module_name, module_prog);
            }
        }
        target.enums.push(prefixed_enum);
    }

    // Errors
    for error_decl in &module_prog.errors {
        let mut prefixed_error = error_decl.clone();
        prefixed_error.node.name.node = prefix_name(module_name, &error_decl.node.name.node);
        for field in &mut prefixed_error.node.fields {
            prefix_type_expr(&mut field.ty.node, module_name, module_prog);
        }
        target.errors.push(prefixed_error);
    }

    // Extern fns (NOT prefixed — C symbols stay as-is, but deduplicated)
    for ext_fn in &module_prog.extern_fns {
        let existing = target.extern_fns.iter()
            .find(|e| e.node.name.node == ext_fn.node.name.node);
        if let Some(existing) = existing {
            if !extern_fn_sigs_match(&existing.node, &ext_fn.node) {
                return Err(CompileError::codegen(format!(
                    "conflicting extern fn signatures for '{}'",
                    ext_fn.node.name.node
                )));
            }
        } else {
            target.extern_fns.push(ext_fn.clone());
        }
    }

    Ok(())
}

/// Flatten resolved imports into a program by prefixing names.
/// Used for sub-module flattening (within a module's own imports).
/// Adds ALL items (not just pub) since visibility is deferred.
fn flatten_into_program(
    program: &mut Program,
    imports: Vec<(String, Program, ImportOrigin)>,
) -> Result<(), CompileError> {
    let import_names: HashSet<String> = imports.iter().map(|(n, _, _)| n.clone()).collect();

    validate_imported_modules(&imports)?;

    for (module_name, module_prog, _origin) in &imports {
        add_prefixed_items(program, module_name, module_prog)?;
    }

    rewrite_program(program, &import_names);

    Ok(())
}

/// Compare two TypeExpr values ignoring source spans.
fn type_expr_eq(a: &TypeExpr, b: &TypeExpr) -> bool {
    match (a, b) {
        (TypeExpr::Named(na), TypeExpr::Named(nb)) => na == nb,
        (TypeExpr::Array(ia), TypeExpr::Array(ib)) => type_expr_eq(&ia.node, &ib.node),
        (TypeExpr::Qualified { module: ma, name: na }, TypeExpr::Qualified { module: mb, name: nb }) => {
            ma == mb && na == nb
        }
        (TypeExpr::Fn { params: pa, return_type: ra }, TypeExpr::Fn { params: pb, return_type: rb }) => {
            pa.len() == pb.len()
                && pa.iter().zip(pb.iter()).all(|(a, b)| type_expr_eq(&a.node, &b.node))
                && type_expr_eq(&ra.node, &rb.node)
        }
        (TypeExpr::Generic { name: na, type_args: ta }, TypeExpr::Generic { name: nb, type_args: tb }) => {
            na == nb
                && ta.len() == tb.len()
                && ta.iter().zip(tb.iter()).all(|(a, b)| type_expr_eq(&a.node, &b.node))
        }
        (TypeExpr::Nullable(ia), TypeExpr::Nullable(ib)) => type_expr_eq(&ia.node, &ib.node),
        _ => false,
    }
}

/// Check if two extern fn declarations have matching signatures.
fn extern_fn_sigs_match(a: &ExternFnDecl, b: &ExternFnDecl) -> bool {
    if a.params.len() != b.params.len() {
        return false;
    }
    for (pa, pb) in a.params.iter().zip(b.params.iter()) {
        if !type_expr_eq(&pa.ty.node, &pb.ty.node) {
            return false;
        }
    }
    match (&a.return_type, &b.return_type) {
        (Some(ra), Some(rb)) => type_expr_eq(&ra.node, &rb.node),
        (None, None) => true,
        _ => false,
    }
}

/// Resolve all modules referenced by the entry file.
///
/// 1. Parse entry file to discover imports
/// 2. Load sibling .pluto files (excluding imported single-file modules) and merge into root
/// 3. For each import, find `<name>/` directory or `<name>.pluto` and load as a separate module
///
/// `stdlib_root`: if set, `import std.x` resolves `x` from this path instead of entry_dir.
pub fn resolve_modules(
    entry_file: &Path,
    stdlib_root: Option<&Path>,
    pkg_graph: &PackageGraph,
) -> Result<ModuleGraph, CompileError> {
    resolve_modules_inner(entry_file, stdlib_root, pkg_graph, false)
}

/// Like resolve_modules but skips sibling .pluto file auto-merging.
/// Used for system member compilation where each member is compiled in isolation.
pub fn resolve_modules_no_siblings(
    entry_file: &Path,
    stdlib_root: Option<&Path>,
    pkg_graph: &PackageGraph,
) -> Result<ModuleGraph, CompileError> {
    resolve_modules_inner(entry_file, stdlib_root, pkg_graph, true)
}

fn resolve_modules_inner(
    entry_file: &Path,
    stdlib_root: Option<&Path>,
    pkg_graph: &PackageGraph,
    skip_siblings: bool,
) -> Result<ModuleGraph, CompileError> {
    let entry_file = entry_file.canonicalize().map_err(|e| {
        CompileError::codegen(format!("could not resolve path '{}': {e}", entry_file.display()))
    })?;
    let entry_dir = entry_file.parent().ok_or_else(|| {
        CompileError::codegen("entry file has no parent directory")
    })?;

    let mut source_map = SourceMap::new();

    // Compute effective stdlib root once
    let fallback_stdlib = entry_dir.join("stdlib");
    let effective_stdlib: Option<&Path> = if let Some(root) = stdlib_root {
        Some(root)
    } else if fallback_stdlib.is_dir() {
        Some(&fallback_stdlib)
    } else {
        None
    };

    let current_deps = pkg_graph.root_deps();

    // Circular import detection: track canonical paths in resolution stack
    let mut visited = HashSet::new();
    visited.insert(entry_file.clone());

    // First, parse the entry file to discover imports
    let (entry_prog, _entry_file_id) = load_and_parse(&entry_file, &mut source_map)?;

    // Collect import binding names to know which sibling .pluto files are imported modules
    let import_first_segments: HashSet<String> = entry_prog.imports.iter()
        .map(|i| i.node.path[0].node.clone())
        .collect();

    // Also collect dep names — sibling files matching dep names should be skipped
    // (they would cause ambiguity errors later if imported)
    let dep_names: HashSet<&String> = current_deps.keys().collect();

    // Start root with the entry file's contents
    let mut root = entry_prog;

    // Load sibling .pluto files (excluding the entry file and imported single-file modules)
    // Skip this step when compiling system members in isolation
    if !skip_siblings {
        let entries = std::fs::read_dir(entry_dir).map_err(|e| {
            CompileError::codegen(format!("could not read directory '{}': {e}", entry_dir.display()))
        })?;

        let mut sibling_files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|ext| ext == "pluto")
                    && p.canonicalize().unwrap_or(p.clone()) != entry_file
            })
            .collect();
        sibling_files.sort();

        for file_path in &sibling_files {
            let stem = file_path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            // Skip files that match an import first segment (they'll be loaded as modules)
            if import_first_segments.contains(stem) {
                continue;
            }
            // Skip files that match a dep name
            if dep_names.contains(&stem.to_string()) {
                continue;
            }
            let (program, _file_id) = load_and_parse(file_path, &mut source_map)
                .map_err(|err| CompileError::sibling_file(file_path.clone(), err))?;
            // Merge sibling's imports into root (they might also have imports)
            root.imports.extend(program.imports);
            root.functions.extend(program.functions);
            root.extern_fns.extend(program.extern_fns);
            root.classes.extend(program.classes);
            root.traits.extend(program.traits);
            root.enums.extend(program.enums);
            if let Some(app_decl) = program.app {
                if root.app.is_some() {
                    return Err(CompileError::codegen(
                        "multiple app declarations in project".to_string(),
                    ));
                }
                root.app = Some(app_decl);
            }
            if let Some(system_decl) = program.system {
                if root.system.is_some() {
                    return Err(CompileError::codegen(
                        "multiple system declarations in project".to_string(),
                    ));
                }
                root.system = Some(system_decl);
            }
            root.stages.extend(program.stages);
            root.errors.extend(program.errors);
            root.test_info.extend(program.test_info);
        }
    }

    // Resolve each import (now with recursive sub-import support)
    let mut imports: Vec<(String, Program, ImportOrigin)> = Vec::new();
    let mut imported_names: HashMap<String, String> = HashMap::new();

    for import in &root.imports {
        let binding_name = import.node.binding_name().to_string();
        let full_path = import.node.full_path();

        // Duplicate import handling: allow exact duplicates, error on conflicts
        if let Some(prev_path) = imported_names.get(&binding_name) {
            if *prev_path == full_path {
                continue; // Exact duplicate — deduplicate silently
            } else {
                return Err(CompileError::syntax(
                    format!("conflicting import binding '{}': imports '{}' and '{}'", binding_name, prev_path, full_path),
                    import.span,
                ));
            }
        }
        imported_names.insert(binding_name.clone(), full_path.clone());

        let first_segment = &import.node.path[0].node;

        if import.node.path.len() == 1 {
            // Single-segment import (e.g., `import math`) — resolve from entry_dir
            let is_dep = current_deps.contains_key(first_segment);
            let dir_path = entry_dir.join(first_segment);
            let file_path_candidate = entry_dir.join(format!("{}.pluto", first_segment));
            let is_local = dir_path.is_dir() || file_path_candidate.is_file();

            if is_dep && is_local {
                return Err(CompileError::syntax(
                    format!("import '{}' is ambiguous: declared as dependency and also exists locally", first_segment),
                    import.node.path[0].span,
                ));
            }

            if is_dep {
                let dep_path = &current_deps[first_segment];
                let dep_canonical = dep_path.canonicalize().map_err(|e| {
                    CompileError::codegen(format!("cannot resolve dep path '{}': {e}", dep_path.display()))
                })?;
                let dep_scope = pkg_graph.deps_for(&dep_canonical);
                let module_prog = load_directory_module(dep_path, &mut source_map, &mut visited, effective_stdlib, dep_scope, pkg_graph, ImportOrigin::PackageDep)?;
                imports.push((binding_name, module_prog, ImportOrigin::PackageDep));
            } else if dir_path.is_dir() {
                let module_prog = load_directory_module(&dir_path, &mut source_map, &mut visited, effective_stdlib, current_deps, pkg_graph, ImportOrigin::Local)?;
                imports.push((binding_name, module_prog, ImportOrigin::Local));
            } else if file_path_candidate.is_file() {
                let canonical = file_path_candidate.canonicalize().unwrap_or_else(|_| file_path_candidate.clone());
                if visited.contains(&canonical) {
                    return Err(CompileError::codegen(format!(
                        "circular import detected: '{}'",
                        file_path_candidate.display()
                    )));
                }
                visited.insert(canonical.clone());
                let (mut module_prog, _) = load_and_parse(&file_path_candidate, &mut source_map)?;
                // Recursively resolve sub-imports
                resolve_module_imports(&mut module_prog, entry_dir, &mut source_map, &mut visited, effective_stdlib, current_deps, pkg_graph, ImportOrigin::Local)?;
                visited.remove(&canonical);
                imports.push((binding_name, module_prog, ImportOrigin::Local));
            } else {
                return Err(CompileError::syntax(
                    format!("cannot find module '{}': no directory or file found", full_path),
                    import.node.path[0].span,
                ));
            }
        } else if first_segment == "std" {
            // Stdlib import: `import std.io` → resolve remaining path from stdlib_root
            match effective_stdlib {
                Some(root) => {
                    // Skip the "std" prefix, resolve remaining segments from stdlib root
                    let remaining = &import.node.path[1..];
                    let module_prog = resolve_module_path(remaining, root, &mut source_map, import.span, &mut visited, effective_stdlib, current_deps, pkg_graph, ImportOrigin::Local)?;
                    imports.push((binding_name, module_prog, ImportOrigin::Local));
                }
                None => {
                    return Err(CompileError::syntax(
                        format!(
                            "cannot import '{}': no stdlib root found (tried --stdlib flag, PLUTO_STDLIB env var, and ./stdlib relative to entry file)",
                            full_path
                        ),
                        import.span,
                    ));
                }
            }
        } else {
            // Multi-segment import
            let is_dep = current_deps.contains_key(first_segment);
            let dir_path = entry_dir.join(first_segment);
            let is_local = dir_path.is_dir();

            if is_dep && is_local {
                return Err(CompileError::syntax(
                    format!("import '{}' is ambiguous: declared as dependency and also exists locally", full_path),
                    import.node.path[0].span,
                ));
            }

            if is_dep {
                let dep_path = &current_deps[first_segment];
                let dep_canonical = dep_path.canonicalize().map_err(|e| {
                    CompileError::codegen(format!("cannot resolve dep path '{}': {e}", dep_path.display()))
                })?;
                let dep_scope = pkg_graph.deps_for(&dep_canonical);
                let remaining = &import.node.path[1..];
                let module_prog = resolve_module_path(remaining, dep_path, &mut source_map, import.span, &mut visited, effective_stdlib, dep_scope, pkg_graph, ImportOrigin::PackageDep)?;
                imports.push((binding_name, module_prog, ImportOrigin::PackageDep));
            } else {
                // Multi-segment import from project (e.g., `import utils.math`) — resolve from entry_dir
                let module_prog = resolve_module_path(&import.node.path, entry_dir, &mut source_map, import.span, &mut visited, effective_stdlib, current_deps, pkg_graph, ImportOrigin::Local)?;
                imports.push((binding_name, module_prog, ImportOrigin::Local));
            }
        }
    }

    Ok(ModuleGraph { root, imports, source_map })
}

/// Flatten imported modules into the root program by prefixing names.
///
/// For each imported module:
/// - Add ALL items with prefixed names (visibility deferred)
/// - Rewrite qualified references in the root program's AST
pub fn flatten_modules(mut graph: ModuleGraph) -> Result<(Program, SourceMap), CompileError> {
    let import_names: HashSet<String> = graph.imports.iter().map(|(n, _, _)| n.clone()).collect();

    validate_imported_modules(&graph.imports)?;

    // Filter out test functions from imported modules before merging
    for (_module_name, module_prog, _origin) in &mut graph.imports {
        let test_fn_names: HashSet<String> = module_prog.test_info.iter()
            .map(|t| t.fn_name.clone()).collect();
        module_prog.functions.retain(|f| !test_fn_names.contains(&f.node.name.node));
        module_prog.test_info.clear();
        module_prog.tests = None;
    }

    for (module_name, module_prog, _origin) in &graph.imports {
        add_prefixed_items(&mut graph.root, module_name, module_prog)?;
    }

    // Rewrite qualified references in root program's AST
    rewrite_program(&mut graph.root, &import_names);

    // Resolve QualifiedAccess nodes: convert to FieldAccess or keep for type checker
    resolve_qualified_access_in_program(&mut graph.root, &import_names);

    // Clear imports since they've been flattened
    graph.root.imports.clear();

    Ok((graph.root, graph.source_map))
}

/// Resolve QualifiedAccess nodes for single-file programs (no imports).
/// All QualifiedAccess nodes become FieldAccess chains since there are no modules.
pub fn resolve_qualified_access_single_file(program: &mut Program) -> Result<(), CompileError> {
    let module_names = HashSet::new(); // Empty set = no modules
    resolve_qualified_access_in_program(program, &module_names);
    Ok(())
}

/// Check if a type name refers to a class or trait defined in the given module.
fn is_module_type(name: &str, module_prog: &Program) -> bool {
    module_prog.classes.iter().any(|c| c.node.name.node == name)
        || module_prog.traits.iter().any(|t| t.node.name.node == name)
        || module_prog.enums.iter().any(|e| e.node.name.node == name)
        || module_prog.errors.iter().any(|e| e.node.name.node == name)
}

/// Prefix type expressions that reference module-internal types.
fn prefix_type_expr(ty: &mut TypeExpr, module_name: &str, module_prog: &Program) {
    match ty {
        TypeExpr::Named(name) => {
            if is_module_type(name, module_prog) {
                *name = prefix_name(module_name, name);
            }
        }
        TypeExpr::Array(inner) => {
            prefix_type_expr(&mut inner.node, module_name, module_prog);
        }
        TypeExpr::Qualified { .. } => {
            // Already qualified, leave alone
        }
        TypeExpr::Fn { params, return_type } => {
            for p in params {
                prefix_type_expr(&mut p.node, module_name, module_prog);
            }
            prefix_type_expr(&mut return_type.node, module_name, module_prog);
        }
        TypeExpr::Generic { name, type_args } => {
            if is_module_type(name, module_prog) {
                *name = prefix_name(module_name, name);
            }
            for arg in type_args {
                prefix_type_expr(&mut arg.node, module_name, module_prog);
            }
        }
        TypeExpr::Nullable(inner) => {
            prefix_type_expr(&mut inner.node, module_name, module_prog);
        }
        TypeExpr::Stream(inner) => {
            prefix_type_expr(&mut inner.node, module_name, module_prog);
        }
    }
}

/// Prefix types referenced in a function's params and return type.
fn prefix_function_types(func: &mut Function, module_name: &str, module_prog: &Program) {
    for param in &mut func.params {
        if param.name.node != "self" {
            prefix_type_expr(&mut param.ty.node, module_name, module_prog);
        }
    }
    if let Some(ret) = &mut func.return_type {
        prefix_type_expr(&mut ret.node, module_name, module_prog);
    }
    // Also rewrite expressions inside the body that reference internal types
    rewrite_block_for_module(&mut func.body.node, module_name, module_prog);
}

/// Rewrite expressions inside a block for module-internal references.
struct ModuleRewriter<'a> {
    module_name: &'a str,
    module_prog: &'a Program,
}

impl VisitMut for ModuleRewriter<'_> {
    fn visit_expr_mut(&mut self, expr: &mut Spanned<Expr>) {
        // Handle expressions that need name prefixing
        match &mut expr.node {
            Expr::Call { name, .. } => {
                // Prefix calls to module-internal functions (but NOT extern fns)
                if self.module_prog.functions.iter().any(|f| f.node.name.node == name.node) {
                    name.node = prefix_name(self.module_name, &name.node);
                }
            }
            Expr::StructLit { name, type_args, .. } => {
                if is_module_type(&name.node, self.module_prog) {
                    name.node = prefix_name(self.module_name, &name.node);
                }
                for ta in type_args {
                    prefix_type_expr(&mut ta.node, self.module_name, self.module_prog);
                }
            }
            Expr::EnumUnit { enum_name, type_args, .. } => {
                if is_module_type(&enum_name.node, self.module_prog) {
                    enum_name.node = prefix_name(self.module_name, &enum_name.node);
                }
                for ta in type_args {
                    prefix_type_expr(&mut ta.node, self.module_name, self.module_prog);
                }
            }
            Expr::EnumData { enum_name, type_args, .. } => {
                if is_module_type(&enum_name.node, self.module_prog) {
                    enum_name.node = prefix_name(self.module_name, &enum_name.node);
                }
                for ta in type_args {
                    prefix_type_expr(&mut ta.node, self.module_name, self.module_prog);
                }
            }
            Expr::MapLit { key_type, value_type, .. } => {
                prefix_type_expr(&mut key_type.node, self.module_name, self.module_prog);
                prefix_type_expr(&mut value_type.node, self.module_name, self.module_prog);
            }
            Expr::SetLit { elem_type, .. } => {
                prefix_type_expr(&mut elem_type.node, self.module_name, self.module_prog);
            }
            Expr::Cast { target_type, .. } => {
                prefix_type_expr(&mut target_type.node, self.module_name, self.module_prog);
            }
            Expr::Closure { params, .. } => {
                for p in params {
                    prefix_type_expr(&mut p.ty.node, self.module_name, self.module_prog);
                }
            }
            Expr::StringInterp { parts } => {
                for part in parts {
                    if let StringInterpPart::Expr(e) = part {
                        self.visit_expr_mut(e);
                    }
                }
                return;
            }
            _ => {}
        }
        // Recurse into sub-expressions
        walk_expr_mut(self, expr);
    }

    fn visit_stmt_mut(&mut self, stmt: &mut Spanned<Stmt>) {
        // Handle statements that need special processing
        match &mut stmt.node {
            Stmt::Let { ty, .. } => {
                if let Some(t) = ty {
                    prefix_type_expr(&mut t.node, self.module_name, self.module_prog);
                }
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    if is_module_type(&arm.enum_name.node, self.module_prog) {
                        arm.enum_name.node = prefix_name(self.module_name, &arm.enum_name.node);
                    }
                    for ta in &mut arm.type_args {
                        prefix_type_expr(&mut ta.node, self.module_name, self.module_prog);
                    }
                }
            }
            Stmt::Raise { error_name, .. } => {
                if self.module_prog.errors.iter().any(|e| e.node.name.node == error_name.node) {
                    error_name.node = prefix_name(self.module_name, &error_name.node);
                }
            }
            Stmt::LetChan { elem_type, .. } => {
                prefix_type_expr(&mut elem_type.node, self.module_name, self.module_prog);
            }
            Stmt::Scope { bindings, .. } => {
                for binding in bindings {
                    prefix_type_expr(&mut binding.ty.node, self.module_name, self.module_prog);
                }
            }
            _ => {}
        }
        // Recurse into sub-statements
        walk_stmt_mut(self, stmt);
    }
}

fn rewrite_block_for_module(block: &mut Block, module_name: &str, module_prog: &Program) {
    let mut rewriter = ModuleRewriter {
        module_name,
        module_prog,
    };
    for stmt in &mut block.stmts {
        rewriter.visit_stmt_mut(stmt);
    }
}


/// Rewrite qualified references in the root program.
/// Converts MethodCall { object: Ident("module"), method, args } → Call { name: "module.method", args }
/// when "module" is a known import name.
/// Also rewrites declaration-level types (class fields, trait sigs, error fields, enum variant fields, app inject fields).
fn rewrite_program(program: &mut Program, import_names: &HashSet<String>) {
    for func in &mut program.functions {
        rewrite_function_body(&mut func.node, import_names);
    }
    for class in &mut program.classes {
        for method in &mut class.node.methods {
            rewrite_function_body(&mut method.node, import_names);
        }
        // Rewrite class field types
        for field in &mut class.node.fields {
            rewrite_type_expr(&mut field.ty, import_names);
        }
    }
    for tr in &mut program.traits {
        for method in &mut tr.node.methods {
            // Rewrite trait method param/return types
            for param in &mut method.params {
                rewrite_type_expr(&mut param.ty, import_names);
            }
            if let Some(ret) = &mut method.return_type {
                rewrite_type_expr(ret, import_names);
            }
            if let Some(body) = &mut method.body {
                rewrite_block(&mut body.node, import_names);
            }
        }
    }
    // Rewrite error field types
    for error in &mut program.errors {
        for field in &mut error.node.fields {
            rewrite_type_expr(&mut field.ty, import_names);
        }
    }
    // Rewrite enum variant field types
    for enum_decl in &mut program.enums {
        for variant in &mut enum_decl.node.variants {
            for field in &mut variant.fields {
                rewrite_type_expr(&mut field.ty, import_names);
            }
        }
    }
    if let Some(app) = &mut program.app {
        for method in &mut app.node.methods {
            rewrite_function_body(&mut method.node, import_names);
        }
        // Rewrite app inject field types
        for field in &mut app.node.inject_fields {
            rewrite_type_expr(&mut field.ty, import_names);
        }
    }
    for stage in &mut program.stages {
        for method in &mut stage.node.methods {
            rewrite_function_body(&mut method.node, import_names);
        }
        // Rewrite required method param/return types
        for req in &mut stage.node.required_methods {
            for param in &mut req.node.params {
                rewrite_type_expr(&mut param.ty, import_names);
            }
            if let Some(ret) = &mut req.node.return_type {
                rewrite_type_expr(ret, import_names);
            }
        }
        // Rewrite stage inject field types
        for field in &mut stage.node.inject_fields {
            rewrite_type_expr(&mut field.ty, import_names);
        }
    }
}

fn rewrite_function_body(func: &mut Function, import_names: &HashSet<String>) {
    // Rewrite qualified types in params
    for param in &mut func.params {
        rewrite_type_expr(&mut param.ty, import_names);
    }
    if let Some(ret) = &mut func.return_type {
        rewrite_type_expr(ret, import_names);
    }
    rewrite_block(&mut func.body.node, import_names);
}

fn rewrite_type_expr(ty: &mut Spanned<TypeExpr>, import_names: &HashSet<String>) {
    match &mut ty.node {
        TypeExpr::Qualified { module, name } => {
            if import_names.contains(module.as_str()) {
                ty.node = TypeExpr::Named(prefix_name(module, name));
            }
        }
        TypeExpr::Array(inner) => {
            rewrite_type_expr(inner, import_names);
        }
        TypeExpr::Named(_) => {}
        TypeExpr::Fn { params, return_type } => {
            for p in params {
                rewrite_type_expr(p, import_names);
            }
            rewrite_type_expr(return_type, import_names);
        }
        TypeExpr::Generic { name, type_args } => {
            // Check if the base name is a qualified type from an import
            if let Some(dot_pos) = name.find('.') {
                let module = &name[..dot_pos];
                if import_names.contains(module) {
                    // Already qualified, leave the name alone
                }
            }
            for arg in type_args {
                rewrite_type_expr(arg, import_names);
            }
        }
        TypeExpr::Nullable(inner) => {
            rewrite_type_expr(inner, import_names);
        }
        TypeExpr::Stream(inner) => {
            rewrite_type_expr(inner, import_names);
        }
    }
}

struct QualifiedAccessRewriter<'a> {
    import_names: &'a HashSet<String>,
}

impl VisitMut for QualifiedAccessRewriter<'_> {
    fn visit_expr_mut(&mut self, expr: &mut Spanned<Expr>) {
        // Handle expressions that need qualification rewriting
        match &mut expr.node {
            Expr::MethodCall { object, method, args } => {
                // Check if object is Ident matching an import name → convert to qualified call
                if let Expr::Ident(name) = &object.node
                    && self.import_names.contains(name.as_str())
                {
                    let qualified_name = prefix_name(name, &method.node);
                    let name_span = Span::new(object.span.start, method.span.end);
                    // Rewrite args first
                    for arg in args.iter_mut() {
                        self.visit_expr_mut(arg);
                    }
                    expr.node = Expr::Call {
                        name: Spanned::new(qualified_name, name_span),
                        args: std::mem::take(args),
                        type_args: vec![],
                        target_id: None,
                    };
                    return;
                }
            }
            Expr::FieldAccess { object, field } => {
                // Check for module-qualified enum access: status.State.Active
                // Pattern: FieldAccess { object: FieldAccess { object: Ident(module), field: enum_name }, field: variant }
                if let Expr::FieldAccess { object: inner_object, field: inner_field } = &object.node {
                    if let Expr::Ident(module_name) = &inner_object.node {
                        if self.import_names.contains(module_name.as_str()) {
                            // This is module.Enum.Variant - convert to EnumUnit
                            let qualified_enum_name = prefix_name(module_name, &inner_field.node);
                            let enum_name_span = Span::new(inner_object.span.start, inner_field.span.end);
                            let variant = field.clone();

                            expr.node = Expr::EnumUnit {
                                enum_name: Spanned::new(qualified_enum_name, enum_name_span),
                                variant,
                                type_args: vec![],
                                enum_id: None,
                                variant_id: None,
                            };
                            return;
                        }
                    }
                }
            }
            Expr::StructLit { type_args, .. } => {
                for ta in type_args {
                    rewrite_type_expr(ta, self.import_names);
                }
            }
            Expr::EnumUnit { type_args, .. } | Expr::EnumData { type_args, .. } => {
                for ta in type_args {
                    rewrite_type_expr(ta, self.import_names);
                }
            }
            Expr::MapLit { key_type, value_type, .. } => {
                rewrite_type_expr(key_type, self.import_names);
                rewrite_type_expr(value_type, self.import_names);
            }
            Expr::SetLit { elem_type, .. } => {
                rewrite_type_expr(elem_type, self.import_names);
            }
            Expr::Cast { target_type, .. } => {
                rewrite_type_expr(target_type, self.import_names);
            }
            Expr::StringInterp { parts } => {
                for part in parts {
                    if let StringInterpPart::Expr(e) = part {
                        self.visit_expr_mut(e);
                    }
                }
                return;
            }
            _ => {}
        }
        // Recurse into sub-expressions
        walk_expr_mut(self, expr);
    }

    fn visit_stmt_mut(&mut self, stmt: &mut Spanned<Stmt>) {
        // Handle statements that need type expression rewriting
        match &mut stmt.node {
            Stmt::Let { ty, .. } => {
                if let Some(t) = ty {
                    rewrite_type_expr(t, self.import_names);
                }
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    for ta in &mut arm.type_args {
                        rewrite_type_expr(ta, self.import_names);
                    }
                }
            }
            Stmt::LetChan { elem_type, .. } => {
                rewrite_type_expr(elem_type, self.import_names);
            }
            Stmt::Scope { bindings, .. } => {
                for binding in bindings {
                    rewrite_type_expr(&mut binding.ty, self.import_names);
                }
            }
            _ => {}
        }
        // Recurse into sub-statements
        walk_stmt_mut(self, stmt);
    }
}

fn rewrite_block(block: &mut Block, import_names: &HashSet<String>) {
    let mut rewriter = QualifiedAccessRewriter {
        import_names,
    };
    for stmt in &mut block.stmts {
        rewriter.visit_stmt_mut(stmt);
    }
}

/// Resolve QualifiedAccess nodes after module flattening.
/// If first segment is a module name, keep as QualifiedAccess for type checker (enum case).
/// Otherwise, convert to nested FieldAccess chain (variable.field.field case).
fn resolve_qualified_access_in_program(program: &mut Program, module_names: &HashSet<String>) {
    // Build mapping from unprefixed to prefixed enum names
    let mut enum_name_map: HashMap<String, String> = HashMap::new();
    for e in &program.enums {
        let full_name = e.node.name.node.clone();
        enum_name_map.insert(full_name.clone(), full_name.clone()); // Full name maps to itself

        // Also add unprefixed name if this is a module-qualified enum
        if let Some(dot_pos) = full_name.rfind('.') {
            let unprefixed = &full_name[dot_pos + 1..];
            enum_name_map.insert(unprefixed.to_string(), full_name.clone());
        }
    }

    for func in &mut program.functions {
        // Resolve in function contracts (requires)
        for contract in &mut func.node.contracts {
            resolve_qualified_access_in_expr(&mut contract.node.expr.node, contract.node.expr.span, module_names, &enum_name_map);
        }
        resolve_qualified_access_in_block(&mut func.node.body.node, module_names, &enum_name_map);
    }
    for class in &mut program.classes {
        // Resolve in class invariants
        for invariant in &mut class.node.invariants {
            resolve_qualified_access_in_expr(&mut invariant.node.expr.node, invariant.node.expr.span, module_names, &enum_name_map);
        }
        for method in &mut class.node.methods {
            // Resolve in method contracts (requires)
            for contract in &mut method.node.contracts {
                resolve_qualified_access_in_expr(&mut contract.node.expr.node, contract.node.expr.span, module_names, &enum_name_map);
            }
            resolve_qualified_access_in_block(&mut method.node.body.node, module_names, &enum_name_map);
        }
    }
    if let Some(app) = &mut program.app {
        for method in &mut app.node.methods {
            // Resolve in app method contracts
            for contract in &mut method.node.contracts {
                resolve_qualified_access_in_expr(&mut contract.node.expr.node, contract.node.expr.span, module_names, &enum_name_map);
            }
            resolve_qualified_access_in_block(&mut method.node.body.node, module_names, &enum_name_map);
        }
    }
    for stage in &mut program.stages {
        for method in &mut stage.node.methods {
            // Resolve in stage method contracts
            for contract in &mut method.node.contracts {
                resolve_qualified_access_in_expr(&mut contract.node.expr.node, contract.node.expr.span, module_names, &enum_name_map);
            }
            resolve_qualified_access_in_block(&mut method.node.body.node, module_names, &enum_name_map);
        }
    }
}

fn resolve_qualified_access_in_block(block: &mut Block, module_names: &HashSet<String>, enum_name_map: &HashMap<String, String>) {
    for stmt in &mut block.stmts {
        resolve_qualified_access_in_stmt(&mut stmt.node, module_names, enum_name_map);
    }
}

fn resolve_qualified_access_in_stmt(stmt: &mut Stmt, module_names: &HashSet<String>, enum_name_map: &HashMap<String, String>) {
    match stmt {
        Stmt::Let { value, .. } => {
            resolve_qualified_access_in_expr(&mut value.node, value.span, module_names, enum_name_map);
        }
        Stmt::Return(Some(expr)) => {
            resolve_qualified_access_in_expr(&mut expr.node, expr.span, module_names, enum_name_map);
        }
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => {
            resolve_qualified_access_in_expr(&mut value.node, value.span, module_names, enum_name_map);
        }
        Stmt::FieldAssign { object, value, .. } => {
            resolve_qualified_access_in_expr(&mut object.node, object.span, module_names, enum_name_map);
            resolve_qualified_access_in_expr(&mut value.node, value.span, module_names, enum_name_map);
        }
        Stmt::If { condition, then_block, else_block } => {
            resolve_qualified_access_in_expr(&mut condition.node, condition.span, module_names, enum_name_map);
            resolve_qualified_access_in_block(&mut then_block.node, module_names, enum_name_map);
            if let Some(eb) = else_block {
                resolve_qualified_access_in_block(&mut eb.node, module_names, enum_name_map);
            }
        }
        Stmt::While { condition, body } => {
            resolve_qualified_access_in_expr(&mut condition.node, condition.span, module_names, enum_name_map);
            resolve_qualified_access_in_block(&mut body.node, module_names, enum_name_map);
        }
        Stmt::For { iterable, body, .. } => {
            resolve_qualified_access_in_expr(&mut iterable.node, iterable.span, module_names, enum_name_map);
            resolve_qualified_access_in_block(&mut body.node, module_names, enum_name_map);
        }
        Stmt::IndexAssign { object, index, value } => {
            resolve_qualified_access_in_expr(&mut object.node, object.span, module_names, enum_name_map);
            resolve_qualified_access_in_expr(&mut index.node, index.span, module_names, enum_name_map);
            resolve_qualified_access_in_expr(&mut value.node, value.span, module_names, enum_name_map);
        }
        Stmt::Match { expr, arms } => {
            resolve_qualified_access_in_expr(&mut expr.node, expr.span, module_names, enum_name_map);
            for arm in arms {
                resolve_qualified_access_in_block(&mut arm.body.node, module_names, enum_name_map);
            }
        }
        Stmt::Raise { fields, .. } => {
            for (_, val) in fields {
                resolve_qualified_access_in_expr(&mut val.node, val.span, module_names, enum_name_map);
            }
        }
        Stmt::Expr(expr) => {
            resolve_qualified_access_in_expr(&mut expr.node, expr.span, module_names, enum_name_map);
        }
        Stmt::LetChan { capacity, .. } => {
            if let Some(cap) = capacity {
                resolve_qualified_access_in_expr(&mut cap.node, cap.span, module_names, enum_name_map);
            }
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &mut arm.op {
                    SelectOp::Recv { channel, .. } => {
                        resolve_qualified_access_in_expr(&mut channel.node, channel.span, module_names, enum_name_map);
                    }
                    SelectOp::Send { channel, value } => {
                        resolve_qualified_access_in_expr(&mut channel.node, channel.span, module_names, enum_name_map);
                        resolve_qualified_access_in_expr(&mut value.node, value.span, module_names, enum_name_map);
                    }
                }
                resolve_qualified_access_in_block(&mut arm.body.node, module_names, enum_name_map);
            }
            if let Some(def) = default {
                resolve_qualified_access_in_block(&mut def.node, module_names, enum_name_map);
            }
        }
        Stmt::Scope { seeds, body, .. } => {
            for seed in seeds {
                resolve_qualified_access_in_expr(&mut seed.node, seed.span, module_names, enum_name_map);
            }
            resolve_qualified_access_in_block(&mut body.node, module_names, enum_name_map);
        }
        Stmt::Yield { value, .. } => {
            resolve_qualified_access_in_expr(&mut value.node, value.span, module_names, enum_name_map);
        }
        Stmt::Assert { expr } => {
            resolve_qualified_access_in_expr(&mut expr.node, expr.span, module_names, enum_name_map);
        }
        Stmt::Break | Stmt::Continue => {}
    }
}

fn resolve_qualified_access_in_expr(expr: &mut Expr, span: Span, module_names: &HashSet<String>, enum_name_map: &HashMap<String, String>) {
    match expr {
        Expr::QualifiedAccess { segments } => {
            if segments.is_empty() {
                return;
            }

            // Check if first segment is a module name
            let is_module_reference = module_names.contains(&segments[0].node);

            if is_module_reference {
                // Convert module.Enum.Variant to EnumUnit
                if segments.len() == 3 {
                    let qualified_enum = format!("{}.{}", segments[0].node, segments[1].node);
                    let enum_span = Span::new(segments[0].span.start, segments[1].span.end);
                    *expr = Expr::EnumUnit {
                        enum_name: Spanned::new(qualified_enum, enum_span),
                        variant: segments[2].clone(),
                        type_args: vec![],
                        enum_id: None,
                        variant_id: None,
                    };
                    return;
                }
                // For 2-segment patterns (module.Type), keep as QualifiedAccess for type checking
                return;
            }

            // Check if 2-segment pattern is Enum.Variant
            // Map unprefixed enum names to their full names (e.g., Status -> src.Status)
            if segments.len() == 2 {
                if let Some(full_enum_name) = enum_name_map.get(&segments[0].node) {
                    *expr = Expr::EnumUnit {
                        enum_name: Spanned::new(full_enum_name.clone(), segments[0].span),
                        variant: segments[1].clone(),
                        type_args: vec![],
                        enum_id: None,
                        variant_id: None,
                    };
                    return;
                }
            }

            // Convert to nested FieldAccess chain (variable.field.field case)
            let mut current = Expr::Ident(segments[0].node.clone());
            let mut current_span = segments[0].span;

            for field_seg in &segments[1..] {
                let object_span = current_span;  // Save span before updating
                current_span = Span::new(current_span.start, field_seg.span.end);
                current = Expr::FieldAccess {
                    object: Box::new(Spanned::new(current, object_span)),
                    field: field_seg.clone(),
                };
            }

            *expr = current;
        }
        Expr::BinOp { lhs, rhs, .. } => {
            resolve_qualified_access_in_expr(&mut lhs.node, lhs.span, module_names, enum_name_map);
            resolve_qualified_access_in_expr(&mut rhs.node, rhs.span, module_names, enum_name_map);
        }
        Expr::UnaryOp { operand, .. } => {
            resolve_qualified_access_in_expr(&mut operand.node, operand.span, module_names, enum_name_map);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                resolve_qualified_access_in_expr(&mut arg.node, arg.span, module_names, enum_name_map);
            }
        }
        Expr::FieldAccess { object, .. } => {
            resolve_qualified_access_in_expr(&mut object.node, object.span, module_names, enum_name_map);
        }
        Expr::MethodCall { object, args, .. } => {
            resolve_qualified_access_in_expr(&mut object.node, object.span, module_names, enum_name_map);
            for arg in args {
                resolve_qualified_access_in_expr(&mut arg.node, arg.span, module_names, enum_name_map);
            }
        }
        Expr::StructLit { fields, .. } => {
            for (_, val) in fields {
                resolve_qualified_access_in_expr(&mut val.node, val.span, module_names, enum_name_map);
            }
        }
        Expr::ArrayLit { elements } => {
            for elem in elements {
                resolve_qualified_access_in_expr(&mut elem.node, elem.span, module_names, enum_name_map);
            }
        }
        Expr::Index { object, index } => {
            resolve_qualified_access_in_expr(&mut object.node, object.span, module_names, enum_name_map);
            resolve_qualified_access_in_expr(&mut index.node, index.span, module_names, enum_name_map);
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    resolve_qualified_access_in_expr(&mut e.node, e.span, module_names, enum_name_map);
                }
            }
        }
        Expr::EnumData { fields, .. } => {
            for (_, val) in fields {
                resolve_qualified_access_in_expr(&mut val.node, val.span, module_names, enum_name_map);
            }
        }
        Expr::Closure { body, .. } => {
            resolve_qualified_access_in_block(&mut body.node, module_names, enum_name_map);
        }
        Expr::Propagate { expr: inner } => {
            resolve_qualified_access_in_expr(&mut inner.node, inner.span, module_names, enum_name_map);
        }
        Expr::Catch { expr: inner, handler } => {
            resolve_qualified_access_in_expr(&mut inner.node, inner.span, module_names, enum_name_map);
            match handler {
                CatchHandler::Wildcard { body, .. } => {
                    resolve_qualified_access_in_block(&mut body.node, module_names, enum_name_map);
                }
                CatchHandler::Shorthand(fb) => {
                    resolve_qualified_access_in_expr(&mut fb.node, fb.span, module_names, enum_name_map);
                }
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                resolve_qualified_access_in_expr(&mut k.node, k.span, module_names, enum_name_map);
                resolve_qualified_access_in_expr(&mut v.node, v.span, module_names, enum_name_map);
            }
        }
        Expr::SetLit { elements, .. } => {
            for elem in elements {
                resolve_qualified_access_in_expr(&mut elem.node, elem.span, module_names, enum_name_map);
            }
        }
        Expr::Cast { expr: inner, .. } => {
            resolve_qualified_access_in_expr(&mut inner.node, inner.span, module_names, enum_name_map);
        }
        Expr::Range { start, end, .. } => {
            resolve_qualified_access_in_expr(&mut start.node, start.span, module_names, enum_name_map);
            resolve_qualified_access_in_expr(&mut end.node, end.span, module_names, enum_name_map);
        }
        Expr::Spawn { call } => {
            resolve_qualified_access_in_expr(&mut call.node, call.span, module_names, enum_name_map);
        }
        Expr::NullPropagate { expr: inner } => {
            resolve_qualified_access_in_expr(&mut inner.node, inner.span, module_names, enum_name_map);
        }
        Expr::StaticTraitCall { args, .. } => {
            for arg in args {
                resolve_qualified_access_in_expr(&mut arg.node, arg.span, module_names, enum_name_map);
            }
        }
        Expr::If { condition, then_block, else_block } => {
            resolve_qualified_access_in_expr(&mut condition.node, condition.span, module_names, enum_name_map);
            for stmt in &mut then_block.node.stmts {
                resolve_qualified_access_in_stmt(&mut stmt.node, module_names, enum_name_map);
            }
            for stmt in &mut else_block.node.stmts {
                resolve_qualified_access_in_stmt(&mut stmt.node, module_names, enum_name_map);
            }
        }
        Expr::Match { expr, arms } => {
            resolve_qualified_access_in_expr(&mut expr.node, expr.span, module_names, enum_name_map);
            for arm in arms {
                resolve_qualified_access_in_expr(&mut arm.value.node, arm.value.span, module_names, enum_name_map);
            }
        }
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_)
        | Expr::Ident(_) | Expr::EnumUnit { .. } | Expr::ClosureCreate { .. } | Expr::NoneLit => {}
    }
    let _ = span;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{TypeExpr, ExternFnDecl, Param};
    use crate::span::{Span, Spanned};

    fn mk_span() -> Span {
        Span { start: 0, end: 0, file_id: 0 }
    }

    fn spanned<T>(node: T) -> Spanned<T> {
        Spanned { node, span: mk_span() }
    }

    // ===== type_expr_eq tests =====

    #[test]
    fn test_type_expr_eq_named_same() {
        let a = TypeExpr::Named("int".to_string());
        let b = TypeExpr::Named("int".to_string());
        assert!(type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_named_different() {
        let a = TypeExpr::Named("int".to_string());
        let b = TypeExpr::Named("string".to_string());
        assert!(!type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_array_same() {
        let a = TypeExpr::Array(Box::new(spanned(TypeExpr::Named("int".to_string()))));
        let b = TypeExpr::Array(Box::new(spanned(TypeExpr::Named("int".to_string()))));
        assert!(type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_array_different_element() {
        let a = TypeExpr::Array(Box::new(spanned(TypeExpr::Named("int".to_string()))));
        let b = TypeExpr::Array(Box::new(spanned(TypeExpr::Named("string".to_string()))));
        assert!(!type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_nested_array() {
        let a = TypeExpr::Array(Box::new(spanned(TypeExpr::Array(Box::new(spanned(
            TypeExpr::Named("int".to_string())
        ))))));
        let b = TypeExpr::Array(Box::new(spanned(TypeExpr::Array(Box::new(spanned(
            TypeExpr::Named("int".to_string())
        ))))));
        assert!(type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_qualified_same() {
        let a = TypeExpr::Qualified {
            module: "math".to_string(),
            name: "Vector".to_string(),
        };
        let b = TypeExpr::Qualified {
            module: "math".to_string(),
            name: "Vector".to_string(),
        };
        assert!(type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_qualified_different_module() {
        let a = TypeExpr::Qualified {
            module: "math".to_string(),
            name: "Vector".to_string(),
        };
        let b = TypeExpr::Qualified {
            module: "geometry".to_string(),
            name: "Vector".to_string(),
        };
        assert!(!type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_qualified_different_name() {
        let a = TypeExpr::Qualified {
            module: "math".to_string(),
            name: "Vector".to_string(),
        };
        let b = TypeExpr::Qualified {
            module: "math".to_string(),
            name: "Matrix".to_string(),
        };
        assert!(!type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_fn_same() {
        let a = TypeExpr::Fn {
            params: vec![
                Box::new(spanned(TypeExpr::Named("int".to_string()))),
                Box::new(spanned(TypeExpr::Named("string".to_string()))),
            ],
            return_type: Box::new(spanned(TypeExpr::Named("bool".to_string()))),
        };
        let b = TypeExpr::Fn {
            params: vec![
                Box::new(spanned(TypeExpr::Named("int".to_string()))),
                Box::new(spanned(TypeExpr::Named("string".to_string()))),
            ],
            return_type: Box::new(spanned(TypeExpr::Named("bool".to_string()))),
        };
        assert!(type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_fn_different_param_count() {
        let a = TypeExpr::Fn {
            params: vec![Box::new(spanned(TypeExpr::Named("int".to_string())))],
            return_type: Box::new(spanned(TypeExpr::Named("bool".to_string()))),
        };
        let b = TypeExpr::Fn {
            params: vec![
                Box::new(spanned(TypeExpr::Named("int".to_string()))),
                Box::new(spanned(TypeExpr::Named("string".to_string()))),
            ],
            return_type: Box::new(spanned(TypeExpr::Named("bool".to_string()))),
        };
        assert!(!type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_fn_different_param_type() {
        let a = TypeExpr::Fn {
            params: vec![Box::new(spanned(TypeExpr::Named("int".to_string())))],
            return_type: Box::new(spanned(TypeExpr::Named("bool".to_string()))),
        };
        let b = TypeExpr::Fn {
            params: vec![Box::new(spanned(TypeExpr::Named("float".to_string())))],
            return_type: Box::new(spanned(TypeExpr::Named("bool".to_string()))),
        };
        assert!(!type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_fn_different_return_type() {
        let a = TypeExpr::Fn {
            params: vec![Box::new(spanned(TypeExpr::Named("int".to_string())))],
            return_type: Box::new(spanned(TypeExpr::Named("bool".to_string()))),
        };
        let b = TypeExpr::Fn {
            params: vec![Box::new(spanned(TypeExpr::Named("int".to_string())))],
            return_type: Box::new(spanned(TypeExpr::Named("string".to_string()))),
        };
        assert!(!type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_generic_same() {
        let a = TypeExpr::Generic {
            name: "Box".to_string(),
            type_args: vec![spanned(TypeExpr::Named("int".to_string()))],
        };
        let b = TypeExpr::Generic {
            name: "Box".to_string(),
            type_args: vec![spanned(TypeExpr::Named("int".to_string()))],
        };
        assert!(type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_generic_different_name() {
        let a = TypeExpr::Generic {
            name: "Box".to_string(),
            type_args: vec![spanned(TypeExpr::Named("int".to_string()))],
        };
        let b = TypeExpr::Generic {
            name: "Container".to_string(),
            type_args: vec![spanned(TypeExpr::Named("int".to_string()))],
        };
        assert!(!type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_generic_different_arg_count() {
        let a = TypeExpr::Generic {
            name: "Pair".to_string(),
            type_args: vec![spanned(TypeExpr::Named("int".to_string()))],
        };
        let b = TypeExpr::Generic {
            name: "Pair".to_string(),
            type_args: vec![
                spanned(TypeExpr::Named("int".to_string())),
                spanned(TypeExpr::Named("string".to_string())),
            ],
        };
        assert!(!type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_generic_different_arg_type() {
        let a = TypeExpr::Generic {
            name: "Box".to_string(),
            type_args: vec![spanned(TypeExpr::Named("int".to_string()))],
        };
        let b = TypeExpr::Generic {
            name: "Box".to_string(),
            type_args: vec![spanned(TypeExpr::Named("string".to_string()))],
        };
        assert!(!type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_nullable_same() {
        let a = TypeExpr::Nullable(Box::new(spanned(TypeExpr::Named("int".to_string()))));
        let b = TypeExpr::Nullable(Box::new(spanned(TypeExpr::Named("int".to_string()))));
        assert!(type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_nullable_different() {
        let a = TypeExpr::Nullable(Box::new(spanned(TypeExpr::Named("int".to_string()))));
        let b = TypeExpr::Nullable(Box::new(spanned(TypeExpr::Named("string".to_string()))));
        assert!(!type_expr_eq(&a, &b));
    }

    #[test]
    fn test_type_expr_eq_different_variants() {
        let a = TypeExpr::Named("int".to_string());
        let b = TypeExpr::Array(Box::new(spanned(TypeExpr::Named("int".to_string()))));
        assert!(!type_expr_eq(&a, &b));
    }

    // ===== extern_fn_sigs_match tests =====

    #[test]
    fn test_extern_fn_sigs_match_same_no_params() {
        let a = ExternFnDecl {
            name: spanned("foo".to_string()),
            params: vec![],
            return_type: None,
            is_pub: false,
        };
        let b = ExternFnDecl {
            name: spanned("foo".to_string()),
            params: vec![],
            return_type: None,
            is_pub: false,
        };
        assert!(extern_fn_sigs_match(&a, &b));
    }

    #[test]
    fn test_extern_fn_sigs_match_same_with_params() {
        use uuid::Uuid;

        let a = ExternFnDecl {
            name: spanned("foo".to_string()),
            params: vec![
                Param {
                    id: Uuid::new_v4(),
                    name: spanned("x".to_string()),
                    ty: spanned(TypeExpr::Named("int".to_string())),
                    is_mut: false,
                },
                Param {
                    id: Uuid::new_v4(),
                    name: spanned("y".to_string()),
                    ty: spanned(TypeExpr::Named("string".to_string())),
                    is_mut: false,
                },
            ],
            return_type: Some(spanned(TypeExpr::Named("bool".to_string()))),
            is_pub: false,
        };
        let b = ExternFnDecl {
            name: spanned("foo".to_string()),
            params: vec![
                Param {
                    id: Uuid::new_v4(),
                    name: spanned("a".to_string()),
                    ty: spanned(TypeExpr::Named("int".to_string())),
                    is_mut: false,
                },
                Param {
                    id: Uuid::new_v4(),
                    name: spanned("b".to_string()),
                    ty: spanned(TypeExpr::Named("string".to_string())),
                    is_mut: false,
                },
            ],
            return_type: Some(spanned(TypeExpr::Named("bool".to_string()))),
            is_pub: false,
        };
        assert!(extern_fn_sigs_match(&a, &b));
    }

    #[test]
    fn test_extern_fn_sigs_match_different_param_count() {
        use uuid::Uuid;

        let a = ExternFnDecl {
            name: spanned("foo".to_string()),
            params: vec![Param {
                id: Uuid::new_v4(),
                name: spanned("x".to_string()),
                ty: spanned(TypeExpr::Named("int".to_string())),
                is_mut: false,
            }],
            return_type: None,
            is_pub: false,
        };
        let b = ExternFnDecl {
            name: spanned("foo".to_string()),
            params: vec![],
            return_type: None,
            is_pub: false,
        };
        assert!(!extern_fn_sigs_match(&a, &b));
    }

    #[test]
    fn test_extern_fn_sigs_match_different_param_type() {
        use uuid::Uuid;

        let a = ExternFnDecl {
            name: spanned("foo".to_string()),
            params: vec![Param {
                id: Uuid::new_v4(),
                name: spanned("x".to_string()),
                ty: spanned(TypeExpr::Named("int".to_string())),
                is_mut: false,
            }],
            return_type: None,
            is_pub: false,
        };
        let b = ExternFnDecl {
            name: spanned("foo".to_string()),
            params: vec![Param {
                id: Uuid::new_v4(),
                name: spanned("x".to_string()),
                ty: spanned(TypeExpr::Named("float".to_string())),
                is_mut: false,
            }],
            return_type: None,
            is_pub: false,
        };
        assert!(!extern_fn_sigs_match(&a, &b));
    }

    #[test]
    fn test_extern_fn_sigs_match_different_return_type() {
        let a = ExternFnDecl {
            name: spanned("foo".to_string()),
            params: vec![],
            return_type: Some(spanned(TypeExpr::Named("int".to_string()))),
            is_pub: false,
        };
        let b = ExternFnDecl {
            name: spanned("foo".to_string()),
            params: vec![],
            return_type: Some(spanned(TypeExpr::Named("string".to_string()))),
            is_pub: false,
        };
        assert!(!extern_fn_sigs_match(&a, &b));
    }

    #[test]
    fn test_extern_fn_sigs_match_one_has_return_other_doesnt() {
        let a = ExternFnDecl {
            name: spanned("foo".to_string()),
            params: vec![],
            return_type: Some(spanned(TypeExpr::Named("int".to_string()))),
            is_pub: false,
        };
        let b = ExternFnDecl {
            name: spanned("foo".to_string()),
            params: vec![],
            return_type: None,
            is_pub: false,
        };
        assert!(!extern_fn_sigs_match(&a, &b));
    }
}
