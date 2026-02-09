use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::diagnostics::CompileError;
use crate::lexer;
use crate::parser::ast::*;
use crate::parser::Parser;
use crate::span::{Span, Spanned};

/// Maps file_id -> (path, source_text).
pub struct SourceMap {
    pub files: Vec<(PathBuf, String)>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self { files: Vec::new() }
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

/// Result of module resolution before flattening.
pub struct ModuleGraph {
    pub root: Program,
    pub imports: Vec<(String, Program)>, // (module_name, parsed program)
    pub source_map: SourceMap,
}

/// Load and parse a single .pluto file, assigning spans with the given file_id.
fn load_and_parse(path: &Path, source_map: &mut SourceMap) -> Result<(Program, u32), CompileError> {
    let source = std::fs::read_to_string(path).map_err(|e| {
        CompileError::codegen(format!("could not read '{}': {e}", path.display()))
    })?;
    let file_id = source_map.add_file(path.to_path_buf(), source.clone());
    let tokens = lexer::lex(&source)?;
    let mut parser = Parser::new(&tokens, &source);
    let program = parser.parse_program()?;
    Ok((program, file_id))
}

/// Load all .pluto files in a directory and merge into one Program.
fn load_directory_module(dir: &Path, source_map: &mut SourceMap) -> Result<Program, CompileError> {
    let mut merged = Program {
        imports: Vec::new(),
        functions: Vec::new(),
        classes: Vec::new(),
        traits: Vec::new(),
        enums: Vec::new(),
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
        merged.classes.extend(program.classes);
        merged.traits.extend(program.traits);
        merged.enums.extend(program.enums);
        // Inner imports not supported in v1
        if !program.imports.is_empty() {
            return Err(CompileError::codegen(format!(
                "transitive imports not supported: '{}' contains imports",
                file_path.display()
            )));
        }
    }

    Ok(merged)
}

/// Resolve all modules referenced by the entry file.
///
/// 1. Parse entry file to discover imports
/// 2. Load sibling .pluto files (excluding imported single-file modules) and merge into root
/// 3. For each import, find `<name>/` directory or `<name>.pluto` and load as a separate module
pub fn resolve_modules(entry_file: &Path) -> Result<ModuleGraph, CompileError> {
    let entry_file = entry_file.canonicalize().map_err(|e| {
        CompileError::codegen(format!("could not resolve path '{}': {e}", entry_file.display()))
    })?;
    let entry_dir = entry_file.parent().ok_or_else(|| {
        CompileError::codegen("entry file has no parent directory")
    })?;

    let mut source_map = SourceMap::new();

    // First, parse the entry file to discover imports
    let (entry_prog, _entry_file_id) = load_and_parse(&entry_file, &mut source_map)?;

    // Collect import names to know which sibling .pluto files are imported modules
    let import_names: HashSet<String> = entry_prog.imports.iter()
        .map(|i| i.node.module_name.node.clone())
        .collect();

    // Start root with the entry file's contents
    let mut root = entry_prog;

    // Load sibling .pluto files (excluding the entry file and imported single-file modules)
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
        // Skip files that match an import name (they'll be loaded as modules)
        if import_names.contains(stem) {
            continue;
        }
        let (program, _file_id) = load_and_parse(file_path, &mut source_map)?;
        // Merge sibling's imports into root (they might also have imports)
        root.imports.extend(program.imports);
        root.functions.extend(program.functions);
        root.classes.extend(program.classes);
        root.traits.extend(program.traits);
        root.enums.extend(program.enums);
    }

    // Resolve each import
    let mut imports = Vec::new();
    let mut imported_names = HashSet::new();

    for import in &root.imports {
        let module_name = &import.node.module_name.node;
        if !imported_names.insert(module_name.clone()) {
            continue; // skip duplicate imports
        }

        // Try directory first: <entry_dir>/<name>/
        let dir_path = entry_dir.join(module_name);
        let file_path = entry_dir.join(format!("{}.pluto", module_name));

        if dir_path.is_dir() {
            let module_prog = load_directory_module(&dir_path, &mut source_map)?;
            imports.push((module_name.clone(), module_prog));
        } else if file_path.is_file() {
            let (module_prog, _) = load_and_parse(&file_path, &mut source_map)?;
            if !module_prog.imports.is_empty() {
                return Err(CompileError::codegen(format!(
                    "transitive imports not supported: '{}' contains imports",
                    file_path.display()
                )));
            }
            imports.push((module_name.clone(), module_prog));
        } else {
            return Err(CompileError::syntax(
                format!("cannot find module '{}': no directory or file found", module_name),
                import.node.module_name.span,
            ));
        }
    }

    Ok(ModuleGraph { root, imports, source_map })
}

