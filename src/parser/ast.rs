use crate::span::Spanned;

#[derive(Debug)]
pub struct Program {
    pub imports: Vec<Spanned<ImportDecl>>,
    pub functions: Vec<Spanned<Function>>,
    pub extern_fns: Vec<Spanned<ExternFnDecl>>,
    pub classes: Vec<Spanned<ClassDecl>>,
    pub traits: Vec<Spanned<TraitDecl>>,
    pub enums: Vec<Spanned<EnumDecl>>,
    pub app: Option<Spanned<AppDecl>>,
    pub errors: Vec<Spanned<ErrorDecl>>,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct ExternFnDecl {
    pub name: Spanned<String>,
    pub params: Vec<Param>,
    pub return_type: Option<Spanned<TypeExpr>>,
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub name: Spanned<String>,
    pub fields: Vec<Field>,
    pub methods: Vec<Spanned<Function>>,
    pub impl_traits: Vec<Spanned<String>>,
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: Spanned<String>,
    pub ty: Spanned<TypeExpr>,
    pub is_injected: bool,
}

#[derive(Debug, Clone)]
pub struct AppDecl {
    pub name: Spanned<String>,
    pub inject_fields: Vec<Field>,
    pub methods: Vec<Spanned<Function>>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: Spanned<String>,
    pub params: Vec<Param>,
    pub return_type: Option<Spanned<TypeExpr>>,
    pub body: Spanned<Block>,
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: Spanned<String>,
    pub ty: Spanned<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Named(String),
    Array(Box<Spanned<TypeExpr>>),
    Qualified { module: String, name: String },
    Fn {
        params: Vec<Box<Spanned<TypeExpr>>>,
        return_type: Box<Spanned<TypeExpr>>,
    },
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Spanned<Stmt>>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: Spanned<String>,
        ty: Option<Spanned<TypeExpr>>,
        value: Spanned<Expr>,
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
    },
    Expr(Spanned<Expr>),
}

#[derive(Debug, Clone)]
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
        fields: Vec<(Spanned<String>, Spanned<Expr>)>,
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
    },
    EnumData {
        enum_name: Spanned<String>,
        variant: Spanned<String>,
        fields: Vec<(Spanned<String>, Spanned<Expr>)>,
    },
    StringInterp {
        parts: Vec<StringInterpPart>,
    },
    Closure {
        params: Vec<Param>,
        return_type: Option<Spanned<TypeExpr>>,
        body: Spanned<Block>,
    },
    ClosureCreate {
        fn_name: String,
        captures: Vec<String>,
    },
    Propagate {
        expr: Box<Spanned<Expr>>,
    },
    Catch {
        expr: Box<Spanned<Expr>>,
        handler: CatchHandler,
    },
}

#[derive(Debug, Clone)]
pub enum StringInterpPart {
    Lit(String),
    Expr(Spanned<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone)]
pub struct TraitDecl {
    pub name: Spanned<String>,
    pub methods: Vec<TraitMethod>,
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub struct TraitMethod {
    pub name: Spanned<String>,
    pub params: Vec<Param>,
    pub return_type: Option<Spanned<TypeExpr>>,
    pub body: Option<Spanned<Block>>,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: Spanned<String>,
    pub variants: Vec<EnumVariant>,
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub struct ErrorDecl {
    pub name: Spanned<String>,
    pub fields: Vec<Field>,
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub enum CatchHandler {
    Wildcard {
        var: Spanned<String>,
        body: Box<Spanned<Expr>>,
    },
    Shorthand(Box<Spanned<Expr>>),
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: Spanned<String>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub enum_name: Spanned<String>,
    pub variant_name: Spanned<String>,
    pub bindings: Vec<(Spanned<String>, Option<Spanned<String>>)>,
    pub body: Spanned<Block>,
}
