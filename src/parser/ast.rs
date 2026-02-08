use crate::span::Spanned;

#[derive(Debug)]
pub struct Program {
    pub functions: Vec<Spanned<Function>>,
}

#[derive(Debug)]
pub struct Function {
    pub name: Spanned<String>,
    pub params: Vec<Param>,
    pub return_type: Option<Spanned<TypeExpr>>,
    pub body: Spanned<Block>,
}

#[derive(Debug)]
pub struct Param {
    pub name: Spanned<String>,
    pub ty: Spanned<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Named(String),
}

#[derive(Debug)]
pub struct Block {
    pub stmts: Vec<Spanned<Stmt>>,
}

#[derive(Debug)]
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
    If {
        condition: Spanned<Expr>,
        then_block: Spanned<Block>,
        else_block: Option<Spanned<Block>>,
    },
    While {
        condition: Spanned<Expr>,
        body: Spanned<Block>,
    },
    Expr(Spanned<Expr>),
}

#[derive(Debug)]
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
