use std::collections::HashSet;

use uuid::Uuid;

use plutoc::parser::ast::*;
use plutoc::span::{Span, Spanned};

use crate::error::SdkError;
use crate::index::ModuleIndex;
use crate::module::Module;

/// Result of deleting a declaration.
pub struct DeleteResult {
    /// The pretty-printed source of the deleted declaration.
    pub source: String,
    /// References that now dangle (best-effort from current xref state).
    pub dangling: Vec<DanglingRef>,
}

/// A reference that dangles after a deletion.
#[derive(Debug)]
pub struct DanglingRef {
    /// What kind of reference this is.
    pub kind: DanglingRefKind,
    /// The name used at the reference site.
    pub name: String,
    /// The span of the reference site.
    pub span: Span,
}

/// The kind of dangling reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DanglingRefKind {
    Call,
    StructLit,
    EnumUsage,
    Raise,
    MatchArm,
    TypeRef,
}

/// Mutable editor for a Module's Program AST.
///
/// Created via `Module::edit()`. Mutations accumulate on the AST in memory.
/// Call `commit()` to pretty-print, re-resolve xrefs, rebuild the index,
/// and produce a new `Module`.
pub struct ModuleEditor {
    program: Program,
    source: String,
}

impl ModuleEditor {
    pub(crate) fn new(program: Program, source: String) -> Self {
        Self { program, source }
    }

    /// Parse source text as a single top-level declaration and append it to the Program.
    /// Returns the UUID of the newly added declaration.
    pub fn add_from_source(&mut self, source: &str) -> Result<Uuid, SdkError> {
        let mut program = parse_single_program(source, &self.program)?;

        // Exactly one declaration expected
        let count = program.functions.len()
            + program.classes.len()
            + program.enums.len()
            + program.traits.len()
            + program.errors.len()
            + program.app.iter().len();

        if count == 0 {
            return Err(SdkError::Edit("source contains no declarations".to_string()));
        }
        if count > 1 {
            return Err(SdkError::Edit("source contains multiple declarations; expected exactly one".to_string()));
        }

        if let Some(f) = program.functions.pop() {
            let id = f.node.id;
            self.program.functions.push(f);
            return Ok(id);
        }
        if let Some(c) = program.classes.pop() {
            let id = c.node.id;
            self.program.classes.push(c);
            return Ok(id);
        }
        if let Some(e) = program.enums.pop() {
            let id = e.node.id;
            self.program.enums.push(e);
            return Ok(id);
        }
        if let Some(t) = program.traits.pop() {
            let id = t.node.id;
            self.program.traits.push(t);
            return Ok(id);
        }
        if let Some(e) = program.errors.pop() {
            let id = e.node.id;
            self.program.errors.push(e);
            return Ok(id);
        }
        if let Some(a) = program.app.take() {
            let id = a.node.id;
            if self.program.app.is_some() {
                return Err(SdkError::Edit("program already has an app declaration".to_string()));
            }
            self.program.app = Some(a);
            return Ok(id);
        }

        Err(SdkError::Edit("could not extract declaration from source".to_string()))
    }

    /// Parse source text containing one or more top-level declarations and append them all.
    /// Returns the UUIDs of all newly added declarations.
    pub fn add_many_from_source(&mut self, source: &str) -> Result<Vec<Uuid>, SdkError> {
        let mut program = parse_single_program(source, &self.program)?;

        let count = program.functions.len()
            + program.classes.len()
            + program.enums.len()
            + program.traits.len()
            + program.errors.len()
            + program.app.iter().len();

        if count == 0 {
            return Err(SdkError::Edit("source contains no declarations".to_string()));
        }

        // Merge any imports from the input source into the program.
        merge_imports(&mut self.program.imports, &program.imports);

        // Merge test metadata so the pretty printer can reconstruct test blocks.
        self.program.test_info.append(&mut program.test_info);
        if program.tests.is_some() && self.program.tests.is_none() {
            self.program.tests = program.tests.take();
        }

        let mut ids = Vec::new();

        for f in program.functions.drain(..) {
            ids.push(f.node.id);
            self.program.functions.push(f);
        }
        for c in program.classes.drain(..) {
            ids.push(c.node.id);
            self.program.classes.push(c);
        }
        for e in program.enums.drain(..) {
            ids.push(e.node.id);
            self.program.enums.push(e);
        }
        for t in program.traits.drain(..) {
            ids.push(t.node.id);
            self.program.traits.push(t);
        }
        for e in program.errors.drain(..) {
            ids.push(e.node.id);
            self.program.errors.push(e);
        }
        if let Some(a) = program.app.take() {
            if self.program.app.is_some() {
                return Err(SdkError::Edit("program already has an app declaration".to_string()));
            }
            ids.push(a.node.id);
            self.program.app = Some(a);
        }

        Ok(ids)
    }

