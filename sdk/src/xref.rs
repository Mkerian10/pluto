use pluto::parser::ast::{Expr, Function, Stmt};
use pluto::span::Span;
use uuid::Uuid;

/// A call site where a function is invoked.
pub struct CallSite<'a> {
    pub caller: &'a Function,
    pub call_expr: &'a Expr,
    pub target_id: Uuid,
    pub span: Span,
}

/// A site where a class is constructed via struct literal.
pub struct ConstructSite<'a> {
    pub function: &'a Function,
    pub struct_lit: &'a Expr,
    pub target_id: Uuid,
    pub span: Span,
}

/// A site where an enum variant is used (unit or data).
pub struct EnumUsageSite<'a> {
    pub function: &'a Function,
    pub expr: &'a Expr,
    pub enum_id: Uuid,
    pub variant_id: Uuid,
    pub span: Span,
}

/// A site where an error is raised.
pub struct RaiseSite<'a> {
    pub function: &'a Function,
    pub stmt: &'a Stmt,
    pub error_id: Uuid,
    pub span: Span,
}

// Owned (non-borrowing) variants for the index — store enough info to reconstruct
// the borrowed versions when queried through Module.

#[derive(Debug, Clone)]
pub(crate) struct CallSiteInfo {
    pub fn_name: String,
    pub span: Span,
    pub target_id: Uuid,
}

#[derive(Debug, Clone)]
pub(crate) struct ConstructSiteInfo {
    pub fn_name: String,
    pub span: Span,
    pub target_id: Uuid,
}

#[derive(Debug, Clone)]
pub(crate) struct EnumUsageSiteInfo {
    pub fn_name: String,
    pub span: Span,
    pub enum_id: Uuid,
    pub variant_id: Uuid,
}

#[derive(Debug, Clone)]
pub(crate) struct RaiseSiteInfo {
    pub fn_name: String,
    pub span: Span,
    pub error_id: Uuid,
}
