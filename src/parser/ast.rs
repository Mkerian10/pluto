use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use crate::span::Spanned;

#[derive(Debug, Serialize, Deserialize)]
pub struct Program {
    pub imports: Vec<Spanned<ImportDecl>>,
    pub functions: Vec<Spanned<Function>>,
    pub extern_fns: Vec<Spanned<ExternFnDecl>>,
    pub extern_rust_crates: Vec<Spanned<ExternRustDecl>>,
    pub classes: Vec<Spanned<ClassDecl>>,
    pub traits: Vec<Spanned<TraitDecl>>,
    pub enums: Vec<Spanned<EnumDecl>>,
    pub app: Option<Spanned<AppDecl>>,
    pub stages: Vec<Spanned<StageDecl>>,
    pub system: Option<Spanned<SystemDecl>>,
    pub errors: Vec<Spanned<ErrorDecl>>,
    pub test_info: Vec<TestInfo>,
    pub tests: Option<Spanned<TestsDecl>>,
    pub fallible_extern_fns: Vec<String>,  // populated by rust_ffi::inject_extern_fns for Result-returning FFI fns
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestsDecl {
    pub id: Uuid,
    pub strategy: String,  // "Sequential", "RoundRobin", "Random"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestInfo {
    pub display_name: String,
    pub fn_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportDecl {
    pub path: Vec<Spanned<String>>,
    pub alias: Option<Spanned<String>>,
}

impl ImportDecl {
    pub fn binding_name(&self) -> &str {
        if let Some(alias) = &self.alias {
            &alias.node
        } else {
            &self.path.last().unwrap().node
        }
    }

    pub fn full_path(&self) -> String {
        self.path.iter().map(|s| s.node.as_str()).collect::<Vec<_>>().join(".")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternFnDecl {
    pub name: Spanned<String>,
    pub params: Vec<Param>,
    pub return_type: Option<Spanned<TypeExpr>>,
    pub is_pub: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternRustDecl {
    pub crate_path: Spanned<String>,
    pub alias: Spanned<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Lifecycle {
    Singleton,  // default
    Scoped,
    Transient,
}

impl std::fmt::Display for Lifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Lifecycle::Singleton => write!(f, "singleton"),
            Lifecycle::Scoped => write!(f, "scoped"),
            Lifecycle::Transient => write!(f, "transient"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassDecl {
    pub id: Uuid,
    pub name: Spanned<String>,
    pub type_params: Vec<Spanned<String>>,
    pub type_param_bounds: HashMap<String, Vec<Spanned<String>>>,
    pub fields: Vec<Field>,
    pub methods: Vec<Spanned<Function>>,
    pub invariants: Vec<Spanned<ContractClause>>,
    pub impl_traits: Vec<Spanned<String>>,
    pub uses: Vec<Spanned<String>>,
    pub is_pub: bool,
    pub lifecycle: Lifecycle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub id: Uuid,
    pub name: Spanned<String>,
    pub ty: Spanned<TypeExpr>,
    pub is_injected: bool,
    pub is_ambient: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppDecl {
    pub id: Uuid,
    pub name: Spanned<String>,
    pub inject_fields: Vec<Field>,
    pub ambient_types: Vec<Spanned<String>>,
    pub lifecycle_overrides: Vec<(Spanned<String>, Lifecycle)>,
    pub methods: Vec<Spanned<Function>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredMethod {
    pub id: Uuid,
    pub name: Spanned<String>,
    pub params: Vec<Param>,
    pub return_type: Option<Spanned<TypeExpr>>,
    pub is_pub: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageDecl {
    pub id: Uuid,
    pub name: Spanned<String>,
    pub parent: Option<Spanned<String>>,
    pub inject_fields: Vec<Field>,
    pub ambient_types: Vec<Spanned<String>>,
    pub lifecycle_overrides: Vec<(Spanned<String>, Lifecycle)>,
    pub required_methods: Vec<Spanned<RequiredMethod>>,
    pub methods: Vec<Spanned<Function>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMember {
    pub id: Uuid,
    pub name: Spanned<String>,         // deployment name (e.g., "api_server")
    pub module_name: Spanned<String>,   // imported module name (e.g., "api")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDecl {
    pub id: Uuid,
    pub name: Spanned<String>,          // system name (e.g., "OrderPlatform")
    pub members: Vec<SystemMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub id: Uuid,
    pub name: Spanned<String>,
    pub type_params: Vec<Spanned<String>>,
    pub type_param_bounds: HashMap<String, Vec<Spanned<String>>>,
    pub params: Vec<Param>,
    pub return_type: Option<Spanned<TypeExpr>>,
    pub contracts: Vec<Spanned<ContractClause>>,
    pub body: Spanned<Block>,
    pub is_pub: bool,
    pub is_override: bool,
    pub is_generator: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Param {
    pub id: Uuid,
    pub name: Spanned<String>,
    pub ty: Spanned<TypeExpr>,
    pub is_mut: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeExpr {
    Named(String),
    Array(Box<Spanned<TypeExpr>>),
    Qualified { module: String, name: String },
    Fn {
        params: Vec<Box<Spanned<TypeExpr>>>,
        return_type: Box<Spanned<TypeExpr>>,
    },
    Generic {
        name: String,
        type_args: Vec<Spanned<TypeExpr>>,
    },
    Nullable(Box<Spanned<TypeExpr>>),
    Stream(Box<Spanned<TypeExpr>>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub stmts: Vec<Spanned<Stmt>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Stmt {
    Let {
        name: Spanned<String>,
        ty: Option<Spanned<TypeExpr>>,
        value: Spanned<Expr>,
        is_mut: bool,
    },
    Return(Option<Spanned<Expr>>),
    Assign {
        target: Spanned<String>,
        value: Spanned<Expr>,
    },
    FieldAssign {
        object: Spanned<Expr>,
        field: Spanned<String>,
        value: Spanned<Expr>,
    },
    If {
        condition: Spanned<Expr>,
        then_block: Spanned<Block>,
        else_block: Option<Spanned<Block>>,
    },
    While {
        condition: Spanned<Expr>,
        body: Spanned<Block>,
    },
    For {
        var: Spanned<String>,
        iterable: Spanned<Expr>,
        body: Spanned<Block>,
    },
    IndexAssign {
        object: Spanned<Expr>,
        index: Spanned<Expr>,
        value: Spanned<Expr>,
    },
    Match {
        expr: Spanned<Expr>,
        arms: Vec<MatchArm>,
    },
    Raise {
        error_name: Spanned<String>,
        fields: Vec<(Spanned<String>, Spanned<Expr>)>,
        error_id: Option<Uuid>,
    },
    LetChan {
        sender: Spanned<String>,
        receiver: Spanned<String>,
        elem_type: Spanned<TypeExpr>,
        capacity: Option<Spanned<Expr>>,
    },
    Select {
        arms: Vec<SelectArm>,
        default: Option<Spanned<Block>>,
    },
    Scope {
        seeds: Vec<Spanned<Expr>>,
        bindings: Vec<ScopeBinding>,
        body: Spanned<Block>,
    },
    Yield {
        value: Spanned<Expr>,
    },
    Break,
    Continue,
    Expr(Spanned<Expr>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeBinding {
    pub name: Spanned<String>,
    pub ty: Spanned<TypeExpr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SelectOp {
    Recv { binding: Spanned<String>, channel: Spanned<Expr> },
    Send { channel: Spanned<Expr>, value: Spanned<Expr> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectArm {
    pub op: SelectOp,
    pub body: Spanned<Block>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expr {
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),
    StringLit(String),
    Ident(String),
    BinOp {
        op: BinOp,
        lhs: Box<Spanned<Expr>>,
        rhs: Box<Spanned<Expr>>,
    },
    UnaryOp {
        op: UnaryOp,
        operand: Box<Spanned<Expr>>,
    },
    Call {
        name: Spanned<String>,
        args: Vec<Spanned<Expr>>,
        type_args: Vec<Spanned<TypeExpr>>,
        target_id: Option<Uuid>,
    },
    FieldAccess {
        object: Box<Spanned<Expr>>,
        field: Spanned<String>,
    },
    MethodCall {
        object: Box<Spanned<Expr>>,
        method: Spanned<String>,
        args: Vec<Spanned<Expr>>,
    },
    StructLit {
        name: Spanned<String>,
        type_args: Vec<Spanned<TypeExpr>>,
        fields: Vec<(Spanned<String>, Spanned<Expr>)>,
        target_id: Option<Uuid>,
    },
    ArrayLit {
        elements: Vec<Spanned<Expr>>,
    },
    Index {
        object: Box<Spanned<Expr>>,
        index: Box<Spanned<Expr>>,
    },
    EnumUnit {
        enum_name: Spanned<String>,
        variant: Spanned<String>,
        type_args: Vec<Spanned<TypeExpr>>,
        enum_id: Option<Uuid>,
        variant_id: Option<Uuid>,
    },
    EnumData {
        enum_name: Spanned<String>,
        variant: Spanned<String>,
        type_args: Vec<Spanned<TypeExpr>>,
        fields: Vec<(Spanned<String>, Spanned<Expr>)>,
        enum_id: Option<Uuid>,
        variant_id: Option<Uuid>,
    },
    StringInterp {
        parts: Vec<StringInterpPart>,
    },
    Closure {
        params: Vec<Param>,
        return_type: Option<Spanned<TypeExpr>>,
        body: Spanned<Block>,
    },
    MapLit {
        key_type: Spanned<TypeExpr>,
        value_type: Spanned<TypeExpr>,
        entries: Vec<(Spanned<Expr>, Spanned<Expr>)>,
    },
    SetLit {
        elem_type: Spanned<TypeExpr>,
        elements: Vec<Spanned<Expr>>,
    },
    ClosureCreate {
        fn_name: String,
        captures: Vec<String>,
        target_id: Option<Uuid>,
    },
    Propagate {
        expr: Box<Spanned<Expr>>,
    },
    Catch {
        expr: Box<Spanned<Expr>>,
        handler: CatchHandler,
    },
    Cast {
        expr: Box<Spanned<Expr>>,
        target_type: Spanned<TypeExpr>,
    },
    Range {
        start: Box<Spanned<Expr>>,
        end: Box<Spanned<Expr>>,
        inclusive: bool,
    },
    Spawn {
        call: Box<Spanned<Expr>>,
    },
    NoneLit,
    NullPropagate {
        expr: Box<Spanned<Expr>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StringInterpPart {
    Lit(String),
    Expr(Spanned<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractClause {
    pub kind: ContractKind,
    pub expr: Spanned<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ContractKind {
    Requires,
    Ensures,
    Invariant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitDecl {
    pub id: Uuid,
    pub name: Spanned<String>,
    pub methods: Vec<TraitMethod>,
    pub is_pub: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitMethod {
    pub id: Uuid,
    pub name: Spanned<String>,
    pub params: Vec<Param>,
    pub return_type: Option<Spanned<TypeExpr>>,
    pub contracts: Vec<Spanned<ContractClause>>,
    pub body: Option<Spanned<Block>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumDecl {
    pub id: Uuid,
    pub name: Spanned<String>,
    pub type_params: Vec<Spanned<String>>,
    pub type_param_bounds: HashMap<String, Vec<Spanned<String>>>,
    pub variants: Vec<EnumVariant>,
    pub is_pub: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDecl {
    pub id: Uuid,
    pub name: Spanned<String>,
    pub fields: Vec<Field>,
    pub is_pub: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CatchHandler {
    Wildcard {
        var: Spanned<String>,
        body: Spanned<Block>,
    },
    Shorthand(Box<Spanned<Expr>>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumVariant {
    pub id: Uuid,
    pub name: Spanned<String>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchArm {
    pub enum_name: Spanned<String>,
    pub variant_name: Spanned<String>,
    pub type_args: Vec<Spanned<TypeExpr>>,
    pub bindings: Vec<(Spanned<String>, Option<Spanned<String>>)>,
    pub body: Spanned<Block>,
    pub enum_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
}
