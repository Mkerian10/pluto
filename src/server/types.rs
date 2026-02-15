//! Type definitions for the CompilerService API.
//!
//! This module defines all result types, options, and data structures used by the
//! CompilerService trait. These types are protocol-agnostic and can be formatted
//! for different frontends (CLI, MCP, socket server, etc.).

use crate::diagnostics::{CompileError, CompileWarning};
use crate::span::Span;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;
use thiserror::Error;
use uuid::Uuid;

// ========== Options Structs ==========

/// Options for loading modules.
#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
    pub stdlib: Option<PathBuf>,
}

/// Options for compilation operations.
#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    pub stdlib: Option<PathBuf>,
    pub gc: bool,
    pub coverage: bool,
}

/// Options for running programs.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    pub stdlib: Option<PathBuf>,
    pub timeout_ms: Option<u64>,
    pub cwd: Option<PathBuf>,
}

/// Options for test execution.
#[derive(Debug, Clone, Default)]
pub struct TestOptions {
    pub stdlib: Option<PathBuf>,
    pub timeout_ms: Option<u64>,
    pub cwd: Option<PathBuf>,
}

/// Options for call graph generation.
#[derive(Debug, Clone)]
pub struct CallGraphOptions {
    pub direction: CallGraphDirection,
    pub max_depth: usize,
}

impl Default for CallGraphOptions {
    fn default() -> Self {
        Self {
            direction: CallGraphDirection::Callees,
            max_depth: 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CallGraphDirection {
    Callers,
    Callees,
}

// ========== Result Types ==========

/// Result of type-checking a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub success: bool,
    pub path: PathBuf,
    pub errors: Vec<Diagnostic>,
    pub warnings: Vec<Diagnostic>,
}

/// Result of compiling a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileResult {
    pub success: bool,
    pub path: PathBuf,
    pub output: Option<PathBuf>,
    pub errors: Vec<Diagnostic>,
    pub warnings: Vec<Diagnostic>,
}

/// Result of running a program.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub success: bool,
    pub path: PathBuf,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub compile_errors: Vec<Diagnostic>,
}

/// Result of running tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub success: bool,
    pub path: PathBuf,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub compile_errors: Vec<Diagnostic>,
}

/// Diagnostic message (error or warning).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub message: String,
    pub span: Option<DiagnosticSpan>,
    pub severity: DiagnosticSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticSpan {
    pub start: usize,
    pub end: usize,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

impl Diagnostic {
    pub fn from_compile_error(err: &CompileError, source: Option<&str>) -> Self {
        let (message, span) = match err {
            CompileError::Syntax { msg, span } => (msg.clone(), Some(*span)),
            CompileError::Type { msg, span } => (msg.clone(), Some(*span)),
            CompileError::Codegen { msg } => (msg.clone(), None),
            CompileError::Link { msg } => (msg.clone(), None),
            CompileError::Manifest { msg, .. } => (msg.clone(), None),
            CompileError::SiblingFile { source, .. } => {
                return Self::from_compile_error(source, None);
            }
        };

        Self {
            message,
            span: span.map(|s| DiagnosticSpan::from_span(s, source)),
            severity: DiagnosticSeverity::Error,
        }
    }

    pub fn from_compile_warning(warning: &CompileWarning, source: Option<&str>) -> Self {
        Self {
            message: warning.msg.clone(),
            span: Some(DiagnosticSpan::from_span(warning.span, source)),
            severity: DiagnosticSeverity::Warning,
        }
    }
}

impl DiagnosticSpan {
    fn from_span(span: Span, source: Option<&str>) -> Self {
        let (line, column) = if let Some(src) = source {
            let line = src[..span.start].chars().filter(|c| *c == '\n').count() + 1;
            let col_start = src[..span.start].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let column = src[col_start..span.start].chars().count() + 1;
            (Some(line), Some(column))
        } else {
            (None, None)
        };

        Self {
            start: span.start,
            end: span.end,
            line,
            column,
        }
    }
}

// ========== Module Types ==========

/// Summary of a loaded module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSummary {
    pub path: PathBuf,
    pub name: String,
    pub function_count: usize,
    pub class_count: usize,
    pub enum_count: usize,
    pub trait_count: usize,
    pub error_count: usize,
    pub app_count: usize,
}

