use std::path::Path;
use uuid::Uuid;

use plutoc::derived::{
    DerivedInfo, ErrorRef, ResolvedClassInfo, ResolvedEnumInfo, ResolvedErrorInfo,
    ResolvedSignature, ResolvedTraitInfo,
};
use plutoc::parser::ast::Program;
use plutoc::span::Span;

use crate::decl::{DeclKind, DeclRef};
use crate::editor::ModuleEditor;
use crate::error::SdkError;
use crate::index::ModuleIndex;
use crate::xref::{CallSite, ConstructSite, EnumUsageSite, RaiseSite};

/// Primary entry point for querying a Pluto program.
/// Owns the deserialized Program and source text, with pre-built indexes and derived type data.
pub struct Module {
    program: Program,
    source: String,
    index: ModuleIndex,
    derived: DerivedInfo,
}

impl Module {
    /// Load from PLTO binary bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SdkError> {
        let (program, source, derived) = plutoc::binary::deserialize_program(bytes)?;
        let index = ModuleIndex::build(&program);
        Ok(Self { program, source, index, derived })
    }

    /// Load from a PLTO file on disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SdkError> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Analyze a .pluto source file (runs full front-end pipeline).
    pub fn from_source_file(path: impl AsRef<Path>) -> Result<Self, SdkError> {
        Self::from_source_file_with_stdlib(path, None)
    }

    /// Analyze a .pluto source file with an explicit stdlib root path.
    pub fn from_source_file_with_stdlib(path: impl AsRef<Path>, stdlib_root: Option<&Path>) -> Result<Self, SdkError> {
        let (program, source, derived) = plutoc::analyze_file(path.as_ref(), stdlib_root)?;
        let index = ModuleIndex::build(&program);
        Ok(Self { program, source, index, derived })
    }

    /// Create an edit-friendly Module from source text.
    /// Parses without transforms (no monomorphize/closure-lift/spawn-desugar).
    /// Does NOT inject prelude (avoids serializing Option<T> etc. into committed source).
    pub fn from_source(source: &str) -> Result<Self, SdkError> {
        let program = plutoc::parse_for_editing(source)?;
        let index = ModuleIndex::build(&program);
        Ok(Self { program, source: source.to_string(), index, derived: DerivedInfo::default() })
    }

    /// Begin editing this module. Consumes the Module.
    pub fn edit(self) -> ModuleEditor {
        ModuleEditor::new(self.program, self.source)
    }

    /// Construct from parts (used by ModuleEditor::commit).
    pub(crate) fn from_parts(program: Program, source: String, index: ModuleIndex) -> Self {
        Self { program, source, index, derived: DerivedInfo::default() }
    }

    // --- By-UUID lookup ---

    /// Look up a declaration by its stable UUID.
    pub fn get(&self, id: Uuid) -> Option<DeclRef<'_>> {
        let loc = self.index.by_uuid.get(&id)?;
        self.resolve_location(loc)
    }

    // --- By-name lookup ---

    /// Find all declarations with the given name.
    pub fn find(&self, name: &str) -> Vec<DeclRef<'_>> {
        let Some(ids) = self.index.by_name.get(name) else {
            return vec![];
        };
        ids.iter()
            .filter_map(|id| self.get(*id))
            .collect()
    }

    // --- Listing methods ---

    pub fn functions(&self) -> Vec<DeclRef<'_>> {
        self.program.functions.iter()
            .map(|f| DeclRef::function(&f.node))
            .collect()
    }

    pub fn classes(&self) -> Vec<DeclRef<'_>> {
        self.program.classes.iter()
            .map(|c| DeclRef::class(&c.node))
            .collect()
    }

    pub fn enums(&self) -> Vec<DeclRef<'_>> {
        self.program.enums.iter()
            .map(|e| DeclRef::enum_decl(&e.node))
            .collect()
    }

    pub fn traits(&self) -> Vec<DeclRef<'_>> {
        self.program.traits.iter()
            .map(|t| DeclRef::trait_decl(&t.node))
            .collect()
    }

    pub fn errors(&self) -> Vec<DeclRef<'_>> {
        self.program.errors.iter()
            .map(|e| DeclRef::error_decl(&e.node))
            .collect()
    }

    pub fn app(&self) -> Option<DeclRef<'_>> {
        self.program.app.as_ref().map(|a| DeclRef::app(&a.node))
    }

    // --- Cross-reference queries ---

    /// Get all call sites that target the given declaration UUID.
    pub fn callers_of(&self, id: Uuid) -> Vec<CallSite<'_>> {
        let Some(infos) = self.index.callers.get(&id) else {
            return vec![];
        };
        infos.iter().filter_map(|info| {
            let caller = self.find_function_by_name(&info.fn_name)?;
            let call_expr = find_expr_at_span(&self.program, info.span)?;
            Some(CallSite {
                caller,
                call_expr,
                target_id: info.target_id,
                span: info.span,
            })
        }).collect()
    }

    /// Get all struct literal sites that construct the given class UUID.
    pub fn constructors_of(&self, id: Uuid) -> Vec<ConstructSite<'_>> {
        let Some(infos) = self.index.constructors.get(&id) else {
            return vec![];
        };
        infos.iter().filter_map(|info| {
            let function = self.find_function_by_name(&info.fn_name)?;
            let struct_lit = find_expr_at_span(&self.program, info.span)?;
            Some(ConstructSite {
                function,
                struct_lit,
                target_id: info.target_id,
                span: info.span,
            })
        }).collect()
    }

    /// Get all sites where the given enum UUID is used.
    pub fn enum_usages_of(&self, id: Uuid) -> Vec<EnumUsageSite<'_>> {
        let Some(infos) = self.index.enum_usages.get(&id) else {
            return vec![];
        };
        infos.iter().filter_map(|info| {
            let function = self.find_function_by_name(&info.fn_name)?;
            let expr = find_expr_at_span(&self.program, info.span)?;
            Some(EnumUsageSite {
                function,
                expr,
                enum_id: info.enum_id,
                variant_id: info.variant_id,
                span: info.span,
            })
        }).collect()
    }

    /// Get all sites where the given error UUID is raised.
    pub fn raise_sites_of(&self, id: Uuid) -> Vec<RaiseSite<'_>> {
        let Some(infos) = self.index.raise_sites.get(&id) else {
            return vec![];
        };
        infos.iter().filter_map(|info| {
            let function = self.find_function_by_name(&info.fn_name)?;
            let stmt = find_stmt_at_span(&self.program, info.span)?;
            Some(RaiseSite {
                function,
                stmt,
                error_id: info.error_id,
                span: info.span,
            })
        }).collect()
    }

    // --- Source access ---

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn source_slice(&self, span: Span) -> &str {
        self.source.get(span.start..span.end).unwrap_or("")
    }

    // --- Raw AST access ---

    pub fn program(&self) -> &Program {
        &self.program
    }

    // --- Derived type data queries ---

    /// Get the error set for a function by its UUID.
    /// Returns an empty slice for unknown UUIDs or infallible functions.
    pub fn error_set_of(&self, id: Uuid) -> &[ErrorRef] {
        self.derived
            .fn_error_sets
            .get(&id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Check whether a function is fallible (can raise errors).
    pub fn is_fallible(&self, id: Uuid) -> bool {
        self.derived
            .fn_error_sets
            .get(&id)
            .is_some_and(|errs| !errs.is_empty())
    }

    /// Get the resolved signature for a function by its UUID.
    pub fn signature_of(&self, id: Uuid) -> Option<&ResolvedSignature> {
        self.derived.fn_signatures.get(&id)
    }

    /// Access the raw derived info.
    pub fn derived(&self) -> &DerivedInfo {
        &self.derived
    }

    /// Get resolved class info by UUID.
    pub fn class_info_of(&self, id: Uuid) -> Option<&ResolvedClassInfo> {
        self.derived.class_infos.get(&id)
    }

    /// Get resolved trait info by UUID.
    pub fn trait_info_of(&self, id: Uuid) -> Option<&ResolvedTraitInfo> {
        self.derived.trait_infos.get(&id)
    }

    /// Get resolved enum info by UUID.
    pub fn enum_info_of(&self, id: Uuid) -> Option<&ResolvedEnumInfo> {
        self.derived.enum_infos.get(&id)
    }

    /// Get resolved error info by UUID.
    pub fn error_info_of(&self, id: Uuid) -> Option<&ResolvedErrorInfo> {
        self.derived.error_infos.get(&id)
    }

    /// Get DI instantiation order (class UUIDs in topological order).
    pub fn di_order(&self) -> &[Uuid] {
        &self.derived.di_order
    }

    /// Get the list of class UUIDs that implement a given trait.
    pub fn trait_implementors_of(&self, trait_id: Uuid) -> &[Uuid] {
        self.derived
            .trait_implementors
            .get(&trait_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    // --- Internal helpers ---

    fn resolve_location(&self, loc: &crate::index::DeclLocation) -> Option<DeclRef<'_>> {
        match loc.kind {
            DeclKind::Function => {
                if let Some(parent_idx) = loc.parent_index {
                    // Method on a class or app
                    if let Some(class) = self.program.classes.get(parent_idx) {
                        let method = class.node.methods.get(loc.index)?;
                        return Some(DeclRef::function(&method.node));
                    }
                    if let Some(app) = &self.program.app {
                        let method = app.node.methods.get(loc.index)?;
                        return Some(DeclRef::function(&method.node));
                    }
                    None
                } else {
                    let f = self.program.functions.get(loc.index)?;
                    Some(DeclRef::function(&f.node))
                }
            }
            DeclKind::Class => {
                let c = self.program.classes.get(loc.index)?;
                Some(DeclRef::class(&c.node))
            }
            DeclKind::Enum => {
                let e = self.program.enums.get(loc.index)?;
                Some(DeclRef::enum_decl(&e.node))
            }
            DeclKind::EnumVariant => {
                let parent = self.program.enums.get(loc.parent_index?)?;
                let v = parent.node.variants.get(loc.index)?;
                Some(DeclRef::enum_variant(v))
            }
            DeclKind::Trait => {
                let t = self.program.traits.get(loc.index)?;
                Some(DeclRef::trait_decl(&t.node))
            }
            DeclKind::TraitMethod => {
                let parent = self.program.traits.get(loc.parent_index?)?;
                let m = parent.node.methods.get(loc.index)?;
                Some(DeclRef::trait_method(m))
            }
            DeclKind::Error => {
                let e = self.program.errors.get(loc.index)?;
                Some(DeclRef::error_decl(&e.node))
            }
            DeclKind::App => {
                let a = self.program.app.as_ref()?;
                Some(DeclRef::app(&a.node))
            }
            DeclKind::Field => {
                // Fields can be on classes, errors, or app — we'd need more context
                // For now, search by UUID in all field containers
                None
            }
            DeclKind::Param => {
                // Params similarly nested — search would be needed
                None
            }
        }
    }

    /// Find a function (top-level, class method, or app method) by mangled name.
    fn find_function_by_name(&self, name: &str) -> Option<&plutoc::parser::ast::Function> {
        // Top-level functions
        for f in &self.program.functions {
            if f.node.name.node == *name {
                return Some(&f.node);
            }
        }
        // Class methods (mangled as ClassName_method)
        for c in &self.program.classes {
            for m in &c.node.methods {
                let mangled = format!("{}_{}", c.node.name.node, m.node.name.node);
                if mangled == name {
                    return Some(&m.node);
                }
            }
        }
        // App methods
        if let Some(app) = &self.program.app {
            for m in &app.node.methods {
                let mangled = format!("{}_{}", app.node.name.node, m.node.name.node);
                if mangled == name {
                    return Some(&m.node);
                }
            }
        }
        None
    }
}

// --- AST search helpers for finding nodes at a specific span ---

use plutoc::parser::ast::*;

fn find_expr_at_span<'a>(program: &'a Program, target: Span) -> Option<&'a Expr> {
    for f in &program.functions {
        if let Some(e) = find_expr_in_block(&f.node.body.node, target) {
            return Some(e);
        }
    }
    for c in &program.classes {
        for m in &c.node.methods {
            if let Some(e) = find_expr_in_block(&m.node.body.node, target) {
                return Some(e);
            }
        }
    }
    if let Some(app) = &program.app {
        for m in &app.node.methods {
            if let Some(e) = find_expr_in_block(&m.node.body.node, target) {
                return Some(e);
            }
        }
    }
    None
}