    /// Replace a top-level declaration with new source.
    /// The replacement must be the same kind (function→function, class→class, etc.).
    /// The top-level UUID is preserved; nested items are matched by name.
    pub fn replace_from_source(&mut self, id: Uuid, source: &str) -> Result<(), SdkError> {
        let (kind, idx) = self.find_top_level(id)?;
        let mut program = parse_single_program(source, &self.program)?;

        // Merge any imports from the replacement source.
        merge_imports(&mut self.program.imports, &program.imports);

        match kind {
            DeclKindSimple::Function => {
                let mut new_fn = program.functions.pop()
                    .ok_or_else(|| SdkError::Edit("replacement source must be a function".to_string()))?;
                // Preserve top-level UUID
                new_fn.node.id = id;
                // Match params by name
                let old_fn = &self.program.functions[idx];
                transplant_params(&mut new_fn.node.params, &old_fn.node.params);
                self.program.functions[idx] = new_fn;
            }
            DeclKindSimple::Class => {
                let mut new_cls = program.classes.pop()
                    .ok_or_else(|| SdkError::Edit("replacement source must be a class".to_string()))?;
                new_cls.node.id = id;
                let old_cls = &self.program.classes[idx];
                transplant_fields(&mut new_cls.node.fields, &old_cls.node.fields);
                transplant_methods(&mut new_cls.node.methods, &old_cls.node.methods);
                self.program.classes[idx] = new_cls;
            }
            DeclKindSimple::Enum => {
                let mut new_enum = program.enums.pop()
                    .ok_or_else(|| SdkError::Edit("replacement source must be an enum".to_string()))?;
                new_enum.node.id = id;
                let old_enum = &self.program.enums[idx];
                transplant_variants(&mut new_enum.node.variants, &old_enum.node.variants);
                self.program.enums[idx] = new_enum;
            }
            DeclKindSimple::Trait => {
                let mut new_trait = program.traits.pop()
                    .ok_or_else(|| SdkError::Edit("replacement source must be a trait".to_string()))?;
                new_trait.node.id = id;
                let old_trait = &self.program.traits[idx];
                transplant_trait_methods(&mut new_trait.node.methods, &old_trait.node.methods);
                self.program.traits[idx] = new_trait;
            }
            DeclKindSimple::Error => {
                let mut new_err = program.errors.pop()
                    .ok_or_else(|| SdkError::Edit("replacement source must be an error".to_string()))?;
                new_err.node.id = id;
                let old_err = &self.program.errors[idx];
                transplant_fields(&mut new_err.node.fields, &old_err.node.fields);
                self.program.errors[idx] = new_err;
            }
            DeclKindSimple::App => {
                return Err(SdkError::Edit("replace is not supported for app declarations".to_string()));
            }
        }

        Ok(())
    }

    /// Delete a top-level declaration by UUID.
    /// Returns the pretty-printed source of the deleted declaration
    /// and a best-effort list of dangling references.
    pub fn delete(&mut self, id: Uuid) -> Result<DeleteResult, SdkError> {
        let (kind, idx) = self.find_top_level(id)?;

        let deleted_source = match kind {
            DeclKindSimple::Function => {
                let removed = self.program.functions.remove(idx);
                plutoc::pretty::pretty_print_function(&removed.node, false)
            }
            DeclKindSimple::Class => {
                let removed = self.program.classes.remove(idx);
                plutoc::pretty::pretty_print_class(&removed.node, false)
            }
            DeclKindSimple::Enum => {
                let removed = self.program.enums.remove(idx);
                plutoc::pretty::pretty_print_enum(&removed.node, false)
            }
            DeclKindSimple::Trait => {
                let removed = self.program.traits.remove(idx);
                plutoc::pretty::pretty_print_trait(&removed.node, false)
            }
            DeclKindSimple::Error => {
                let removed = self.program.errors.remove(idx);
                plutoc::pretty::pretty_print_error(&removed.node, false)
            }
            DeclKindSimple::App => {
                return Err(SdkError::Edit("delete is not supported for app declarations".to_string()));
            }
        };

        // Best-effort dangling reference scan from current AST state
        let dangling = collect_dangling_refs(&self.program, id);

        Ok(DeleteResult {
            source: deleted_source,
            dangling,
        })
    }

    /// Rename a top-level declaration and update all reference sites in the AST.
    pub fn rename(&mut self, id: Uuid, new_name: &str) -> Result<(), SdkError> {
        let (kind, idx) = self.find_top_level(id)?;

        let old_name = match kind {
            DeclKindSimple::Function => {
                let old = self.program.functions[idx].node.name.node.clone();
                self.program.functions[idx].node.name.node = new_name.to_string();
                old
            }
            DeclKindSimple::Class => {
                let old = self.program.classes[idx].node.name.node.clone();
                self.program.classes[idx].node.name.node = new_name.to_string();
                old
            }
            DeclKindSimple::Enum => {
                let old = self.program.enums[idx].node.name.node.clone();
                self.program.enums[idx].node.name.node = new_name.to_string();
                old
            }
            DeclKindSimple::Trait => {
                let old = self.program.traits[idx].node.name.node.clone();
                self.program.traits[idx].node.name.node = new_name.to_string();
                old
            }
            DeclKindSimple::Error => {
                let old = self.program.errors[idx].node.name.node.clone();
                self.program.errors[idx].node.name.node = new_name.to_string();
                old
            }
            DeclKindSimple::App => {
                return Err(SdkError::Edit("rename is not supported for app declarations".to_string()));
            }
        };

        // Walk entire AST to update reference sites
        rename_references(&mut self.program, id, kind, &old_name, new_name);

        Ok(())
    }

