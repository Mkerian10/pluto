use serde::Serialize;

use plutoc::parser::ast::{
    AppDecl, ClassDecl, EnumDecl, ErrorDecl, Field, Function, Param, TraitDecl,
    TypeExpr,
};
use plutoc::span::Span;
use plutoc_sdk::decl::{DeclKind, DeclRef};

// --- JSON-serializable output structs ---

#[derive(Serialize)]
pub struct DeclSummary {
    pub name: String,
    pub uuid: String,
    pub kind: String,
}

#[derive(Serialize)]
pub struct ModuleSummary {
    pub path: String,
    pub summary: DeclCounts,
    pub declarations: Vec<DeclSummary>,
}

#[derive(Serialize)]
pub struct DeclCounts {
    pub functions: usize,
    pub classes: usize,
    pub enums: usize,
    pub traits: usize,
    pub errors: usize,
    pub app: usize,
}

#[derive(Serialize)]
pub struct FunctionDetail {
    pub name: String,
    pub uuid: String,
    pub kind: String,
    pub params: Vec<ParamInfo>,
    pub return_type: Option<String>,
    pub is_fallible: bool,
    pub error_set: Vec<ErrorRefInfo>,
    pub signature: Option<SignatureInfo>,
    pub source: String,
}

#[derive(Serialize)]
pub struct ClassDetail {
    pub name: String,
    pub uuid: String,
    pub kind: String,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodSummary>,
    pub bracket_deps: Vec<FieldInfo>,
    pub impl_traits: Vec<String>,
    pub invariant_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_fields: Option<Vec<ResolvedFieldJson>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<String>,
    pub source: String,
}

#[derive(Serialize)]
pub struct EnumDetail {
    pub name: String,
    pub uuid: String,
    pub kind: String,
    pub variants: Vec<VariantInfo>,
    pub type_params: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_variants: Option<Vec<ResolvedVariantJson>>,
    pub source: String,
}

#[derive(Serialize)]
pub struct TraitDetail {
    pub name: String,
    pub uuid: String,
    pub kind: String,
    pub methods: Vec<TraitMethodInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_methods: Option<Vec<ResolvedTraitMethodJson>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implementors: Option<Vec<String>>,
    pub source: String,
}

#[derive(Serialize)]
pub struct ErrorDeclDetail {
    pub name: String,
    pub uuid: String,
    pub kind: String,
    pub fields: Vec<FieldInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_fields: Option<Vec<ResolvedFieldJson>>,
    pub source: String,
}

#[derive(Serialize)]
pub struct AppDetail {
    pub name: String,
    pub uuid: String,
    pub kind: String,
    pub bracket_deps: Vec<FieldInfo>,
    pub methods: Vec<MethodSummary>,
    pub ambient_types: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub di_order: Option<Vec<String>>,
    pub source: String,
}

#[derive(Serialize)]
pub struct ParamInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub type_str: String,
    pub is_mut: bool,
}

#[derive(Serialize)]
pub struct FieldInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub type_str: String,
    pub uuid: String,
}

#[derive(Serialize)]
pub struct MethodSummary {
    pub name: String,
    pub uuid: String,
}

#[derive(Serialize)]
pub struct VariantInfo {
    pub name: String,
    pub uuid: String,
    pub fields: Vec<FieldInfo>,
}

#[derive(Serialize)]
pub struct TraitMethodInfo {
    pub name: String,
    pub uuid: String,
    pub params: Vec<ParamInfo>,
    pub return_type: Option<String>,
    pub has_default_body: bool,
}

#[derive(Serialize)]
pub struct ResolvedFieldJson {
    pub name: String,
    #[serde(rename = "type")]
    pub type_str: String,
    pub is_injected: bool,
}

#[derive(Serialize)]
pub struct ResolvedTraitMethodJson {
    pub name: String,
    pub param_types: Vec<String>,
    pub return_type: String,
    pub is_fallible: bool,
}