fn find_stmt_at_span<'a>(program: &'a Program, target: Span) -> Option<&'a Stmt> {
    for f in &program.functions {
        if let Some(s) = find_stmt_in_block(&f.node.body.node, target) {
            return Some(s);
        }
    }
    for c in &program.classes {
        for m in &c.node.methods {
            if let Some(s) = find_stmt_in_block(&m.node.body.node, target) {
                return Some(s);
            }
        }
    }
    if let Some(app) = &program.app {
        for m in &app.node.methods {
            if let Some(s) = find_stmt_in_block(&m.node.body.node, target) {
                return Some(s);
            }
        }
    }
    None
}

fn find_expr_in_block<'a>(block: &'a Block, target: Span) -> Option<&'a Expr> {
    for stmt in &block.stmts {
        if let Some(e) = find_expr_in_stmt(&stmt.node, target) {
            return Some(e);
        }
    }
    None
}

fn find_stmt_in_block<'a>(block: &'a Block, target: Span) -> Option<&'a Stmt> {
    for stmt in &block.stmts {
        if stmt.span == target {
            return Some(&stmt.node);
        }
        // Recurse into nested blocks
        if let Some(s) = find_stmt_nested(&stmt.node, target) {
            return Some(s);
        }
    }
    None
}

fn find_stmt_nested<'a>(stmt: &'a Stmt, target: Span) -> Option<&'a Stmt> {
    match stmt {
        Stmt::If { then_block, else_block, .. } => {
            if let Some(s) = find_stmt_in_block(&then_block.node, target) {
                return Some(s);
            }
            if let Some(eb) = else_block {
                if let Some(s) = find_stmt_in_block(&eb.node, target) {
                    return Some(s);
                }
            }
        }
        Stmt::While { body, .. } | Stmt::For { body, .. } => {
            if let Some(s) = find_stmt_in_block(&body.node, target) {
                return Some(s);
            }
        }
        Stmt::Match { arms, .. } => {
            for arm in arms {
                if let Some(s) = find_stmt_in_block(&arm.body.node, target) {
                    return Some(s);
                }
            }
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                if let Some(s) = find_stmt_in_block(&arm.body.node, target) {
                    return Some(s);
                }
            }
            if let Some(def) = default {
                if let Some(s) = find_stmt_in_block(&def.node, target) {
                    return Some(s);
                }
            }
        }
        _ => {}
    }
    None
}