    /// Parse a method from source and add it to a class.
    /// Uses the class-wrapper technique to parse method syntax (self params).
    pub fn add_method_from_source(&mut self, class_id: Uuid, source: &str) -> Result<Uuid, SdkError> {
        let class_idx = self.find_class(class_id)?;

        let method = parse_method_snippet(source, &self.program)?;
        let id = method.node.id;
        self.program.classes[class_idx].node.methods.push(method);
        Ok(id)
    }

    /// Add a field to a class.
    pub fn add_field(&mut self, class_id: Uuid, name: &str, ty: &str) -> Result<Uuid, SdkError> {
        let class_idx = self.find_class(class_id)?;

        // Parse the type expression via a dummy function
        let ty_expr = parse_type_expr(ty)?;
        let field_id = Uuid::new_v4();
        let field = Field {
            id: field_id,
            name: Spanned::new(name.to_string(), Span::dummy()),
            ty: Spanned::new(ty_expr, Span::dummy()),
            is_injected: false,
            is_ambient: false,
        };
        self.program.classes[class_idx].node.fields.push(field);
        Ok(field_id)
    }

    /// Pretty-print the modified AST, re-resolve cross-references, rebuild the index,
    /// and return a new `Module`.
    pub fn commit(mut self) -> Module {
        // Pretty-print produces fresh source
        let source = plutoc::pretty::pretty_print(&self.program, false);

        // Re-resolve cross-references
        plutoc::xref::resolve_cross_refs(&mut self.program);

        // Rebuild index
        let index = ModuleIndex::build(&self.program);

        Module::from_parts(self.program, source, index)
    }

    /// Access the in-progress program (read-only).
    pub fn program(&self) -> &Program {
        &self.program
    }

    // --- Internal helpers ---

    /// Find a top-level declaration by UUID, returning its simple kind and vec index.
    fn find_top_level(&self, id: Uuid) -> Result<(DeclKindSimple, usize), SdkError> {
        for (i, f) in self.program.functions.iter().enumerate() {
            if f.node.id == id {
                return Ok((DeclKindSimple::Function, i));
            }
        }
        for (i, c) in self.program.classes.iter().enumerate() {
            if c.node.id == id {
                return Ok((DeclKindSimple::Class, i));
            }
        }
        for (i, e) in self.program.enums.iter().enumerate() {
            if e.node.id == id {
                return Ok((DeclKindSimple::Enum, i));
            }
        }
        for (i, t) in self.program.traits.iter().enumerate() {
            if t.node.id == id {
                return Ok((DeclKindSimple::Trait, i));
            }
        }
        for (i, e) in self.program.errors.iter().enumerate() {
            if e.node.id == id {
                return Ok((DeclKindSimple::Error, i));
            }
        }
        if let Some(app) = &self.program.app {
            if app.node.id == id {
                return Ok((DeclKindSimple::App, 0));
            }
        }
        Err(SdkError::Edit(format!("declaration with UUID {} not found", id)))
    }

    /// Find a class by UUID, returning its index.
    fn find_class(&self, id: Uuid) -> Result<usize, SdkError> {
        for (i, c) in self.program.classes.iter().enumerate() {
            if c.node.id == id {
                return Ok(i);
            }
        }
        Err(SdkError::Edit("target must be a class".to_string()))
    }
}

// --- Simple declaration kind (top-level only) ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclKindSimple {
    Function,
    Class,
    Enum,
    Trait,
    Error,
    App,
}

// --- Parsing helpers ---

/// Merge imports from `new` into `existing`, skipping duplicates.
/// Two imports are considered duplicates if they have the same path segments and alias.
fn merge_imports(existing: &mut Vec<Spanned<ImportDecl>>, new: &[Spanned<ImportDecl>]) {
    for imp in new {
        let new_path: Vec<&str> = imp.node.path.iter().map(|s| s.node.as_str()).collect();
        let new_alias: Option<&str> = imp.node.alias.as_ref().map(|a| a.node.as_str());
        let already_exists = existing.iter().any(|e| {
            let existing_path: Vec<&str> = e.node.path.iter().map(|s| s.node.as_str()).collect();
            let existing_alias: Option<&str> = e.node.alias.as_ref().map(|a| a.node.as_str());
            existing_path == new_path && existing_alias == new_alias
        });
        if !already_exists {
            existing.push(imp.clone());
        }
    }
}

/// Collect enum names from the current Program for parser context.
fn collect_enum_names(program: &Program) -> HashSet<String> {
    let mut names = HashSet::new();
    for e in &program.enums {
        names.insert(e.node.name.node.clone());
    }
    names
}

