use crate::parser::ast::TypeExpr;
use crate::span::{Span, Spanned};

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PlutoType {
    Int,
    Float,
    Bool,
    String,
    Void,
    Class(std::string::String),
    Array(Box<PlutoType>),
    Trait(std::string::String),
    Enum(std::string::String),
    Fn(Vec<PlutoType>, Box<PlutoType>),
    Map(Box<PlutoType>, Box<PlutoType>),
    Set(Box<PlutoType>),
    Range,
    Error,
    TypeParam(std::string::String),
    Task(Box<PlutoType>),
    Byte,
    Bytes,
    Sender(Box<PlutoType>),
    Receiver(Box<PlutoType>),
    /// A user-defined generic type with unresolved type parameters.
    /// Stored as (kind, base_name, type_args) — e.g., GenericInstance(Class, "Pair", [TypeParam("A"), TypeParam("B")]).
    /// Used during generic function signature registration when the type args include TypeParams.
    /// substitute_pluto_type resolves these to concrete Class/Enum types when all args become concrete.
    GenericInstance(GenericKind, std::string::String, Vec<PlutoType>),
    Nullable(Box<PlutoType>),
    Stream(Box<PlutoType>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GenericKind {
    Class,
    Enum,
}

impl std::fmt::Display for PlutoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlutoType::Int => write!(f, "int"),
            PlutoType::Float => write!(f, "float"),
            PlutoType::Bool => write!(f, "bool"),
            PlutoType::String => write!(f, "string"),
            PlutoType::Void => write!(f, "void"),
            PlutoType::Class(name) => write!(f, "{name}"),
            PlutoType::Array(inner) => write!(f, "[{inner}]"),
            PlutoType::Trait(name) => write!(f, "trait {name}"),
            PlutoType::Enum(name) => write!(f, "{name}"),
            PlutoType::Fn(params, ret) => {
                write!(f, "fn(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", p)?;
                }
                write!(f, ") {}", ret)
            }
            PlutoType::Map(k, v) => write!(f, "Map<{k}, {v}>"),
            PlutoType::Set(t) => write!(f, "Set<{t}>"),
            PlutoType::Range => write!(f, "range"),
            PlutoType::Error => write!(f, "error"),
            PlutoType::TypeParam(name) => write!(f, "{name}"),
            PlutoType::Task(inner) => write!(f, "Task<{inner}>"),
            PlutoType::Byte => write!(f, "byte"),
            PlutoType::Bytes => write!(f, "bytes"),
            PlutoType::Sender(inner) => write!(f, "Sender<{inner}>"),
            PlutoType::Receiver(inner) => write!(f, "Receiver<{inner}>"),
            PlutoType::Nullable(inner) => write!(f, "{inner}?"),
            PlutoType::Stream(inner) => write!(f, "stream {inner}"),
            PlutoType::GenericInstance(_, name, args) => {
                write!(f, "{name}<")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{a}")?;
                }
                write!(f, ">")
            }
        }
    }
}

/// Convert a PlutoType back to a TypeExpr (AST representation).
/// Used by closure lifting and monomorphization to build type annotations.
pub fn pluto_type_to_type_expr(ty: &PlutoType) -> TypeExpr {
    match ty {
        PlutoType::Int => TypeExpr::Named("int".to_string()),
        PlutoType::Float => TypeExpr::Named("float".to_string()),
        PlutoType::Bool => TypeExpr::Named("bool".to_string()),
        PlutoType::String => TypeExpr::Named("string".to_string()),
        PlutoType::Void => TypeExpr::Named("void".to_string()),
        PlutoType::Class(name) => TypeExpr::Named(name.clone()),
        PlutoType::Array(inner) => {
            TypeExpr::Array(Box::new(Spanned::new(pluto_type_to_type_expr(inner), Span::new(0, 0))))
        }
        PlutoType::Trait(name) => TypeExpr::Named(name.clone()),
        PlutoType::Enum(name) => TypeExpr::Named(name.clone()),
        PlutoType::Fn(params, ret) => TypeExpr::Fn {
            params: params
                .iter()
                .map(|p| Box::new(Spanned::new(pluto_type_to_type_expr(p), Span::new(0, 0))))
                .collect(),
            return_type: Box::new(Spanned::new(pluto_type_to_type_expr(ret), Span::new(0, 0))),
        },
        PlutoType::Map(k, v) => TypeExpr::Generic {
            name: "Map".to_string(),
            type_args: vec![
                Spanned::new(pluto_type_to_type_expr(k), Span::new(0, 0)),
                Spanned::new(pluto_type_to_type_expr(v), Span::new(0, 0)),
            ],
        },
        PlutoType::Set(t) => TypeExpr::Generic {
            name: "Set".to_string(),
            type_args: vec![Spanned::new(pluto_type_to_type_expr(t), Span::new(0, 0))],
        },
        PlutoType::Task(t) => TypeExpr::Generic {
            name: "Task".to_string(),
            type_args: vec![Spanned::new(pluto_type_to_type_expr(t), Span::new(0, 0))],
        },
        PlutoType::Sender(t) => TypeExpr::Generic {
            name: "Sender".to_string(),
            type_args: vec![Spanned::new(pluto_type_to_type_expr(t), Span::new(0, 0))],
        },
        PlutoType::Receiver(t) => TypeExpr::Generic {
            name: "Receiver".to_string(),
            type_args: vec![Spanned::new(pluto_type_to_type_expr(t), Span::new(0, 0))],
        },
        PlutoType::Error => TypeExpr::Named("error".to_string()),
        PlutoType::TypeParam(name) => TypeExpr::Named(name.clone()),
        PlutoType::Range => TypeExpr::Named("range".to_string()),
        PlutoType::Byte => TypeExpr::Named("byte".to_string()),
        PlutoType::Bytes => TypeExpr::Named("bytes".to_string()),
        PlutoType::GenericInstance(_, name, args) => TypeExpr::Generic {
            name: name.clone(),
            type_args: args.iter()
                .map(|a| Spanned::new(pluto_type_to_type_expr(a), Span::new(0, 0)))
                .collect(),
        },
        PlutoType::Nullable(inner) => {
            TypeExpr::Nullable(Box::new(Spanned::new(pluto_type_to_type_expr(inner), Span::new(0, 0))))
        }
        PlutoType::Stream(inner) => {
            TypeExpr::Stream(Box::new(Spanned::new(pluto_type_to_type_expr(inner), Span::new(0, 0))))
        }
    }
}