fn find_expr_in_stmt<'a>(stmt: &'a Stmt, target: Span) -> Option<&'a Expr> {
    match stmt {
        Stmt::Let { value, .. } => find_expr_recursive(&value.node, value.span, target),
        Stmt::Return(Some(e)) => find_expr_recursive(&e.node, e.span, target),
        Stmt::Return(None) => None,
        Stmt::Assign { value, .. } => find_expr_recursive(&value.node, value.span, target),
        Stmt::FieldAssign { object, value, .. } => {
            find_expr_recursive(&object.node, object.span, target)
                .or_else(|| find_expr_recursive(&value.node, value.span, target))
        }
        Stmt::If { condition, then_block, else_block } => {
            find_expr_recursive(&condition.node, condition.span, target)
                .or_else(|| find_expr_in_block(&then_block.node, target))
                .or_else(|| else_block.as_ref().and_then(|eb| find_expr_in_block(&eb.node, target)))
        }
        Stmt::While { condition, body } => {
            find_expr_recursive(&condition.node, condition.span, target)
                .or_else(|| find_expr_in_block(&body.node, target))
        }
        Stmt::For { iterable, body, .. } => {
            find_expr_recursive(&iterable.node, iterable.span, target)
                .or_else(|| find_expr_in_block(&body.node, target))
        }
        Stmt::IndexAssign { object, index, value } => {
            find_expr_recursive(&object.node, object.span, target)
                .or_else(|| find_expr_recursive(&index.node, index.span, target))
                .or_else(|| find_expr_recursive(&value.node, value.span, target))
        }
        Stmt::Match { expr, arms } => {
            find_expr_recursive(&expr.node, expr.span, target)
                .or_else(|| {
                    for arm in arms {
                        if let Some(e) = find_expr_in_block(&arm.body.node, target) {
                            return Some(e);
                        }
                    }
                    None
                })
        }
        Stmt::Raise { fields, .. } => {
            for (_, e) in fields {
                if let Some(found) = find_expr_recursive(&e.node, e.span, target) {
                    return Some(found);
                }
            }
            None
        }
        Stmt::Expr(e) => find_expr_recursive(&e.node, e.span, target),
        Stmt::LetChan { capacity, .. } => {
            capacity.as_ref().and_then(|c| find_expr_recursive(&c.node, c.span, target))
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &arm.op {
                    SelectOp::Recv { channel, .. } => {
                        if let Some(e) = find_expr_recursive(&channel.node, channel.span, target) {
                            return Some(e);
                        }
                    }
                    SelectOp::Send { channel, value } => {
                        if let Some(e) = find_expr_recursive(&channel.node, channel.span, target) {
                            return Some(e);
                        }
                        if let Some(e) = find_expr_recursive(&value.node, value.span, target) {
                            return Some(e);
                        }
                    }
                }
                if let Some(e) = find_expr_in_block(&arm.body.node, target) {
                    return Some(e);
                }
            }
            if let Some(def) = default {
                return find_expr_in_block(&def.node, target);
            }
            None
        }
        Stmt::Break | Stmt::Continue => None,
    }
}