/// Parse source as a program with enum context from the current state.
fn parse_single_program(source: &str, context: &Program) -> Result<Program, SdkError> {
    let tokens = plutoc::lexer::lex(source)?;
    let enum_names = collect_enum_names(context);
    let mut parser = plutoc::parser::Parser::new_with_enum_context(&tokens, source, enum_names);
    let program = parser.parse_program()?;
    Ok(program)
}

/// Parse a method snippet by wrapping it in a temporary class.
fn parse_method_snippet(source: &str, context: &Program) -> Result<Spanned<Function>, SdkError> {
    let wrapped = format!("class __Tmp {{\n{}\n}}", source);
    let tokens = plutoc::lexer::lex(&wrapped)?;
    let enum_names = collect_enum_names(context);
    let mut parser = plutoc::parser::Parser::new_with_enum_context(&tokens, &wrapped, enum_names);
    let mut program = parser.parse_program()?;

    if program.classes.is_empty() {
        return Err(SdkError::Edit("failed to parse method wrapper class".to_string()));
    }

    let mut cls = program.classes.remove(0);
    if cls.node.methods.is_empty() {
        return Err(SdkError::Edit("source contains no method".to_string()));
    }
    if cls.node.methods.len() > 1 {
        return Err(SdkError::Edit("source contains multiple methods; expected exactly one".to_string()));
    }

    Ok(cls.node.methods.remove(0))
}

/// Parse a type expression by wrapping it in a dummy function parameter.
fn parse_type_expr(ty: &str) -> Result<TypeExpr, SdkError> {
    let dummy = format!("fn __tmp(x: {}) {{}}", ty);
    let tokens = plutoc::lexer::lex(&dummy)?;
    let mut parser = plutoc::parser::Parser::new(&tokens, &dummy);
    let mut program = parser.parse_program()?;

    if program.functions.is_empty() || program.functions[0].node.params.is_empty() {
        return Err(SdkError::Edit(format!("failed to parse type expression: {}", ty)));
    }

    Ok(program.functions.remove(0).node.params.remove(0).ty.node)
}

// --- UUID transplanting ---

/// Match new params to old params by name, preserving UUIDs for matching names.
fn transplant_params(new_params: &mut [Param], old_params: &[Param]) {
    for new_p in new_params.iter_mut() {
        for old_p in old_params {
            if new_p.name.node == old_p.name.node {
                new_p.id = old_p.id;
                break;
            }
        }
    }
}

/// Match new fields to old fields by name, preserving UUIDs.
fn transplant_fields(new_fields: &mut [Field], old_fields: &[Field]) {
    for new_f in new_fields.iter_mut() {
        for old_f in old_fields {
            if new_f.name.node == old_f.name.node {
                new_f.id = old_f.id;
                break;
            }
        }
    }
}

/// Match new methods to old methods by name, preserving UUIDs and recursively matching params.
fn transplant_methods(new_methods: &mut [Spanned<Function>], old_methods: &[Spanned<Function>]) {
    for new_m in new_methods.iter_mut() {
        for old_m in old_methods {
            if new_m.node.name.node == old_m.node.name.node {
                new_m.node.id = old_m.node.id;
                transplant_params(&mut new_m.node.params, &old_m.node.params);
                break;
            }
        }
    }
}

/// Match new enum variants to old variants by name, preserving UUIDs and recursively matching fields.
fn transplant_variants(new_variants: &mut [EnumVariant], old_variants: &[EnumVariant]) {
    for new_v in new_variants.iter_mut() {
        for old_v in old_variants {
            if new_v.name.node == old_v.name.node {
                new_v.id = old_v.id;
                transplant_fields(&mut new_v.fields, &old_v.fields);
                break;
            }
        }
    }
}

/// Match new trait methods to old trait methods by name, preserving UUIDs.
fn transplant_trait_methods(new_methods: &mut [TraitMethod], old_methods: &[TraitMethod]) {
    for new_m in new_methods.iter_mut() {
        for old_m in old_methods {
            if new_m.name.node == old_m.name.node {
                new_m.id = old_m.id;
                transplant_params(&mut new_m.params, &old_m.params);
                break;
            }
        }
    }
}

// --- Dangling reference collection ---

/// Scan the AST for references to a deleted UUID.
fn collect_dangling_refs(program: &Program, deleted_id: Uuid) -> Vec<DanglingRef> {
    let mut dangling = Vec::new();

    for f in &program.functions {
        collect_dangling_in_block(&f.node.body.node, deleted_id, &mut dangling);
    }
    for c in &program.classes {
        for m in &c.node.methods {
            collect_dangling_in_block(&m.node.body.node, deleted_id, &mut dangling);
        }
    }
    if let Some(app) = &program.app {
        for m in &app.node.methods {
            collect_dangling_in_block(&m.node.body.node, deleted_id, &mut dangling);
        }
    }

    dangling
}

fn collect_dangling_in_block(block: &Block, target: Uuid, out: &mut Vec<DanglingRef>) {
    for stmt in &block.stmts {
        collect_dangling_in_stmt(&stmt.node, stmt.span, target, out);
    }
}