#[derive(Serialize)]
pub struct ResolvedVariantJson {
    pub name: String,
    pub fields: Vec<ResolvedFieldJson>,
}

#[derive(Serialize)]
pub struct SignatureInfo {
    pub param_types: Vec<String>,
    pub return_type: String,
    pub is_fallible: bool,
}

#[derive(Serialize)]
pub struct ErrorRefInfo {
    pub name: String,
    pub uuid: Option<String>,
}

#[derive(Serialize)]
pub struct SpanInfo {
    pub start: usize,
    pub end: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_col: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_col: Option<usize>,
}

#[derive(Serialize)]
pub struct XrefSiteInfo {
    pub function_name: String,
    pub function_uuid: Option<String>,
    pub span: SpanInfo,
}

#[derive(Serialize)]
pub struct UnifiedXrefInfo {
    pub usage_kind: String, // "call", "construct", "enum_variant", "raise"
    pub function_name: String,
    pub function_uuid: Option<String>,
    pub span: SpanInfo,
}

#[derive(Serialize)]
pub struct ErrorsResult {
    pub function_name: String,
    pub is_fallible: bool,
    pub error_set: Vec<ErrorRefInfo>,
}

#[derive(Serialize)]
pub struct DisambiguationEntry {
    pub uuid: String,
    pub name: String,
    pub kind: String,
}

// --- Project-level output structs ---

#[derive(Serialize)]
pub struct ProjectSummary {
    pub project_root: String,
    pub files_found: usize,
    pub files_loaded: usize,
    pub files_failed: usize,
    pub modules: Vec<ModuleBrief>,
    pub errors: Vec<LoadError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependency_graph: Option<DependencyGraphInfo>,
}

#[derive(Serialize)]
pub struct DependencyGraphInfo {
    pub module_count: usize,
    pub has_circular_imports: bool,
    pub modules: Vec<ModuleDependencyInfo>,
}

#[derive(Serialize)]
pub struct ModuleDependencyInfo {
    pub path: String,
    pub name: String,
    pub imports: Vec<String>,
}

#[derive(Serialize)]
pub struct ModuleBrief {
    pub path: String,
    pub declarations: usize,
}

#[derive(Serialize)]
pub struct LoadError {
    pub path: String,
    pub error: String,
}

#[derive(Serialize)]
pub struct ModuleListEntry {
    pub path: String,
    pub summary: DeclCounts,
}

#[derive(Serialize)]
pub struct CrossModuleMatch {
    pub module_path: String,
    pub uuid: String,
    pub name: String,
    pub kind: String,
}

// --- Compile/check/run/test result structs ---

#[derive(Serialize)]
pub struct DiagnosticInfo {
    pub severity: String,
    pub kind: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<SpanInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Serialize)]
pub struct CheckResult {
    pub success: bool,
    pub path: String,
    pub errors: Vec<DiagnosticInfo>,
    pub warnings: Vec<DiagnosticInfo>,
}

#[derive(Serialize)]
pub struct CompileResult {
    pub success: bool,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    pub errors: Vec<DiagnosticInfo>,
}