fn find_expr_recursive<'a>(expr: &'a Expr, span: Span, target: Span) -> Option<&'a Expr> {
    if span == target {
        return Some(expr);
    }
    match expr {
        Expr::BinOp { lhs, rhs, .. } => {
            find_expr_recursive(&lhs.node, lhs.span, target)
                .or_else(|| find_expr_recursive(&rhs.node, rhs.span, target))
        }
        Expr::UnaryOp { operand, .. } => {
            find_expr_recursive(&operand.node, operand.span, target)
        }
        Expr::Call { args, .. } => {
            for arg in args {
                if let Some(e) = find_expr_recursive(&arg.node, arg.span, target) {
                    return Some(e);
                }
            }
            None
        }
        Expr::FieldAccess { object, .. } => {
            find_expr_recursive(&object.node, object.span, target)
        }
        Expr::MethodCall { object, args, .. } => {
            find_expr_recursive(&object.node, object.span, target)
                .or_else(|| {
                    for arg in args {
                        if let Some(e) = find_expr_recursive(&arg.node, arg.span, target) {
                            return Some(e);
                        }
                    }
                    None
                })
        }
        Expr::StructLit { fields, .. } => {
            for (_, fexpr) in fields {
                if let Some(e) = find_expr_recursive(&fexpr.node, fexpr.span, target) {
                    return Some(e);
                }
            }
            None
        }
        Expr::ArrayLit { elements } | Expr::SetLit { elements, .. } => {
            for el in elements {
                if let Some(e) = find_expr_recursive(&el.node, el.span, target) {
                    return Some(e);
                }
            }
            None
        }
        Expr::Index { object, index } => {
            find_expr_recursive(&object.node, object.span, target)
                .or_else(|| find_expr_recursive(&index.node, index.span, target))
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    if let Some(found) = find_expr_recursive(&e.node, e.span, target) {
                        return Some(found);
                    }
                }
            }
            None
        }
        Expr::Closure { body, .. } => {
            find_expr_in_block(&body.node, target)
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                if let Some(e) = find_expr_recursive(&k.node, k.span, target) {
                    return Some(e);
                }
                if let Some(e) = find_expr_recursive(&v.node, v.span, target) {
                    return Some(e);
                }
            }
            None
        }
        Expr::EnumData { fields, .. } => {
            for (_, fexpr) in fields {
                if let Some(e) = find_expr_recursive(&fexpr.node, fexpr.span, target) {
                    return Some(e);
                }
            }
            None
        }
        Expr::Propagate { expr: inner } | Expr::Cast { expr: inner, .. } => {
            find_expr_recursive(&inner.node, inner.span, target)
        }
        Expr::Catch { expr: inner, handler } => {
            find_expr_recursive(&inner.node, inner.span, target)
                .or_else(|| match handler {
                    CatchHandler::Wildcard { body, .. } => find_expr_recursive(&body.node, body.span, target),
                    CatchHandler::Shorthand(body) => find_expr_recursive(&body.node, body.span, target),
                })
        }
        Expr::Range { start, end, .. } => {
            find_expr_recursive(&start.node, start.span, target)
                .or_else(|| find_expr_recursive(&end.node, end.span, target))
        }
        Expr::Spawn { call } => {
            find_expr_recursive(&call.node, call.span, target)
        }
        // Leaf expressions
        _ => None,
    }
}