/// Flatten imported modules into the root program by prefixing names.
///
/// For each imported module:
/// - Add its `pub` items with prefixed names (e.g., `math`'s `add` → `math.add`)
/// - Rewrite qualified references in the root program's AST
pub fn flatten_modules(mut graph: ModuleGraph) -> Result<(Program, SourceMap), CompileError> {
    let import_names: HashSet<String> = graph.imports.iter().map(|(n, _)| n.clone()).collect();

    // Add prefixed items from imports
    for (module_name, module_prog) in &graph.imports {
        // Add pub functions with prefixed names
        for func in &module_prog.functions {
            if func.node.is_pub {
                let mut prefixed_func = func.clone();
                prefixed_func.node.name.node = format!("{}.{}", module_name, func.node.name.node);
                // Prefix types in params and return type that reference module-internal classes
                prefix_function_types(&mut prefixed_func.node, module_name, module_prog);
                graph.root.functions.push(prefixed_func);
            }
        }

        // Add pub classes with prefixed names
        for class in &module_prog.classes {
            if class.node.is_pub {
                let mut prefixed_class = class.clone();
                prefixed_class.node.name.node = format!("{}.{}", module_name, class.node.name.node);
                // Prefix field types that reference module-internal classes
                for field in &mut prefixed_class.node.fields {
                    prefix_type_expr(&mut field.ty.node, module_name, module_prog);
                }
                // Prefix method params/return types and names
                for method in &mut prefixed_class.node.methods {
                    prefix_function_types(&mut method.node, module_name, module_prog);
                }
                // Prefix trait names
                for trait_name in &mut prefixed_class.node.impl_traits {
                    if module_prog.traits.iter().any(|t| t.node.name.node == trait_name.node) {
                        trait_name.node = format!("{}.{}", module_name, trait_name.node);
                    }
                }
                graph.root.classes.push(prefixed_class);
            }
        }

        // Add pub traits with prefixed names
        for tr in &module_prog.traits {
            if tr.node.is_pub {
                let mut prefixed_trait = tr.clone();
                prefixed_trait.node.name.node = format!("{}.{}", module_name, tr.node.name.node);
                // Prefix types in method signatures
                for method in &mut prefixed_trait.node.methods {
                    for param in &mut method.params {
                        prefix_type_expr(&mut param.ty.node, module_name, module_prog);
                    }
                    if let Some(ret) = &mut method.return_type {
                        prefix_type_expr(&mut ret.node, module_name, module_prog);
                    }
                }
                graph.root.traits.push(prefixed_trait);
            }
        }

        // Also add non-pub classes/traits/functions that are needed internally
        // (for now, import ALL items from the module but mark non-pub ones)
        // Actually, for v1, we only expose pub items. Internal items stay internal.
        // If a pub function references a non-pub type, that would be a user error.
    }

    // Rewrite qualified references in root program's AST
    rewrite_program(&mut graph.root, &import_names);

    // Clear imports since they've been flattened
    graph.root.imports.clear();

    Ok((graph.root, graph.source_map))
}

/// Check if a type name refers to a class or trait defined in the given module.
fn is_module_type(name: &str, module_prog: &Program) -> bool {
    module_prog.classes.iter().any(|c| c.node.name.node == name)
        || module_prog.traits.iter().any(|t| t.node.name.node == name)
}

