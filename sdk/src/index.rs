use std::collections::HashMap;
use uuid::Uuid;

use plutoc::parser::ast::*;
use plutoc::span::Span;

use crate::decl::DeclKind;
use crate::xref::{CallSiteInfo, ConstructSiteInfo, EnumUsageSiteInfo, RaiseSiteInfo};

/// Location of a declaration within the Program's vectors.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DeclLocation {
    pub kind: DeclKind,
    pub index: usize,
    /// For nested items: parent vector index.
    pub parent_index: Option<usize>,
}

/// Pre-built indexes for O(1) lookups into a Program.
pub struct ModuleIndex {
    // Forward indexes
    pub(crate) by_uuid: HashMap<Uuid, DeclLocation>,
    pub(crate) by_name: HashMap<String, Vec<Uuid>>,

    // Reverse cross-reference indexes
    pub(crate) callers: HashMap<Uuid, Vec<CallSiteInfo>>,
    pub(crate) constructors: HashMap<Uuid, Vec<ConstructSiteInfo>>,
    pub(crate) enum_usages: HashMap<Uuid, Vec<EnumUsageSiteInfo>>,
    pub(crate) raise_sites: HashMap<Uuid, Vec<RaiseSiteInfo>>,
}

impl ModuleIndex {
    /// Build all indexes by walking the Program AST once.
    pub fn build(program: &Program) -> Self {
        let mut by_uuid = HashMap::new();
        let mut by_name: HashMap<String, Vec<Uuid>> = HashMap::new();

        // Index top-level functions
        for (i, f) in program.functions.iter().enumerate() {
            let loc = DeclLocation { kind: DeclKind::Function, index: i, parent_index: None };
            by_uuid.insert(f.node.id, loc);
            by_name.entry(f.node.name.node.clone()).or_default().push(f.node.id);

            // Index params
            for (pi, p) in f.node.params.iter().enumerate() {
                let ploc = DeclLocation { kind: DeclKind::Param, index: pi, parent_index: Some(i) };
                by_uuid.insert(p.id, ploc);
                by_name.entry(p.name.node.clone()).or_default().push(p.id);
            }
        }

        // Index classes + their fields, methods, params
        for (i, c) in program.classes.iter().enumerate() {
            let loc = DeclLocation { kind: DeclKind::Class, index: i, parent_index: None };
            by_uuid.insert(c.node.id, loc);
            by_name.entry(c.node.name.node.clone()).or_default().push(c.node.id);

            for (fi, field) in c.node.fields.iter().enumerate() {
                let floc = DeclLocation { kind: DeclKind::Field, index: fi, parent_index: Some(i) };
                by_uuid.insert(field.id, floc);
                by_name.entry(field.name.node.clone()).or_default().push(field.id);
            }

            for (mi, method) in c.node.methods.iter().enumerate() {
                // Use a unique index space: methods start after top-level functions
                // but we store class_index in parent_index
                let mloc = DeclLocation { kind: DeclKind::Function, index: mi, parent_index: Some(i) };
                by_uuid.insert(method.node.id, mloc);
                by_name.entry(method.node.name.node.clone()).or_default().push(method.node.id);

                for (pi, p) in method.node.params.iter().enumerate() {
                    let ploc = DeclLocation { kind: DeclKind::Param, index: pi, parent_index: Some(i) };
                    by_uuid.insert(p.id, ploc);
                }
            }
        }

        // Index enums + variants
        for (i, e) in program.enums.iter().enumerate() {
            let loc = DeclLocation { kind: DeclKind::Enum, index: i, parent_index: None };
            by_uuid.insert(e.node.id, loc);
            by_name.entry(e.node.name.node.clone()).or_default().push(e.node.id);

            for (vi, v) in e.node.variants.iter().enumerate() {
                let vloc = DeclLocation { kind: DeclKind::EnumVariant, index: vi, parent_index: Some(i) };
                by_uuid.insert(v.id, vloc);
                by_name.entry(v.name.node.clone()).or_default().push(v.id);
            }
        }

        // Index traits + methods
        for (i, t) in program.traits.iter().enumerate() {
            let loc = DeclLocation { kind: DeclKind::Trait, index: i, parent_index: None };
            by_uuid.insert(t.node.id, loc);
            by_name.entry(t.node.name.node.clone()).or_default().push(t.node.id);

            for (mi, m) in t.node.methods.iter().enumerate() {
                let mloc = DeclLocation { kind: DeclKind::TraitMethod, index: mi, parent_index: Some(i) };
                by_uuid.insert(m.id, mloc);
                by_name.entry(m.name.node.clone()).or_default().push(m.id);
            }
        }

        // Index errors
        for (i, e) in program.errors.iter().enumerate() {
            let loc = DeclLocation { kind: DeclKind::Error, index: i, parent_index: None };
            by_uuid.insert(e.node.id, loc);
            by_name.entry(e.node.name.node.clone()).or_default().push(e.node.id);

            for (fi, field) in e.node.fields.iter().enumerate() {
                let floc = DeclLocation { kind: DeclKind::Field, index: fi, parent_index: Some(i) };
                by_uuid.insert(field.id, floc);
                by_name.entry(field.name.node.clone()).or_default().push(field.id);
            }
        }

        // Index app
        if let Some(app) = &program.app {
            let loc = DeclLocation { kind: DeclKind::App, index: 0, parent_index: None };
            by_uuid.insert(app.node.id, loc);
            by_name.entry(app.node.name.node.clone()).or_default().push(app.node.id);

            for (fi, field) in app.node.inject_fields.iter().enumerate() {
                let floc = DeclLocation { kind: DeclKind::Field, index: fi, parent_index: Some(0) };
                by_uuid.insert(field.id, floc);
                by_name.entry(field.name.node.clone()).or_default().push(field.id);
            }

            for (mi, method) in app.node.methods.iter().enumerate() {
                let mloc = DeclLocation { kind: DeclKind::Function, index: mi, parent_index: Some(0) };
                by_uuid.insert(method.node.id, mloc);
                by_name.entry(method.node.name.node.clone()).or_default().push(method.node.id);
            }
        }

        // Build reverse xref indexes
        let mut callers: HashMap<Uuid, Vec<CallSiteInfo>> = HashMap::new();
        let mut constructors: HashMap<Uuid, Vec<ConstructSiteInfo>> = HashMap::new();
        let mut enum_usages: HashMap<Uuid, Vec<EnumUsageSiteInfo>> = HashMap::new();
        let mut raise_sites: HashMap<Uuid, Vec<RaiseSiteInfo>> = HashMap::new();

        // Walk all function bodies for xrefs
        for f in &program.functions {
            collect_block_xrefs(
                &f.node.body.node,
                &f.node.name.node,
                &mut callers, &mut constructors, &mut enum_usages, &mut raise_sites,
            );
        }
        for c in &program.classes {
            for m in &c.node.methods {
                let mangled = format!("{}_{}", c.node.name.node, m.node.name.node);
                collect_block_xrefs(
                    &m.node.body.node,
                    &mangled,
                    &mut callers, &mut constructors, &mut enum_usages, &mut raise_sites,
                );
            }
        }
        if let Some(app) = &program.app {
            for m in &app.node.methods {
                let mangled = format!("{}_{}", app.node.name.node, m.node.name.node);
                collect_block_xrefs(
                    &m.node.body.node,
                    &mangled,
                    &mut callers, &mut constructors, &mut enum_usages, &mut raise_sites,
                );
            }
        }

        ModuleIndex {
            by_uuid,
            by_name,
            callers,
            constructors,
            enum_usages,
            raise_sites,
        }
    }
}