fn collect_dangling_in_stmt(stmt: &Stmt, span: Span, target: Uuid, out: &mut Vec<DanglingRef>) {
    match stmt {
        Stmt::Let { value, .. } => collect_dangling_in_expr(&value.node, value.span, target, out),
        Stmt::Return(Some(e)) => collect_dangling_in_expr(&e.node, e.span, target, out),
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => collect_dangling_in_expr(&value.node, value.span, target, out),
        Stmt::FieldAssign { object, value, .. } => {
            collect_dangling_in_expr(&object.node, object.span, target, out);
            collect_dangling_in_expr(&value.node, value.span, target, out);
        }
        Stmt::If { condition, then_block, else_block } => {
            collect_dangling_in_expr(&condition.node, condition.span, target, out);
            collect_dangling_in_block(&then_block.node, target, out);
            if let Some(eb) = else_block {
                collect_dangling_in_block(&eb.node, target, out);
            }
        }
        Stmt::While { condition, body } => {
            collect_dangling_in_expr(&condition.node, condition.span, target, out);
            collect_dangling_in_block(&body.node, target, out);
        }
        Stmt::For { iterable, body, .. } => {
            collect_dangling_in_expr(&iterable.node, iterable.span, target, out);
            collect_dangling_in_block(&body.node, target, out);
        }
        Stmt::IndexAssign { object, index, value } => {
            collect_dangling_in_expr(&object.node, object.span, target, out);
            collect_dangling_in_expr(&index.node, index.span, target, out);
            collect_dangling_in_expr(&value.node, value.span, target, out);
        }
        Stmt::Match { expr, arms } => {
            collect_dangling_in_expr(&expr.node, expr.span, target, out);
            for arm in arms {
                if arm.enum_id == Some(target) {
                    out.push(DanglingRef {
                        kind: DanglingRefKind::MatchArm,
                        name: arm.enum_name.node.clone(),
                        span: arm.enum_name.span,
                    });
                }
                collect_dangling_in_block(&arm.body.node, target, out);
            }
        }
        Stmt::Raise { error_id, error_name, fields, .. } => {
            if *error_id == Some(target) {
                out.push(DanglingRef {
                    kind: DanglingRefKind::Raise,
                    name: error_name.node.clone(),
                    span,
                });
            }
            for (_, e) in fields {
                collect_dangling_in_expr(&e.node, e.span, target, out);
            }
        }
        Stmt::LetChan { capacity, .. } => {
            if let Some(cap) = capacity {
                collect_dangling_in_expr(&cap.node, cap.span, target, out);
            }
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &arm.op {
                    SelectOp::Recv { channel, .. } => {
                        collect_dangling_in_expr(&channel.node, channel.span, target, out);
                    }
                    SelectOp::Send { channel, value } => {
                        collect_dangling_in_expr(&channel.node, channel.span, target, out);
                        collect_dangling_in_expr(&value.node, value.span, target, out);
                    }
                }
                collect_dangling_in_block(&arm.body.node, target, out);
            }
            if let Some(def) = default {
                collect_dangling_in_block(&def.node, target, out);
            }
        }
        Stmt::Break | Stmt::Continue => {}
        Stmt::Expr(e) => collect_dangling_in_expr(&e.node, e.span, target, out),
        Stmt::Scope { seeds, body, .. } => {
            for seed in seeds {
                collect_dangling_in_expr(&seed.node, seed.span, target, out);
            }
            collect_dangling_in_block(&body.node, target, out);
        }
        Stmt::Yield { value } => {
            collect_dangling_in_expr(&value.node, value.span, target, out);
        }
        Stmt::Assert { expr } => {
            collect_dangling_in_expr(&expr.node, expr.span, target, out);
        }
    }
}