/// Information about a loaded module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub path: PathBuf,
    pub name: String,
    pub loaded_at: SystemTime,
}

/// Status of a module (including staleness).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleStatus {
    pub path: PathBuf,
    pub name: String,
    pub loaded_at: SystemTime,
    pub is_stale: bool,
}

/// Summary of a loaded project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub root: PathBuf,
    pub loaded: Vec<PathBuf>,
    pub failed: Vec<(PathBuf, String)>,
}

// ========== Declaration Types ==========

/// Declaration kind filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeclKind {
    Function,
    Class,
    Enum,
    Trait,
    Error,
    App,
}

/// Summary of a declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclSummary {
    pub uuid: Uuid,
    pub name: String,
    pub kind: DeclKind,
}

/// Detailed declaration information (tagged enum).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DeclDetail {
    Function(FunctionDetail),
    Class(ClassDetail),
    Enum(EnumDetail),
    Trait(TraitDetail),
    Error(ErrorDetail),
    App(AppDetail),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDetail {
    pub uuid: Uuid,
    pub name: String,
    pub params: Vec<ParamInfo>,
    pub return_type: String,
    pub is_fallible: bool,
    pub error_set: Vec<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamInfo {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassDetail {
    pub uuid: Uuid,
    pub name: String,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInfo {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodInfo {
    pub uuid: Uuid,
    pub name: String,
    pub params: Vec<ParamInfo>,
    pub return_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumDetail {
    pub uuid: Uuid,
    pub name: String,
    pub variants: Vec<VariantInfo>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantInfo {
    pub name: String,
    pub fields: Vec<FieldInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitDetail {
    pub uuid: Uuid,
    pub name: String,
    pub methods: Vec<MethodSignature>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodSignature {
    pub name: String,
    pub params: Vec<ParamInfo>,
    pub return_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub uuid: Uuid,
    pub name: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppDetail {
    pub uuid: Uuid,
    pub name: String,
    pub deps: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
    pub source: String,
}

/// Match result when finding declarations by name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclMatch {
    pub uuid: Uuid,
    pub name: String,
    pub kind: DeclKind,
    pub module_path: PathBuf,
}

// ========== Cross-Reference Types ==========

/// Cross-reference site (call site, usage, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XrefSite {
    pub module_path: PathBuf,
    pub span: DiagnosticSpan,
    pub context: Option<String>,
}

/// Call graph result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallGraphResult {
    pub root: CallGraphNode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallGraphNode {
    pub uuid: Uuid,
    pub name: String,
    pub module_path: PathBuf,
    pub children: Vec<CallGraphNode>,
    pub is_cycle: bool,
}

/// Error set information for a function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSetInfo {
    pub is_fallible: bool,
    pub errors: Vec<String>,
}

// ========== Editing Types ==========

/// Result of an edit operation (add, replace, rename).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditResult {
    pub uuid: Uuid,
    pub name: String,
    pub kind: DeclKind,
}

/// Result of a delete operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResult {
    pub name: String,
    pub deleted_source: String,
    pub dangling_references: Vec<XrefSite>,
}

/// Result of a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub pluto_path: PathBuf,
    pub preserved_uuids: usize,
    pub new_declarations: usize,
}

// ========== Utility Types ==========

/// Byte range for source access.
#[derive(Debug, Clone, Copy)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

// ========== Error Types ==========

/// Service-level errors.
#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Module not found: {0}")]
    ModuleNotFound(PathBuf),

    #[error("Declaration not found: {0}")]
    DeclarationNotFound(Uuid),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Compilation failed: {0}")]
    CompilationFailed(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<CompileError> for ServiceError {
    fn from(err: CompileError) -> Self {
        ServiceError::CompilationFailed(err.to_string())
    }
}