fn collect_block_xrefs(
    block: &Block,
    fn_name: &str,
    callers: &mut HashMap<Uuid, Vec<CallSiteInfo>>,
    constructors: &mut HashMap<Uuid, Vec<ConstructSiteInfo>>,
    enum_usages: &mut HashMap<Uuid, Vec<EnumUsageSiteInfo>>,
    raise_sites: &mut HashMap<Uuid, Vec<RaiseSiteInfo>>,
) {
    for stmt in &block.stmts {
        collect_stmt_xrefs(&stmt.node, stmt.span, fn_name, callers, constructors, enum_usages, raise_sites);
    }
}

fn collect_stmt_xrefs(
    stmt: &Stmt,
    stmt_span: Span,
    fn_name: &str,
    callers: &mut HashMap<Uuid, Vec<CallSiteInfo>>,
    constructors: &mut HashMap<Uuid, Vec<ConstructSiteInfo>>,
    enum_usages: &mut HashMap<Uuid, Vec<EnumUsageSiteInfo>>,
    raise_sites: &mut HashMap<Uuid, Vec<RaiseSiteInfo>>,
) {
    match stmt {
        Stmt::Let { value, .. } => {
            collect_expr_xrefs(&value.node, value.span, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        Stmt::Return(Some(expr)) => {
            collect_expr_xrefs(&expr.node, expr.span, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => {
            collect_expr_xrefs(&value.node, value.span, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        Stmt::FieldAssign { object, value, .. } => {
            collect_expr_xrefs(&object.node, object.span, fn_name, callers, constructors, enum_usages, raise_sites);
            collect_expr_xrefs(&value.node, value.span, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        Stmt::If { condition, then_block, else_block } => {
            collect_expr_xrefs(&condition.node, condition.span, fn_name, callers, constructors, enum_usages, raise_sites);
            collect_block_xrefs(&then_block.node, fn_name, callers, constructors, enum_usages, raise_sites);
            if let Some(eb) = else_block {
                collect_block_xrefs(&eb.node, fn_name, callers, constructors, enum_usages, raise_sites);
            }
        }
        Stmt::While { condition, body } => {
            collect_expr_xrefs(&condition.node, condition.span, fn_name, callers, constructors, enum_usages, raise_sites);
            collect_block_xrefs(&body.node, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        Stmt::For { iterable, body, .. } => {
            collect_expr_xrefs(&iterable.node, iterable.span, fn_name, callers, constructors, enum_usages, raise_sites);
            collect_block_xrefs(&body.node, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        Stmt::IndexAssign { object, index, value } => {
            collect_expr_xrefs(&object.node, object.span, fn_name, callers, constructors, enum_usages, raise_sites);
            collect_expr_xrefs(&index.node, index.span, fn_name, callers, constructors, enum_usages, raise_sites);
            collect_expr_xrefs(&value.node, value.span, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        Stmt::Match { expr, arms } => {
            collect_expr_xrefs(&expr.node, expr.span, fn_name, callers, constructors, enum_usages, raise_sites);
            for arm in arms {
                collect_block_xrefs(&arm.body.node, fn_name, callers, constructors, enum_usages, raise_sites);
            }
        }
        Stmt::Raise { error_id, error_name, fields, .. } => {
            if let Some(eid) = error_id {
                raise_sites.entry(*eid).or_default().push(RaiseSiteInfo {
                    fn_name: fn_name.to_string(),
                    span: stmt_span,
                    error_id: *eid,
                });
            }
            for (_, expr) in fields {
                collect_expr_xrefs(&expr.node, expr.span, fn_name, callers, constructors, enum_usages, raise_sites);
            }
            // suppress unused warning
            let _ = error_name;
        }
        Stmt::LetChan { capacity, .. } => {
            if let Some(cap) = capacity {
                collect_expr_xrefs(&cap.node, cap.span, fn_name, callers, constructors, enum_usages, raise_sites);
            }
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &arm.op {
                    SelectOp::Recv { channel, .. } => {
                        collect_expr_xrefs(&channel.node, channel.span, fn_name, callers, constructors, enum_usages, raise_sites);
                    }
                    SelectOp::Send { channel, value } => {
                        collect_expr_xrefs(&channel.node, channel.span, fn_name, callers, constructors, enum_usages, raise_sites);
                        collect_expr_xrefs(&value.node, value.span, fn_name, callers, constructors, enum_usages, raise_sites);
                    }
                }
                collect_block_xrefs(&arm.body.node, fn_name, callers, constructors, enum_usages, raise_sites);
            }
            if let Some(def) = default {
                collect_block_xrefs(&def.node, fn_name, callers, constructors, enum_usages, raise_sites);
            }
        }
        Stmt::Break | Stmt::Continue => {}
        Stmt::Expr(expr) => {
            collect_expr_xrefs(&expr.node, expr.span, fn_name, callers, constructors, enum_usages, raise_sites);
        }
    }
}

fn collect_expr_xrefs(
    expr: &Expr,
    expr_span: Span,
    fn_name: &str,
    callers: &mut HashMap<Uuid, Vec<CallSiteInfo>>,
    constructors: &mut HashMap<Uuid, Vec<ConstructSiteInfo>>,
    enum_usages: &mut HashMap<Uuid, Vec<EnumUsageSiteInfo>>,
    raise_sites: &mut HashMap<Uuid, Vec<RaiseSiteInfo>>,
) {
    match expr {
        Expr::Call { target_id: Some(tid), args, .. } => {
            callers.entry(*tid).or_default().push(CallSiteInfo {
                fn_name: fn_name.to_string(),
                span: expr_span,
                target_id: *tid,
            });
            for arg in args {
                collect_expr_xrefs(&arg.node, arg.span, fn_name, callers, constructors, enum_usages, raise_sites);
            }
        }
        Expr::Call { target_id: None, args, .. } => {
            for arg in args {
                collect_expr_xrefs(&arg.node, arg.span, fn_name, callers, constructors, enum_usages, raise_sites);
            }
        }
        Expr::StructLit { target_id: Some(tid), fields, .. } => {
            constructors.entry(*tid).or_default().push(ConstructSiteInfo {
                fn_name: fn_name.to_string(),
                span: expr_span,
                target_id: *tid,
            });
            for (_, fexpr) in fields {
                collect_expr_xrefs(&fexpr.node, fexpr.span, fn_name, callers, constructors, enum_usages, raise_sites);
            }
        }
        Expr::StructLit { target_id: None, fields, .. } => {
            for (_, fexpr) in fields {
                collect_expr_xrefs(&fexpr.node, fexpr.span, fn_name, callers, constructors, enum_usages, raise_sites);
            }
        }
        Expr::EnumUnit { enum_id: Some(eid), variant_id: Some(vid), .. } => {
            enum_usages.entry(*eid).or_default().push(EnumUsageSiteInfo {
                fn_name: fn_name.to_string(),
                span: expr_span,
                enum_id: *eid,
                variant_id: *vid,
            });
        }
        Expr::EnumData { enum_id: Some(eid), variant_id: Some(vid), fields, .. } => {
            enum_usages.entry(*eid).or_default().push(EnumUsageSiteInfo {
                fn_name: fn_name.to_string(),
                span: expr_span,
                enum_id: *eid,
                variant_id: *vid,
            });
            for (_, fexpr) in fields {
                collect_expr_xrefs(&fexpr.node, fexpr.span, fn_name, callers, constructors, enum_usages, raise_sites);
            }
        }
        Expr::ClosureCreate { target_id: Some(tid), .. } => {
            callers.entry(*tid).or_default().push(CallSiteInfo {
                fn_name: fn_name.to_string(),
                span: expr_span,
                target_id: *tid,
            });
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_expr_xrefs(&lhs.node, lhs.span, fn_name, callers, constructors, enum_usages, raise_sites);
            collect_expr_xrefs(&rhs.node, rhs.span, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_expr_xrefs(&operand.node, operand.span, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        Expr::FieldAccess { object, .. } => {
            collect_expr_xrefs(&object.node, object.span, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        Expr::MethodCall { object, args, .. } => {
            collect_expr_xrefs(&object.node, object.span, fn_name, callers, constructors, enum_usages, raise_sites);
            for arg in args {
                collect_expr_xrefs(&arg.node, arg.span, fn_name, callers, constructors, enum_usages, raise_sites);
            }
        }
        Expr::ArrayLit { elements } => {
            for el in elements {
                collect_expr_xrefs(&el.node, el.span, fn_name, callers, constructors, enum_usages, raise_sites);
            }
        }
        Expr::Index { object, index } => {
            collect_expr_xrefs(&object.node, object.span, fn_name, callers, constructors, enum_usages, raise_sites);
            collect_expr_xrefs(&index.node, index.span, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    collect_expr_xrefs(&e.node, e.span, fn_name, callers, constructors, enum_usages, raise_sites);
                }
            }
        }
        Expr::Closure { body, .. } => {
            collect_block_xrefs(&body.node, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_expr_xrefs(&k.node, k.span, fn_name, callers, constructors, enum_usages, raise_sites);
                collect_expr_xrefs(&v.node, v.span, fn_name, callers, constructors, enum_usages, raise_sites);
            }
        }
        Expr::SetLit { elements, .. } => {
            for el in elements {
                collect_expr_xrefs(&el.node, el.span, fn_name, callers, constructors, enum_usages, raise_sites);
            }
        }
        Expr::Propagate { expr } => {
            collect_expr_xrefs(&expr.node, expr.span, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        Expr::Catch { expr: inner, handler } => {
            collect_expr_xrefs(&inner.node, inner.span, fn_name, callers, constructors, enum_usages, raise_sites);
            match handler {
                CatchHandler::Wildcard { body, .. } => {
                    collect_block_xrefs(&body.node, fn_name, callers, constructors, enum_usages, raise_sites);
                }
                CatchHandler::Shorthand(body) => {
                    collect_expr_xrefs(&body.node, body.span, fn_name, callers, constructors, enum_usages, raise_sites);
                }
            }
        }
        Expr::Cast { expr: inner, .. } => {
            collect_expr_xrefs(&inner.node, inner.span, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        Expr::Range { start, end, .. } => {
            collect_expr_xrefs(&start.node, start.span, fn_name, callers, constructors, enum_usages, raise_sites);
            collect_expr_xrefs(&end.node, end.span, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        Expr::Spawn { call } => {
            collect_expr_xrefs(&call.node, call.span, fn_name, callers, constructors, enum_usages, raise_sites);
        }
        // Leaf expressions and unresolved xrefs
        _ => {}
    }
}