/// Prefix type expressions that reference module-internal types.
fn prefix_type_expr(ty: &mut TypeExpr, module_name: &str, module_prog: &Program) {
    match ty {
        TypeExpr::Named(name) => {
            if is_module_type(name, module_prog) {
                *name = format!("{}.{}", module_name, name);
            }
        }
        TypeExpr::Array(inner) => {
            prefix_type_expr(&mut inner.node, module_name, module_prog);
        }
        TypeExpr::Qualified { .. } => {
            // Already qualified, leave alone
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
fn rewrite_block_for_module(block: &mut Block, module_name: &str, module_prog: &Program) {
    for stmt in &mut block.stmts {
        rewrite_stmt_for_module(&mut stmt.node, module_name, module_prog);
    }
}

fn rewrite_stmt_for_module(stmt: &mut Stmt, module_name: &str, module_prog: &Program) {
    match stmt {
        Stmt::Let { ty, value, .. } => {
            if let Some(t) = ty {
                prefix_type_expr(&mut t.node, module_name, module_prog);
            }
            rewrite_expr_for_module(&mut value.node, module_name, module_prog);
        }
        Stmt::Return(Some(expr)) => {
            rewrite_expr_for_module(&mut expr.node, module_name, module_prog);
        }
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => {
            rewrite_expr_for_module(&mut value.node, module_name, module_prog);
        }
        Stmt::FieldAssign { object, value, .. } => {
            rewrite_expr_for_module(&mut object.node, module_name, module_prog);
            rewrite_expr_for_module(&mut value.node, module_name, module_prog);
        }
        Stmt::If { condition, then_block, else_block } => {
            rewrite_expr_for_module(&mut condition.node, module_name, module_prog);
            rewrite_block_for_module(&mut then_block.node, module_name, module_prog);
            if let Some(eb) = else_block {
                rewrite_block_for_module(&mut eb.node, module_name, module_prog);
            }
        }
        Stmt::While { condition, body } => {
            rewrite_expr_for_module(&mut condition.node, module_name, module_prog);
            rewrite_block_for_module(&mut body.node, module_name, module_prog);
        }
        Stmt::For { iterable, body, .. } => {
            rewrite_expr_for_module(&mut iterable.node, module_name, module_prog);
            rewrite_block_for_module(&mut body.node, module_name, module_prog);
        }
        Stmt::IndexAssign { object, index, value } => {
            rewrite_expr_for_module(&mut object.node, module_name, module_prog);
            rewrite_expr_for_module(&mut index.node, module_name, module_prog);
            rewrite_expr_for_module(&mut value.node, module_name, module_prog);
        }
        Stmt::Match { expr, arms } => {
            rewrite_expr_for_module(&mut expr.node, module_name, module_prog);
            for arm in arms {
                rewrite_block_for_module(&mut arm.body.node, module_name, module_prog);
            }
        }
        Stmt::Expr(expr) => {
            rewrite_expr_for_module(&mut expr.node, module_name, module_prog);
        }
    }
}

fn rewrite_expr_for_module(expr: &mut Expr, module_name: &str, module_prog: &Program) {
    match expr {
        Expr::Call { name, args } => {
            // Prefix calls to module-internal functions
            if module_prog.functions.iter().any(|f| f.node.name.node == name.node) {
                name.node = format!("{}.{}", module_name, name.node);
            }
            for arg in args {
                rewrite_expr_for_module(&mut arg.node, module_name, module_prog);
            }
        }
        Expr::StructLit { name, fields } => {
            if is_module_type(&name.node, module_prog) {
                name.node = format!("{}.{}", module_name, name.node);
            }
            for (_, val) in fields {
                rewrite_expr_for_module(&mut val.node, module_name, module_prog);
            }
        }
        Expr::MethodCall { object, args, .. } => {
            rewrite_expr_for_module(&mut object.node, module_name, module_prog);
            for arg in args {
                rewrite_expr_for_module(&mut arg.node, module_name, module_prog);
            }
        }
        Expr::FieldAccess { object, .. } => {
            rewrite_expr_for_module(&mut object.node, module_name, module_prog);
        }
        Expr::BinOp { lhs, rhs, .. } => {
            rewrite_expr_for_module(&mut lhs.node, module_name, module_prog);
            rewrite_expr_for_module(&mut rhs.node, module_name, module_prog);
        }
        Expr::UnaryOp { operand, .. } => {
            rewrite_expr_for_module(&mut operand.node, module_name, module_prog);
        }
        Expr::ArrayLit { elements } => {
            for elem in elements {
                rewrite_expr_for_module(&mut elem.node, module_name, module_prog);
            }
        }
        Expr::Index { object, index } => {
            rewrite_expr_for_module(&mut object.node, module_name, module_prog);
            rewrite_expr_for_module(&mut index.node, module_name, module_prog);
        }
        Expr::EnumUnit { enum_name, .. } => {
            if is_module_type(&enum_name.node, module_prog) {
                enum_name.node = format!("{}.{}", module_name, enum_name.node);
            }
        }
        Expr::EnumData { enum_name, fields, .. } => {
            if is_module_type(&enum_name.node, module_prog) {
                enum_name.node = format!("{}.{}", module_name, enum_name.node);
            }
            for (_, val) in fields {
                rewrite_expr_for_module(&mut val.node, module_name, module_prog);
            }
        }
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_) | Expr::Ident(_) => {}
    }
}

/// Rewrite qualified references in the root program.
/// Converts MethodCall { object: Ident("module"), method, args } → Call { name: "module.method", args }
/// when "module" is a known import name.
fn rewrite_program(program: &mut Program, import_names: &HashSet<String>) {
    for func in &mut program.functions {
        rewrite_function_body(&mut func.node, import_names);
    }
    for class in &mut program.classes {
        for method in &mut class.node.methods {
            rewrite_function_body(&mut method.node, import_names);
        }
    }
    for tr in &mut program.traits {
        for method in &mut tr.node.methods {
            if let Some(body) = &mut method.body {
                rewrite_block(&mut body.node, import_names);
            }
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
                ty.node = TypeExpr::Named(format!("{}.{}", module, name));
            }
        }
        TypeExpr::Array(inner) => {
            rewrite_type_expr(inner, import_names);
        }
        TypeExpr::Named(_) => {}
    }
}

fn rewrite_block(block: &mut Block, import_names: &HashSet<String>) {
    for stmt in &mut block.stmts {
        rewrite_stmt(&mut stmt.node, import_names);
    }
}

fn rewrite_stmt(stmt: &mut Stmt, import_names: &HashSet<String>) {
    match stmt {
        Stmt::Let { ty, value, .. } => {
            if let Some(t) = ty {
                rewrite_type_expr(t, import_names);
            }
            rewrite_expr(&mut value.node, value.span, import_names);
        }
        Stmt::Return(Some(expr)) => {
            rewrite_expr(&mut expr.node, expr.span, import_names);
        }
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => {
            rewrite_expr(&mut value.node, value.span, import_names);
        }
        Stmt::FieldAssign { object, value, .. } => {
            rewrite_expr(&mut object.node, object.span, import_names);
            rewrite_expr(&mut value.node, value.span, import_names);
        }
        Stmt::If { condition, then_block, else_block } => {
            rewrite_expr(&mut condition.node, condition.span, import_names);
            rewrite_block(&mut then_block.node, import_names);
            if let Some(eb) = else_block {
                rewrite_block(&mut eb.node, import_names);
            }
        }
        Stmt::While { condition, body } => {
            rewrite_expr(&mut condition.node, condition.span, import_names);
            rewrite_block(&mut body.node, import_names);
        }
        Stmt::For { iterable, body, .. } => {
            rewrite_expr(&mut iterable.node, iterable.span, import_names);
            rewrite_block(&mut body.node, import_names);
        }
        Stmt::IndexAssign { object, index, value } => {
            rewrite_expr(&mut object.node, object.span, import_names);
            rewrite_expr(&mut index.node, index.span, import_names);
            rewrite_expr(&mut value.node, value.span, import_names);
        }
        Stmt::Match { expr, arms } => {
            rewrite_expr(&mut expr.node, expr.span, import_names);
            for arm in arms {
                rewrite_block(&mut arm.body.node, import_names);
            }
        }
        Stmt::Expr(expr) => {
            rewrite_expr(&mut expr.node, expr.span, import_names);
        }
    }
}

fn rewrite_expr(expr: &mut Expr, span: Span, import_names: &HashSet<String>) {
    match expr {
        Expr::MethodCall { object, method, args } => {
            // Check if object is Ident matching an import name → convert to qualified call
            if let Expr::Ident(name) = &object.node {
                if import_names.contains(name.as_str()) {
                    let qualified_name = format!("{}.{}", name, method.node);
                    let name_span = Span::new(object.span.start, method.span.end);
                    // Rewrite args first
                    for arg in args.iter_mut() {
                        rewrite_expr(&mut arg.node, arg.span, import_names);
                    }
                    *expr = Expr::Call {
                        name: Spanned::new(qualified_name, name_span),
                        args: std::mem::take(args),
                    };
                    return;
                }
            }
            rewrite_expr(&mut object.node, object.span, import_names);
            for arg in args {
                rewrite_expr(&mut arg.node, arg.span, import_names);
            }
        }
        Expr::FieldAccess { object, field } => {
            // Check if this is module.Type (which wasn't followed by { for struct lit)
            // This could be used in other contexts; leave as-is for now, it will be
            // handled by the type checker if needed
            rewrite_expr(&mut object.node, object.span, import_names);
            let _ = field;
        }
        Expr::Call { name, args } => {
            // name might already be qualified (e.g., "math.add") from struct lit parsing
            for arg in args {
                rewrite_expr(&mut arg.node, arg.span, import_names);
            }
            let _ = name;
        }
        Expr::StructLit { fields, .. } => {
            // name is already qualified from parser (e.g., "math.Point")
            for (_, val) in fields {
                rewrite_expr(&mut val.node, val.span, import_names);
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            rewrite_expr(&mut lhs.node, lhs.span, import_names);
            rewrite_expr(&mut rhs.node, rhs.span, import_names);
        }
        Expr::UnaryOp { operand, .. } => {
            rewrite_expr(&mut operand.node, operand.span, import_names);
        }
        Expr::ArrayLit { elements } => {
            for elem in elements {
                rewrite_expr(&mut elem.node, elem.span, import_names);
            }
        }
        Expr::Index { object, index } => {
            rewrite_expr(&mut object.node, object.span, import_names);
            rewrite_expr(&mut index.node, index.span, import_names);
        }
        Expr::EnumUnit { enum_name, .. } => {
            // If enum_name is a known import, prefix it
            // (This will be handled when module flattening adds the enum)
        }
        Expr::EnumData { fields, .. } => {
            for (_, val) in fields {
                rewrite_expr(&mut val.node, val.span, import_names);
            }
        }
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_) | Expr::Ident(_) => {}
    }
    let _ = span;
}