#[derive(Serialize)]
pub struct RunResult {
    pub success: bool,
    pub path: String,
    pub compilation_errors: Vec<DiagnosticInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

#[derive(Serialize)]
pub struct TestResult {
    pub success: bool,
    pub path: String,
    pub compilation_errors: Vec<DiagnosticInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

// --- Write tool result structs ---

#[derive(Serialize)]
pub struct AddDeclResult {
    pub uuid: String,
    pub name: String,
    pub kind: String,
}

#[derive(Serialize)]
pub struct ReplaceDeclResult {
    pub uuid: String,
    pub name: String,
    pub kind: String,
}

#[derive(Serialize)]
pub struct DeleteDeclResult {
    pub deleted_source: String,
    pub dangling_refs: Vec<DanglingRefInfo>,
}

#[derive(Serialize)]
pub struct RenameDeclResult {
    pub old_name: String,
    pub new_name: String,
    pub uuid: String,
}

#[derive(Serialize)]
pub struct AddMethodResult {
    pub uuid: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct AddFieldResult {
    pub uuid: String,
}

#[derive(Serialize)]
pub struct DanglingRefInfo {
    pub kind: String,
    pub name: String,
    pub span: SpanInfo,
}

pub fn compile_error_to_diagnostic(err: &plutoc::diagnostics::CompileError, source: Option<&str>) -> DiagnosticInfo {
    let make_span = |span: Span| -> SpanInfo {
        match source {
            Some(src) => span_to_info_with_source(span, src),
            None => span_to_info(span),
        }
    };
    match err {
        plutoc::diagnostics::CompileError::Syntax { msg, span } => DiagnosticInfo {
            severity: "error".to_string(),
            kind: "syntax".to_string(),
            message: msg.clone(),
            span: Some(make_span(*span)),
            path: None,
        },
        plutoc::diagnostics::CompileError::Type { msg, span } => DiagnosticInfo {
            severity: "error".to_string(),
            kind: "type".to_string(),
            message: msg.clone(),
            span: Some(make_span(*span)),
            path: None,
        },
        plutoc::diagnostics::CompileError::Codegen { msg } => DiagnosticInfo {
            severity: "error".to_string(),
            kind: "codegen".to_string(),
            message: msg.clone(),
            span: None,
            path: None,
        },
        plutoc::diagnostics::CompileError::Link { msg } => DiagnosticInfo {
            severity: "error".to_string(),
            kind: "link".to_string(),
            message: msg.clone(),
            span: None,
            path: None,
        },
        plutoc::diagnostics::CompileError::Manifest { msg, path } => DiagnosticInfo {
            severity: "error".to_string(),
            kind: "manifest".to_string(),
            message: msg.clone(),
            span: None,
            path: Some(path.display().to_string()),
        },
        plutoc::diagnostics::CompileError::SiblingFile { path, source } => {
            // Recursively convert the inner error
            let mut inner = compile_error_to_diagnostic(source, None);
            inner.path = Some(path.display().to_string());
            inner
        },
    }
}

pub fn compile_warning_to_diagnostic(w: &plutoc::diagnostics::CompileWarning, source: Option<&str>) -> DiagnosticInfo {
    let span = match source {
        Some(src) => span_to_info_with_source(w.span, src),
        None => span_to_info(w.span),
    };
    DiagnosticInfo {
        severity: "warning".to_string(),
        kind: format!("{:?}", w.kind).to_lowercase(),
        message: w.msg.clone(),
        span: Some(span),
        path: None,
    }
}

// --- Conversion functions ---

pub fn type_expr_to_string(te: &TypeExpr) -> String {
    match te {
        TypeExpr::Named(n) => n.clone(),
        TypeExpr::Array(inner) => format!("[{}]", type_expr_to_string(&inner.node)),
        TypeExpr::Qualified { module, name } => format!("{module}.{name}"),
        TypeExpr::Fn { params, return_type } => {
            let params_str: Vec<String> =
                params.iter().map(|p| type_expr_to_string(&p.node)).collect();
            let ret = type_expr_to_string(&return_type.node);
            if ret == "void" {
                format!("fn({})", params_str.join(", "))
            } else {
                format!("fn({}) {}", params_str.join(", "), ret)
            }
        }
        TypeExpr::Generic { name, type_args } => {
            let args_str: Vec<String> =
                type_args.iter().map(|a| type_expr_to_string(&a.node)).collect();
            format!("{}<{}>", name, args_str.join(", "))
        }
        TypeExpr::Nullable(inner) => {
            format!("{}?", type_expr_to_string(&inner.node))
        }
        TypeExpr::Stream(inner) => {
            format!("stream {}", type_expr_to_string(&inner.node))
        }
    }
}

fn param_to_info(p: &Param) -> ParamInfo {
    ParamInfo {
        name: p.name.node.clone(),
        type_str: type_expr_to_string(&p.ty.node),
        is_mut: p.is_mut,
    }
}

fn field_to_info(f: &Field) -> FieldInfo {
    FieldInfo {
        name: f.name.node.clone(),
        type_str: type_expr_to_string(&f.ty.node),
        uuid: f.id.to_string(),
    }
}

pub fn decl_kind_to_string(kind: DeclKind) -> &'static str {
    match kind {
        DeclKind::Function => "function",
        DeclKind::Class => "class",
        DeclKind::Enum => "enum",
        DeclKind::EnumVariant => "enum_variant",
        DeclKind::Trait => "trait",
        DeclKind::TraitMethod => "trait_method",
        DeclKind::Error => "error",
        DeclKind::App => "app",
        DeclKind::Field => "field",
        DeclKind::Param => "param",
    }
}

pub fn decl_to_summary(decl: &DeclRef<'_>) -> DeclSummary {
    DeclSummary {
        name: decl.name().to_string(),
        uuid: decl.id().to_string(),
        kind: decl_kind_to_string(decl.kind()).to_string(),
    }
}

pub fn span_to_info(span: Span) -> SpanInfo {
    SpanInfo {
        start: span.start,
        end: span.end,
        start_line: None,
        start_col: None,
        end_line: None,
        end_col: None,
    }
}

/// Convert a byte offset to 1-based (line, col).
pub fn byte_offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

pub fn span_to_info_with_source(span: Span, source: &str) -> SpanInfo {
    let (start_line, start_col) = byte_offset_to_line_col(source, span.start);
    let (end_line, end_col) = byte_offset_to_line_col(source, span.end);
    SpanInfo {
        start: span.start,
        end: span.end,
        start_line: Some(start_line),
        start_col: Some(start_col),
        end_line: Some(end_line),
        end_col: Some(end_col),
    }
}

// --- Pretty-print helpers ---

fn pretty_print_function(func: &Function) -> String {
    let program = plutoc::parser::ast::Program {
        imports: vec![],
        functions: vec![plutoc::span::Spanned::new(func.clone(), plutoc::span::Span::dummy())],
        extern_fns: vec![],
        classes: vec![],
        traits: vec![],
        enums: vec![],
        app: None,
        stages: vec![],
        system: None,
        errors: vec![],
        test_info: vec![],
        tests: None,
        fallible_extern_fns: vec![],
    };
    plutoc::pretty::pretty_print(&program)
}

fn pretty_print_class(cls: &ClassDecl) -> String {
    let program = plutoc::parser::ast::Program {
        imports: vec![],
        functions: vec![],
        extern_fns: vec![],
        classes: vec![plutoc::span::Spanned::new(cls.clone(), plutoc::span::Span::dummy())],
        traits: vec![],
        enums: vec![],
        app: None,
        stages: vec![],
        system: None,
        errors: vec![],
        test_info: vec![],
        tests: None,
        fallible_extern_fns: vec![],
    };
    plutoc::pretty::pretty_print(&program)
}

fn pretty_print_enum(en: &EnumDecl) -> String {
    let program = plutoc::parser::ast::Program {
        imports: vec![],
        functions: vec![],
        extern_fns: vec![],
        classes: vec![],
        traits: vec![],
        enums: vec![plutoc::span::Spanned::new(en.clone(), plutoc::span::Span::dummy())],
        app: None,
        stages: vec![],
        system: None,
        errors: vec![],
        test_info: vec![],
        tests: None,
        fallible_extern_fns: vec![],
    };
    plutoc::pretty::pretty_print(&program)
}

fn pretty_print_trait(tr: &TraitDecl) -> String {
    let program = plutoc::parser::ast::Program {
        imports: vec![],
        functions: vec![],
        extern_fns: vec![],
        classes: vec![],
        traits: vec![plutoc::span::Spanned::new(tr.clone(), plutoc::span::Span::dummy())],
        enums: vec![],
        app: None,
        stages: vec![],
        system: None,
        errors: vec![],
        test_info: vec![],
        tests: None,
        fallible_extern_fns: vec![],
    };
    plutoc::pretty::pretty_print(&program)
}

fn pretty_print_error_decl(err: &ErrorDecl) -> String {
    let program = plutoc::parser::ast::Program {
        imports: vec![],
        functions: vec![],
        extern_fns: vec![],
        classes: vec![],
        traits: vec![],
        enums: vec![],
        app: None,
        stages: vec![],
        system: None,
        errors: vec![plutoc::span::Spanned::new(err.clone(), plutoc::span::Span::dummy())],
        test_info: vec![],
        tests: None,
        fallible_extern_fns: vec![],
    };
    plutoc::pretty::pretty_print(&program)
}

fn pretty_print_app(app: &AppDecl) -> String {
    let program = plutoc::parser::ast::Program {
        imports: vec![],
        functions: vec![],
        extern_fns: vec![],
        classes: vec![],
        traits: vec![],
        enums: vec![],
        app: Some(plutoc::span::Spanned::new(app.clone(), plutoc::span::Span::dummy())),
        stages: vec![],
        system: None,
        errors: vec![],
        test_info: vec![],
        tests: None,
        fallible_extern_fns: vec![],
    };
    plutoc::pretty::pretty_print(&program)
}

// --- High-level detail builders ---

pub fn function_detail(
    func: &Function,
    module: &plutoc_sdk::Module,
) -> FunctionDetail {
    let id = func.id;
    let error_set = module
        .error_set_of(id)
        .iter()
        .map(|e| ErrorRefInfo {
            name: e.name.clone(),
            uuid: e.id.map(|u| u.to_string()),
        })
        .collect();

    let signature = module.signature_of(id).map(|sig| SignatureInfo {
        param_types: sig.param_types.iter().map(|t| format!("{t:?}")).collect(),
        return_type: format!("{:?}", sig.return_type),
        is_fallible: sig.is_fallible,
    });

    FunctionDetail {
        name: func.name.node.clone(),
        uuid: id.to_string(),
        kind: "function".to_string(),
        params: func.params.iter().map(param_to_info).collect(),
        return_type: func.return_type.as_ref().map(|rt| type_expr_to_string(&rt.node)),
        is_fallible: module.is_fallible(id),
        error_set,
        signature,
        source: pretty_print_function(func),
    }
}

pub fn class_detail(cls: &ClassDecl, module: &plutoc_sdk::Module) -> ClassDetail {
    let regular_fields: Vec<FieldInfo> = cls
        .fields
        .iter()
        .filter(|f| !f.is_injected)
        .map(field_to_info)
        .collect();

    let bracket_deps: Vec<FieldInfo> = cls
        .fields
        .iter()
        .filter(|f| f.is_injected)
        .map(field_to_info)
        .collect();

    let resolved = module.class_info_of(cls.id);
    let resolved_fields = resolved.map(|ci| {
        ci.fields
            .iter()
            .map(|f| ResolvedFieldJson {
                name: f.name.clone(),
                type_str: format!("{}", f.ty),
                is_injected: f.is_injected,
            })
            .collect()
    });
    let lifecycle = resolved.map(|ci| format!("{:?}", ci.lifecycle));

    ClassDetail {
        name: cls.name.node.clone(),
        uuid: cls.id.to_string(),
        kind: "class".to_string(),
        fields: regular_fields,
        methods: cls
            .methods
            .iter()
            .map(|m| MethodSummary {
                name: m.node.name.node.clone(),
                uuid: m.node.id.to_string(),
            })
            .collect(),
        bracket_deps,
        impl_traits: cls.impl_traits.iter().map(|t| t.node.clone()).collect(),
        invariant_count: cls.invariants.len(),
        resolved_fields,
        lifecycle,
        source: pretty_print_class(cls),
    }
}

pub fn enum_detail(en: &EnumDecl, module: &plutoc_sdk::Module) -> EnumDetail {
    let resolved_variants = module.enum_info_of(en.id).map(|ei| {
        ei.variants
            .iter()
            .map(|v| ResolvedVariantJson {
                name: v.name.clone(),
                fields: v
                    .fields
                    .iter()
                    .map(|f| ResolvedFieldJson {
                        name: f.name.clone(),
                        type_str: format!("{}", f.ty),
                        is_injected: f.is_injected,
                    })
                    .collect(),
            })
            .collect()
    });

    EnumDetail {
        name: en.name.node.clone(),
        uuid: en.id.to_string(),
        kind: "enum".to_string(),
        variants: en
            .variants
            .iter()
            .map(|v| VariantInfo {
                name: v.name.node.clone(),
                uuid: v.id.to_string(),
                fields: v.fields.iter().map(field_to_info).collect(),
            })
            .collect(),
        type_params: en.type_params.iter().map(|tp| tp.node.clone()).collect(),
        resolved_variants,
        source: pretty_print_enum(en),
    }
}

pub fn trait_detail(tr: &TraitDecl, module: &plutoc_sdk::Module) -> TraitDetail {
    let resolved = module.trait_info_of(tr.id);
    let resolved_methods = resolved.map(|ti| {
        ti.methods
            .iter()
            .map(|(name, sig)| ResolvedTraitMethodJson {
                name: name.clone(),
                param_types: sig.param_types.iter().map(|t| format!("{}", t)).collect(),
                return_type: format!("{}", sig.return_type),
                is_fallible: sig.is_fallible,
            })
            .collect()
    });
    let implementors = resolved.map(|ti| {
        ti.implementors
            .iter()
            .map(|uuid| uuid.to_string())
            .collect()
    });

    TraitDetail {
        name: tr.name.node.clone(),
        uuid: tr.id.to_string(),
        kind: "trait".to_string(),
        methods: tr
            .methods
            .iter()
            .map(|m| TraitMethodInfo {
                name: m.name.node.clone(),
                uuid: m.id.to_string(),
                params: m.params.iter().map(param_to_info).collect(),
                return_type: m.return_type.as_ref().map(|rt| type_expr_to_string(&rt.node)),
                has_default_body: m.body.is_some(),
            })
            .collect(),
        resolved_methods,
        implementors,
        source: pretty_print_trait(tr),
    }
}

pub fn error_decl_detail(err: &ErrorDecl, module: &plutoc_sdk::Module) -> ErrorDeclDetail {
    let resolved_fields = module.error_info_of(err.id).map(|ei| {
        ei.fields
            .iter()
            .map(|f| ResolvedFieldJson {
                name: f.name.clone(),
                type_str: format!("{}", f.ty),
                is_injected: f.is_injected,
            })
            .collect()
    });

    ErrorDeclDetail {
        name: err.name.node.clone(),
        uuid: err.id.to_string(),
        kind: "error".to_string(),
        fields: err.fields.iter().map(field_to_info).collect(),
        resolved_fields,
        source: pretty_print_error_decl(err),
    }
}

pub fn app_detail(app: &AppDecl, module: &plutoc_sdk::Module) -> AppDetail {
    let di_order = {
        let order = module.di_order();
        if order.is_empty() {
            None
        } else {
            Some(order.iter().map(|uuid| uuid.to_string()).collect())
        }
    };

    AppDetail {
        name: app.name.node.clone(),
        uuid: app.id.to_string(),
        kind: "app".to_string(),
        bracket_deps: app.inject_fields.iter().map(field_to_info).collect(),
        methods: app
            .methods
            .iter()
            .map(|m| MethodSummary {
                name: m.node.name.node.clone(),
                uuid: m.node.id.to_string(),
            })
            .collect(),
        ambient_types: app.ambient_types.iter().map(|a| a.node.clone()).collect(),
        di_order,
        source: pretty_print_app(app),
    }
}