fn collect_dangling_in_expr(expr: &Expr, span: Span, target: Uuid, out: &mut Vec<DanglingRef>) {
    match expr {
        Expr::Call { name, args, target_id, .. } => {
            if *target_id == Some(target) {
                out.push(DanglingRef {
                    kind: DanglingRefKind::Call,
                    name: name.node.clone(),
                    span,
                });
            }
            for arg in args {
                collect_dangling_in_expr(&arg.node, arg.span, target, out);
            }
        }
        Expr::StructLit { name, fields, target_id, .. } => {
            if *target_id == Some(target) {
                out.push(DanglingRef {
                    kind: DanglingRefKind::StructLit,
                    name: name.node.clone(),
                    span,
                });
            }
            for (_, fexpr) in fields {
                collect_dangling_in_expr(&fexpr.node, fexpr.span, target, out);
            }
        }
        Expr::EnumUnit { enum_name, enum_id, .. } => {
            if *enum_id == Some(target) {
                out.push(DanglingRef {
                    kind: DanglingRefKind::EnumUsage,
                    name: enum_name.node.clone(),
                    span,
                });
            }
        }
        Expr::EnumData { enum_name, fields, enum_id, .. } => {
            if *enum_id == Some(target) {
                out.push(DanglingRef {
                    kind: DanglingRefKind::EnumUsage,
                    name: enum_name.node.clone(),
                    span,
                });
            }
            for (_, fexpr) in fields {
                collect_dangling_in_expr(&fexpr.node, fexpr.span, target, out);
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_dangling_in_expr(&lhs.node, lhs.span, target, out);
            collect_dangling_in_expr(&rhs.node, rhs.span, target, out);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_dangling_in_expr(&operand.node, operand.span, target, out);
        }
        Expr::FieldAccess { object, .. } => {
            collect_dangling_in_expr(&object.node, object.span, target, out);
        }
        Expr::MethodCall { object, args, .. } => {
            collect_dangling_in_expr(&object.node, object.span, target, out);
            for arg in args {
                collect_dangling_in_expr(&arg.node, arg.span, target, out);
            }
        }
        Expr::ArrayLit { elements } | Expr::SetLit { elements, .. } => {
            for el in elements {
                collect_dangling_in_expr(&el.node, el.span, target, out);
            }
        }
        Expr::Index { object, index } => {
            collect_dangling_in_expr(&object.node, object.span, target, out);
            collect_dangling_in_expr(&index.node, index.span, target, out);
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    collect_dangling_in_expr(&e.node, e.span, target, out);
                }
            }
        }
        Expr::Closure { body, .. } => {
            collect_dangling_in_block(&body.node, target, out);
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_dangling_in_expr(&k.node, k.span, target, out);
                collect_dangling_in_expr(&v.node, v.span, target, out);
            }
        }
        Expr::Propagate { expr } | Expr::Cast { expr, .. } | Expr::Spawn { call: expr } | Expr::NullPropagate { expr } => {
            collect_dangling_in_expr(&expr.node, expr.span, target, out);
        }
        Expr::Catch { expr: inner, handler } => {
            collect_dangling_in_expr(&inner.node, inner.span, target, out);
            match handler {
                CatchHandler::Wildcard { body, .. } => collect_dangling_in_block(&body.node, target, out),
                CatchHandler::Shorthand(body) => collect_dangling_in_expr(&body.node, body.span, target, out),
            }
        }
        Expr::Range { start, end, .. } => {
            collect_dangling_in_expr(&start.node, start.span, target, out);
            collect_dangling_in_expr(&end.node, end.span, target, out);
        }
        // Leaf expressions
        _ => {}
    }
}

// --- Rename reference walker ---

/// Walk the entire program AST and update references to a renamed declaration.
fn rename_references(
    program: &mut Program,
    id: Uuid,
    kind: DeclKindSimple,
    old_name: &str,
    new_name: &str,
) {
    // Walk all function bodies and type signatures
    for f in &mut program.functions {
        rename_in_params(&mut f.node.params, kind, old_name, new_name);
        rename_in_return_type(&mut f.node.return_type, kind, old_name, new_name);
        rename_in_block(&mut f.node.body.node, id, kind, old_name, new_name);
    }
    for c in &mut program.classes {
        // Walk class fields type expressions
        for field in &mut c.node.fields {
            rename_in_type_expr(&mut field.ty.node, kind, old_name, new_name);
        }
        // Walk impl_traits
        if kind == DeclKindSimple::Trait {
            for trait_name in &mut c.node.impl_traits {
                if trait_name.node == old_name {
                    trait_name.node = new_name.to_string();
                }
            }
        }
        for m in &mut c.node.methods {
            rename_in_params(&mut m.node.params, kind, old_name, new_name);
            rename_in_return_type(&mut m.node.return_type, kind, old_name, new_name);
            rename_in_block(&mut m.node.body.node, id, kind, old_name, new_name);
        }
    }
    // Walk trait method signatures
    for t in &mut program.traits {
        for m in &mut t.node.methods {
            rename_in_params(&mut m.params, kind, old_name, new_name);
            if let Some(ref mut rt) = m.return_type {
                rename_in_type_expr(&mut rt.node, kind, old_name, new_name);
            }
            if let Some(ref mut body) = m.body {
                rename_in_block(&mut body.node, id, kind, old_name, new_name);
            }
        }
    }
    // Walk error field types
    for e in &mut program.errors {
        for field in &mut e.node.fields {
            rename_in_type_expr(&mut field.ty.node, kind, old_name, new_name);
        }
    }
    // Walk app methods
    if let Some(app) = &mut program.app {
        for field in &mut app.node.inject_fields {
            rename_in_type_expr(&mut field.ty.node, kind, old_name, new_name);
        }
        for m in &mut app.node.methods {
            rename_in_params(&mut m.node.params, kind, old_name, new_name);
            rename_in_return_type(&mut m.node.return_type, kind, old_name, new_name);
            rename_in_block(&mut m.node.body.node, id, kind, old_name, new_name);
        }
    }
}

fn rename_in_params(params: &mut [Param], kind: DeclKindSimple, old_name: &str, new_name: &str) {
    for p in params {
        rename_in_type_expr(&mut p.ty.node, kind, old_name, new_name);
    }
}

fn rename_in_return_type(rt: &mut Option<Spanned<TypeExpr>>, kind: DeclKindSimple, old_name: &str, new_name: &str) {
    if let Some(ref mut rt) = rt {
        rename_in_type_expr(&mut rt.node, kind, old_name, new_name);
    }
}

fn rename_in_type_expr(te: &mut TypeExpr, kind: DeclKindSimple, old_name: &str, new_name: &str) {
    match te {
        TypeExpr::Named(name) => {
            if name == old_name && matches!(kind, DeclKindSimple::Class | DeclKindSimple::Enum | DeclKindSimple::Trait | DeclKindSimple::Error) {
                *name = new_name.to_string();
            }
        }
        TypeExpr::Generic { name, type_args } => {
            if name == old_name && matches!(kind, DeclKindSimple::Class | DeclKindSimple::Enum | DeclKindSimple::Trait | DeclKindSimple::Error) {
                *name = new_name.to_string();
            }
            for arg in type_args {
                rename_in_type_expr(&mut arg.node, kind, old_name, new_name);
            }
        }
        TypeExpr::Array(inner) => {
            rename_in_type_expr(&mut inner.node, kind, old_name, new_name);
        }
        TypeExpr::Fn { params, return_type } => {
            for p in params {
                rename_in_type_expr(&mut p.node, kind, old_name, new_name);
            }
            rename_in_type_expr(&mut return_type.node, kind, old_name, new_name);
        }
        TypeExpr::Qualified { name, .. } => {
            if name == old_name && matches!(kind, DeclKindSimple::Class | DeclKindSimple::Enum | DeclKindSimple::Trait | DeclKindSimple::Error) {
                *name = new_name.to_string();
            }
        }
        TypeExpr::Nullable(inner) => {
            rename_in_type_expr(&mut inner.node, kind, old_name, new_name);
        }
        TypeExpr::Stream(inner) => {
            rename_in_type_expr(&mut inner.node, kind, old_name, new_name);
        }
    }
}

fn rename_in_block(block: &mut Block, id: Uuid, kind: DeclKindSimple, old_name: &str, new_name: &str) {
    for stmt in &mut block.stmts {
        rename_in_stmt(&mut stmt.node, id, kind, old_name, new_name);
    }
}

fn rename_in_stmt(stmt: &mut Stmt, id: Uuid, kind: DeclKindSimple, old_name: &str, new_name: &str) {
    match stmt {
        Stmt::Let { ty, value, .. } => {
            if let Some(ref mut t) = ty {
                rename_in_type_expr(&mut t.node, kind, old_name, new_name);
            }
            rename_in_expr(&mut value.node, id, kind, old_name, new_name);
        }
        Stmt::Return(Some(e)) => {
            rename_in_expr(&mut e.node, id, kind, old_name, new_name);
        }
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => {
            rename_in_expr(&mut value.node, id, kind, old_name, new_name);
        }
        Stmt::FieldAssign { object, value, .. } => {
            rename_in_expr(&mut object.node, id, kind, old_name, new_name);
            rename_in_expr(&mut value.node, id, kind, old_name, new_name);
        }
        Stmt::If { condition, then_block, else_block } => {
            rename_in_expr(&mut condition.node, id, kind, old_name, new_name);
            rename_in_block(&mut then_block.node, id, kind, old_name, new_name);
            if let Some(eb) = else_block {
                rename_in_block(&mut eb.node, id, kind, old_name, new_name);
            }
        }
        Stmt::While { condition, body } => {
            rename_in_expr(&mut condition.node, id, kind, old_name, new_name);
            rename_in_block(&mut body.node, id, kind, old_name, new_name);
        }
        Stmt::For { iterable, body, .. } => {
            rename_in_expr(&mut iterable.node, id, kind, old_name, new_name);
            rename_in_block(&mut body.node, id, kind, old_name, new_name);
        }
        Stmt::IndexAssign { object, index, value } => {
            rename_in_expr(&mut object.node, id, kind, old_name, new_name);
            rename_in_expr(&mut index.node, id, kind, old_name, new_name);
            rename_in_expr(&mut value.node, id, kind, old_name, new_name);
        }
        Stmt::Match { expr, arms } => {
            rename_in_expr(&mut expr.node, id, kind, old_name, new_name);
            for arm in arms {
                if kind == DeclKindSimple::Enum {
                    if arm.enum_id == Some(id) {
                        arm.enum_name.node = new_name.to_string();
                    }
                }
                rename_in_block(&mut arm.body.node, id, kind, old_name, new_name);
            }
        }
        Stmt::Raise { error_name, fields, error_id } => {
            if kind == DeclKindSimple::Error && *error_id == Some(id) {
                error_name.node = new_name.to_string();
            }
            for (_, e) in fields {
                rename_in_expr(&mut e.node, id, kind, old_name, new_name);
            }
        }
        Stmt::LetChan { elem_type, capacity, .. } => {
            rename_in_type_expr(&mut elem_type.node, kind, old_name, new_name);
            if let Some(cap) = capacity {
                rename_in_expr(&mut cap.node, id, kind, old_name, new_name);
            }
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &mut arm.op {
                    SelectOp::Recv { channel, .. } => {
                        rename_in_expr(&mut channel.node, id, kind, old_name, new_name);
                    }
                    SelectOp::Send { channel, value } => {
                        rename_in_expr(&mut channel.node, id, kind, old_name, new_name);
                        rename_in_expr(&mut value.node, id, kind, old_name, new_name);
                    }
                }
                rename_in_block(&mut arm.body.node, id, kind, old_name, new_name);
            }
            if let Some(def) = default {
                rename_in_block(&mut def.node, id, kind, old_name, new_name);
            }
        }
        Stmt::Break | Stmt::Continue => {}
        Stmt::Expr(e) => {
            rename_in_expr(&mut e.node, id, kind, old_name, new_name);
        }
        Stmt::Scope { seeds, bindings, body, .. } => {
            for seed in seeds {
                rename_in_expr(&mut seed.node, id, kind, old_name, new_name);
            }
            for binding in bindings {
                rename_in_type_expr(&mut binding.ty.node, kind, old_name, new_name);
            }
            rename_in_block(&mut body.node, id, kind, old_name, new_name);
        }
        Stmt::Yield { value } => {
            rename_in_expr(&mut value.node, id, kind, old_name, new_name);
        }
        Stmt::Assert { expr } => {
            rename_in_expr(&mut expr.node, id, kind, old_name, new_name);
        }
    }
}

fn rename_in_expr(expr: &mut Expr, id: Uuid, kind: DeclKindSimple, old_name: &str, new_name: &str) {
    match expr {
        Expr::Call { name, args, target_id, .. } => {
            if kind == DeclKindSimple::Function && *target_id == Some(id) {
                name.node = new_name.to_string();
            }
            for arg in args {
                rename_in_expr(&mut arg.node, id, kind, old_name, new_name);
            }
        }
        Expr::StructLit { name, fields, target_id, .. } => {
            if kind == DeclKindSimple::Class && *target_id == Some(id) {
                name.node = new_name.to_string();
            }
            for (_, fexpr) in fields {
                rename_in_expr(&mut fexpr.node, id, kind, old_name, new_name);
            }
        }
        Expr::EnumUnit { enum_name, enum_id, .. } => {
            if kind == DeclKindSimple::Enum && *enum_id == Some(id) {
                enum_name.node = new_name.to_string();
            }
        }
        Expr::EnumData { enum_name, fields, enum_id, .. } => {
            if kind == DeclKindSimple::Enum && *enum_id == Some(id) {
                enum_name.node = new_name.to_string();
            }
            for (_, fexpr) in fields {
                rename_in_expr(&mut fexpr.node, id, kind, old_name, new_name);
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            rename_in_expr(&mut lhs.node, id, kind, old_name, new_name);
            rename_in_expr(&mut rhs.node, id, kind, old_name, new_name);
        }
        Expr::UnaryOp { operand, .. } => {
            rename_in_expr(&mut operand.node, id, kind, old_name, new_name);
        }
        Expr::FieldAccess { object, .. } => {
            rename_in_expr(&mut object.node, id, kind, old_name, new_name);
        }
        Expr::MethodCall { object, args, .. } => {
            rename_in_expr(&mut object.node, id, kind, old_name, new_name);
            for arg in args {
                rename_in_expr(&mut arg.node, id, kind, old_name, new_name);
            }
        }
        Expr::ArrayLit { elements } | Expr::SetLit { elements, .. } => {
            for el in elements {
                rename_in_expr(&mut el.node, id, kind, old_name, new_name);
            }
        }
        Expr::Index { object, index } => {
            rename_in_expr(&mut object.node, id, kind, old_name, new_name);
            rename_in_expr(&mut index.node, id, kind, old_name, new_name);
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    rename_in_expr(&mut e.node, id, kind, old_name, new_name);
                }
            }
        }
        Expr::Closure { params, return_type, body } => {
            rename_in_params(params, kind, old_name, new_name);
            rename_in_return_type(return_type, kind, old_name, new_name);
            rename_in_block(&mut body.node, id, kind, old_name, new_name);
        }
        Expr::MapLit { key_type, value_type, entries } => {
            rename_in_type_expr(&mut key_type.node, kind, old_name, new_name);
            rename_in_type_expr(&mut value_type.node, kind, old_name, new_name);
            for (k, v) in entries {
                rename_in_expr(&mut k.node, id, kind, old_name, new_name);
                rename_in_expr(&mut v.node, id, kind, old_name, new_name);
            }
        }
        Expr::Propagate { expr } | Expr::Cast { expr, .. } | Expr::Spawn { call: expr } | Expr::NullPropagate { expr } => {
            rename_in_expr(&mut expr.node, id, kind, old_name, new_name);
        }
        Expr::Catch { expr: inner, handler } => {
            rename_in_expr(&mut inner.node, id, kind, old_name, new_name);
            match handler {
                CatchHandler::Wildcard { body, .. } => rename_in_block(&mut body.node, id, kind, old_name, new_name),
                CatchHandler::Shorthand(body) => rename_in_expr(&mut body.node, id, kind, old_name, new_name),
            }
        }
        Expr::Range { start, end, .. } => {
            rename_in_expr(&mut start.node, id, kind, old_name, new_name);
            rename_in_expr(&mut end.node, id, kind, old_name, new_name);
        }
        // Leaf expressions
        _ => {}
    }
}
