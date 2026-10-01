use crate::parser::ast::TypeExpr;
use crate::span::Spanned;

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
    /// A function value type: (params, return, fallible, provides).
    /// `fallible` is the fallibility contract (`fn(int) int!`); `provides`
    /// is the sorted list of resolved property names the value's declaration
    /// provides (docs/design/rfc-properties.md phase 5) — a value flows into
    /// a fn type requiring properties only if its own set includes them.
    Fn(Vec<PlutoType>, Box<PlutoType>, bool, Vec<std::string::String>),
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

impl PlutoType {
    /// Recursively transform all inner types via `f`, rebuilding the structure.
    /// Leaf types (Int, Float, Bool, etc.) are returned unchanged.
    pub fn map_inner_types(&self, f: &impl Fn(&PlutoType) -> PlutoType) -> PlutoType {
        match self {
            PlutoType::Array(inner) => PlutoType::Array(Box::new(f(inner))),
            PlutoType::Fn(params, ret, fallible, provides) => PlutoType::Fn(
                params.iter().map(|p| f(p)).collect(),
                Box::new(f(ret)),
                *fallible,
                provides.clone(),
            ),
            PlutoType::Map(k, v) => PlutoType::Map(Box::new(f(k)), Box::new(f(v))),
            PlutoType::Set(t) => PlutoType::Set(Box::new(f(t))),
            PlutoType::Task(t) => PlutoType::Task(Box::new(f(t))),
            PlutoType::Sender(t) => PlutoType::Sender(Box::new(f(t))),
            PlutoType::Receiver(t) => PlutoType::Receiver(Box::new(f(t))),
            PlutoType::Nullable(inner) => PlutoType::Nullable(Box::new(f(inner))),
            PlutoType::GenericInstance(kind, name, args) => PlutoType::GenericInstance(
                kind.clone(),
                name.clone(),
                args.iter().map(|a| f(a)).collect(),
            ),
            // Leaf types — no inner types to transform
            _ => self.clone(),
        }
    }

    /// Returns true if any inner type (recursively) satisfies the predicate.
    /// Does NOT test `self` — only child types.
    pub fn any_inner_type(&self, pred: &impl Fn(&PlutoType) -> bool) -> bool {
        match self {
            PlutoType::Array(inner) => pred(inner),
            PlutoType::Fn(params, ret, _, _) => params.iter().any(|p| pred(p)) || pred(ret),
            PlutoType::Map(k, v) => pred(k) || pred(v),
            PlutoType::Set(t) | PlutoType::Task(t) | PlutoType::Sender(t)
            | PlutoType::Receiver(t) | PlutoType::Nullable(t) => pred(t),
            PlutoType::GenericInstance(_, _, args) => args.iter().any(|a| pred(a)),
            _ => false,
        }
    }
}

impl std::fmt::Display for PlutoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlutoType::Int => write!(f, "int"),
            PlutoType::Float => write!(f, "float"),
            PlutoType::Bool => write!(f, "bool"),
            PlutoType::String => write!(f, "string"),
            PlutoType::Void => write!(f, "void"),
            // '%' marks skolem classes standing in for type parameters during
            // generic template checking; show the parameter name users wrote.
            PlutoType::Class(name) => write!(f, "{}", name.replace('%', "")),
            PlutoType::Array(inner) => write!(f, "[{inner}]"),
            PlutoType::Trait(name) => write!(f, "trait {name}"),
            PlutoType::Enum(name) => write!(f, "{}", name.replace('%', "")),
            PlutoType::Fn(params, ret, fallible, provides) => {
                write!(f, "fn(")?;
                let fallible = *fallible;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", p)?;
                }
                write!(f, ") {}", ret)?;
                if fallible {
                    write!(f, "!")?;
                }
                for p in provides {
                    write!(f, " provides {p}")?;
                }
                Ok(())
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
            TypeExpr::Array(Box::new(Spanned::dummy(pluto_type_to_type_expr(inner))))
        }
        PlutoType::Trait(name) => TypeExpr::Named(name.clone()),
        PlutoType::Enum(name) => TypeExpr::Named(name.clone()),
        PlutoType::Fn(params, ret, fallible, provides) => TypeExpr::Fn {
            params: params
                .iter()
                .map(|p| Box::new(Spanned::dummy(pluto_type_to_type_expr(p))))
                .collect(),
            return_type: Box::new(Spanned::dummy(pluto_type_to_type_expr(ret))),
            fallible: *fallible,
            provides: provides.clone(),
        },
        PlutoType::Map(k, v) => TypeExpr::Generic {
            name: "Map".to_string(),
            type_args: vec![
                Spanned::dummy(pluto_type_to_type_expr(k)),
                Spanned::dummy(pluto_type_to_type_expr(v)),
            ],
        },
        PlutoType::Set(t) => TypeExpr::Generic {
            name: "Set".to_string(),
            type_args: vec![Spanned::dummy(pluto_type_to_type_expr(t))],
        },
        PlutoType::Task(t) => TypeExpr::Generic {
            name: "Task".to_string(),
            type_args: vec![Spanned::dummy(pluto_type_to_type_expr(t))],
        },
        PlutoType::Sender(t) => TypeExpr::Generic {
            name: "Sender".to_string(),
            type_args: vec![Spanned::dummy(pluto_type_to_type_expr(t))],
        },
        PlutoType::Receiver(t) => TypeExpr::Generic {
            name: "Receiver".to_string(),
            type_args: vec![Spanned::dummy(pluto_type_to_type_expr(t))],
        },
        PlutoType::Error => TypeExpr::Named("error".to_string()),
        PlutoType::TypeParam(name) => TypeExpr::Named(name.clone()),
        PlutoType::Range => TypeExpr::Named("range".to_string()),
        PlutoType::Byte => TypeExpr::Named("byte".to_string()),
        PlutoType::Bytes => TypeExpr::Named("bytes".to_string()),
        PlutoType::GenericInstance(_, name, args) => TypeExpr::Generic {
            name: name.clone(),
            type_args: args.iter()
                .map(|a| Spanned::dummy(pluto_type_to_type_expr(a)))
                .collect(),
        },
        PlutoType::Nullable(inner) => {
            TypeExpr::Nullable(Box::new(Spanned::dummy(pluto_type_to_type_expr(inner))))
        }
        PlutoType::Stream(inner) => {
            TypeExpr::Stream(Box::new(Spanned::dummy(pluto_type_to_type_expr(inner))))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== map_inner_types tests =====

    #[test]
    fn test_map_inner_types_array() {
        let ty = PlutoType::Array(Box::new(PlutoType::Int));
        let result = ty.map_inner_types(&|t| {
            if matches!(t, PlutoType::Int) {
                PlutoType::Float
            } else {
                t.clone()
            }
        });
        assert_eq!(result, PlutoType::Array(Box::new(PlutoType::Float)));
    }

    #[test]
    fn test_map_inner_types_fn() {
        let ty = PlutoType::Fn(
            vec![PlutoType::Int, PlutoType::Bool],
            Box::new(PlutoType::String),
            false, vec![]);
        let result = ty.map_inner_types(&|t| {
            if matches!(t, PlutoType::Int) {
                PlutoType::Float
            } else {
                t.clone()
            }
        });
        assert_eq!(
            result,
            PlutoType::Fn(
                vec![PlutoType::Float, PlutoType::Bool],
                Box::new(PlutoType::String)
            , false, vec![])
        );
    }

    #[test]
    fn test_map_inner_types_map() {
        let ty = PlutoType::Map(Box::new(PlutoType::Int), Box::new(PlutoType::String));
        let result = ty.map_inner_types(&|t| {
            if matches!(t, PlutoType::Int) {
                PlutoType::Float
            } else {
                t.clone()
            }
        });
        assert_eq!(
            result,
            PlutoType::Map(Box::new(PlutoType::Float), Box::new(PlutoType::String))
        );
    }

    #[test]
    fn test_map_inner_types_set() {
        let ty = PlutoType::Set(Box::new(PlutoType::Int));
        let result = ty.map_inner_types(&|t| {
            if matches!(t, PlutoType::Int) {
                PlutoType::Float
            } else {
                t.clone()
            }
        });
        assert_eq!(result, PlutoType::Set(Box::new(PlutoType::Float)));
    }

    #[test]
    fn test_map_inner_types_task() {
        let ty = PlutoType::Task(Box::new(PlutoType::Int));
        let result = ty.map_inner_types(&|t| {
            if matches!(t, PlutoType::Int) {
                PlutoType::String
            } else {
                t.clone()
            }
        });
        assert_eq!(result, PlutoType::Task(Box::new(PlutoType::String)));
    }

    #[test]
    fn test_map_inner_types_nullable() {
        let ty = PlutoType::Nullable(Box::new(PlutoType::Int));
        let result = ty.map_inner_types(&|t| {
            if matches!(t, PlutoType::Int) {
                PlutoType::Bool
            } else {
                t.clone()
            }
        });
        assert_eq!(result, PlutoType::Nullable(Box::new(PlutoType::Bool)));
    }

    #[test]
    fn test_map_inner_types_generic_instance() {
        let ty = PlutoType::GenericInstance(
            GenericKind::Class,
            "Pair".to_string(),
            vec![PlutoType::Int, PlutoType::String],
        );
        let result = ty.map_inner_types(&|t| {
            if matches!(t, PlutoType::Int) {
                PlutoType::Float
            } else {
                t.clone()
            }
        });
        assert_eq!(
            result,
            PlutoType::GenericInstance(
                GenericKind::Class,
                "Pair".to_string(),
                vec![PlutoType::Float, PlutoType::String]
            )
        );
    }

    #[test]
    fn test_map_inner_types_leaf() {
        let leaf_types = vec![
            PlutoType::Int,
            PlutoType::Float,
            PlutoType::Bool,
            PlutoType::String,
            PlutoType::Void,
            PlutoType::Byte,
            PlutoType::Bytes,
            PlutoType::Range,
            PlutoType::Error,
        ];
        for ty in leaf_types {
            let result = ty.map_inner_types(&|_| PlutoType::Float);
            assert_eq!(result, ty); // Leaf types unchanged
        }
    }

    // ===== any_inner_type tests =====

    #[test]
    fn test_any_inner_type_array() {
        let ty = PlutoType::Array(Box::new(PlutoType::Int));
        assert!(ty.any_inner_type(&|t| matches!(t, PlutoType::Int)));
        assert!(!ty.any_inner_type(&|t| matches!(t, PlutoType::Float)));
    }

    #[test]
    fn test_any_inner_type_fn_params() {
        let ty = PlutoType::Fn(
            vec![PlutoType::Int, PlutoType::Bool],
            Box::new(PlutoType::String),
            false, vec![]);
        assert!(ty.any_inner_type(&|t| matches!(t, PlutoType::Int)));
        assert!(ty.any_inner_type(&|t| matches!(t, PlutoType::Bool)));
    }

    #[test]
    fn test_any_inner_type_fn_return() {
        let ty = PlutoType::Fn(
            vec![PlutoType::Int],
            Box::new(PlutoType::String),
            false, vec![]);
        assert!(ty.any_inner_type(&|t| matches!(t, PlutoType::String)));
    }

    #[test]
    fn test_any_inner_type_map() {
        let ty = PlutoType::Map(Box::new(PlutoType::Int), Box::new(PlutoType::String));
        assert!(ty.any_inner_type(&|t| matches!(t, PlutoType::Int)));
        assert!(ty.any_inner_type(&|t| matches!(t, PlutoType::String)));
        assert!(!ty.any_inner_type(&|t| matches!(t, PlutoType::Float)));
    }

    #[test]
    fn test_any_inner_type_generic_instance() {
        let ty = PlutoType::GenericInstance(
            GenericKind::Class,
            "Pair".to_string(),
            vec![PlutoType::Int, PlutoType::Bool],
        );
        assert!(ty.any_inner_type(&|t| matches!(t, PlutoType::Int)));
        assert!(ty.any_inner_type(&|t| matches!(t, PlutoType::Bool)));
        assert!(!ty.any_inner_type(&|t| matches!(t, PlutoType::String)));
    }

    #[test]
    fn test_any_inner_type_sender_receiver() {
        let sender = PlutoType::Sender(Box::new(PlutoType::Int));
        let receiver = PlutoType::Receiver(Box::new(PlutoType::Int));
        assert!(sender.any_inner_type(&|t| matches!(t, PlutoType::Int)));
        assert!(receiver.any_inner_type(&|t| matches!(t, PlutoType::Int)));
    }

    #[test]
    fn test_any_inner_type_leaf_false() {
        let ty = PlutoType::Int;
        assert!(!ty.any_inner_type(&|_| true)); // Leaf has no inner types
    }

    // ===== Display tests =====

    #[test]
    fn test_display_primitives() {
        assert_eq!(PlutoType::Int.to_string(), "int");
        assert_eq!(PlutoType::Float.to_string(), "float");
        assert_eq!(PlutoType::Bool.to_string(), "bool");
        assert_eq!(PlutoType::String.to_string(), "string");
        assert_eq!(PlutoType::Void.to_string(), "void");
        assert_eq!(PlutoType::Byte.to_string(), "byte");
        assert_eq!(PlutoType::Bytes.to_string(), "bytes");
        assert_eq!(PlutoType::Range.to_string(), "range");
        assert_eq!(PlutoType::Error.to_string(), "error");
    }

    #[test]
    fn test_display_class() {
        let ty = PlutoType::Class("User".to_string());
        assert_eq!(ty.to_string(), "User");
    }

    #[test]
    fn test_display_array() {
        let ty = PlutoType::Array(Box::new(PlutoType::Int));
        assert_eq!(ty.to_string(), "[int]");
    }

    #[test]
    fn test_display_trait() {
        let ty = PlutoType::Trait("Printable".to_string());
        assert_eq!(ty.to_string(), "trait Printable");
    }

    #[test]
    fn test_display_enum() {
        let ty = PlutoType::Enum("Option".to_string());
        assert_eq!(ty.to_string(), "Option");
    }

    #[test]
    fn test_display_fn_no_params() {
        let ty = PlutoType::Fn(vec![], Box::new(PlutoType::Int), false, vec![]);
        assert_eq!(ty.to_string(), "fn() int");
    }

    #[test]
    fn test_display_fn_with_params() {
        let ty = PlutoType::Fn(
            vec![PlutoType::Int, PlutoType::String],
            Box::new(PlutoType::Bool),
            false, vec![]);
        assert_eq!(ty.to_string(), "fn(int, string) bool");
    }

    #[test]
    fn test_display_map() {
        let ty = PlutoType::Map(Box::new(PlutoType::String), Box::new(PlutoType::Int));
        assert_eq!(ty.to_string(), "Map<string, int>");
    }

    #[test]
    fn test_display_set() {
        let ty = PlutoType::Set(Box::new(PlutoType::Int));
        assert_eq!(ty.to_string(), "Set<int>");
    }

    #[test]
    fn test_display_task() {
        let ty = PlutoType::Task(Box::new(PlutoType::String));
        assert_eq!(ty.to_string(), "Task<string>");
    }

    #[test]
    fn test_display_nullable() {
        let ty = PlutoType::Nullable(Box::new(PlutoType::Int));
        assert_eq!(ty.to_string(), "int?");
    }

    #[test]
    fn test_display_generic_instance() {
        let ty = PlutoType::GenericInstance(
            GenericKind::Class,
            "Pair".to_string(),
            vec![PlutoType::Int, PlutoType::String],
        );
        assert_eq!(ty.to_string(), "Pair<int, string>");
    }

    #[test]
    fn test_display_sender() {
        let ty = PlutoType::Sender(Box::new(PlutoType::Int));
        assert_eq!(ty.to_string(), "Sender<int>");
    }

    #[test]
    fn test_display_receiver() {
        let ty = PlutoType::Receiver(Box::new(PlutoType::Int));
        assert_eq!(ty.to_string(), "Receiver<int>");
    }

    #[test]
    fn test_display_stream() {
        let ty = PlutoType::Stream(Box::new(PlutoType::Int));
        assert_eq!(ty.to_string(), "stream int");
    }

    #[test]
    fn test_display_type_param() {
        let ty = PlutoType::TypeParam("T".to_string());
        assert_eq!(ty.to_string(), "T");
    }

    // ===== pluto_type_to_type_expr tests =====

    #[test]
    fn test_type_expr_primitives() {
        let int_expr = pluto_type_to_type_expr(&PlutoType::Int);
        assert!(matches!(int_expr, TypeExpr::Named(s) if s == "int"));

        let float_expr = pluto_type_to_type_expr(&PlutoType::Float);
        assert!(matches!(float_expr, TypeExpr::Named(s) if s == "float"));

        let bool_expr = pluto_type_to_type_expr(&PlutoType::Bool);
        assert!(matches!(bool_expr, TypeExpr::Named(s) if s == "bool"));

        let void_expr = pluto_type_to_type_expr(&PlutoType::Void);
        assert!(matches!(void_expr, TypeExpr::Named(s) if s == "void"));
    }

    #[test]
    fn test_type_expr_array() {
        let ty = PlutoType::Array(Box::new(PlutoType::Int));
        let expr = pluto_type_to_type_expr(&ty);
        match expr {
            TypeExpr::Array(inner) => {
                assert!(matches!(inner.node, TypeExpr::Named(s) if s == "int"));
            }
            _ => panic!("Expected TypeExpr::Array"),
        }
    }

    #[test]
    fn test_type_expr_fn() {
        let ty = PlutoType::Fn(
            vec![PlutoType::Int, PlutoType::Bool],
            Box::new(PlutoType::String),
            false, vec![]);
        let expr = pluto_type_to_type_expr(&ty);
        match expr {
            TypeExpr::Fn { params, return_type, fallible: _, provides: _ } => {
                assert_eq!(params.len(), 2);
                assert!(matches!(return_type.node, TypeExpr::Named(s) if s == "string"));
            }
            _ => panic!("Expected TypeExpr::Fn"),
        }
    }

    #[test]
    fn test_type_expr_map() {
        let ty = PlutoType::Map(Box::new(PlutoType::String), Box::new(PlutoType::Int));
        let expr = pluto_type_to_type_expr(&ty);
        match expr {
            TypeExpr::Generic { name, type_args } => {
                assert_eq!(name, "Map");
                assert_eq!(type_args.len(), 2);
            }
            _ => panic!("Expected TypeExpr::Generic"),
        }
    }

    #[test]
    fn test_type_expr_set() {
        let ty = PlutoType::Set(Box::new(PlutoType::Int));
        let expr = pluto_type_to_type_expr(&ty);
        match expr {
            TypeExpr::Generic { name, type_args } => {
                assert_eq!(name, "Set");
                assert_eq!(type_args.len(), 1);
            }
            _ => panic!("Expected TypeExpr::Generic"),
        }
    }

    #[test]
    fn test_type_expr_task() {
        let ty = PlutoType::Task(Box::new(PlutoType::String));
        let expr = pluto_type_to_type_expr(&ty);
        match expr {
            TypeExpr::Generic { name, type_args } => {
                assert_eq!(name, "Task");
                assert_eq!(type_args.len(), 1);
            }
            _ => panic!("Expected TypeExpr::Generic"),
        }
    }

    #[test]
    fn test_type_expr_nullable() {
        let ty = PlutoType::Nullable(Box::new(PlutoType::Int));
        let expr = pluto_type_to_type_expr(&ty);
        match expr {
            TypeExpr::Nullable(inner) => {
                assert!(matches!(inner.node, TypeExpr::Named(s) if s == "int"));
            }
            _ => panic!("Expected TypeExpr::Nullable"),
        }
    }

    #[test]
    fn test_type_expr_generic_instance() {
        let ty = PlutoType::GenericInstance(
            GenericKind::Class,
            "Pair".to_string(),
            vec![PlutoType::Int, PlutoType::String],
        );
        let expr = pluto_type_to_type_expr(&ty);
        match expr {
            TypeExpr::Generic { name, type_args } => {
                assert_eq!(name, "Pair");
                assert_eq!(type_args.len(), 2);
            }
            _ => panic!("Expected TypeExpr::Generic"),
        }
    }

    #[test]
    fn test_type_expr_stream() {
        let ty = PlutoType::Stream(Box::new(PlutoType::Int));
        let expr = pluto_type_to_type_expr(&ty);
        match expr {
            TypeExpr::Stream(inner) => {
                assert!(matches!(inner.node, TypeExpr::Named(s) if s == "int"));
            }
            _ => panic!("Expected TypeExpr::Stream"),
        }
    }

    // ===== Additional tests =====

    #[test]
    fn test_equality() {
        assert_eq!(PlutoType::Int, PlutoType::Int);
        assert_ne!(PlutoType::Int, PlutoType::Float);
        assert_eq!(
            PlutoType::Array(Box::new(PlutoType::Int)),
            PlutoType::Array(Box::new(PlutoType::Int))
        );
        assert_ne!(
            PlutoType::Array(Box::new(PlutoType::Int)),
            PlutoType::Array(Box::new(PlutoType::Float))
        );
    }

    #[test]
    fn test_clone() {
        let ty = PlutoType::Fn(
            vec![PlutoType::Int, PlutoType::String],
            Box::new(PlutoType::Bool),
            false, vec![]);
        let cloned = ty.clone();
        assert_eq!(ty, cloned);
    }

    #[test]
    fn test_generic_kind_equality() {
        assert_eq!(GenericKind::Class, GenericKind::Class);
        assert_eq!(GenericKind::Enum, GenericKind::Enum);
        assert_ne!(GenericKind::Class, GenericKind::Enum);
    }

    #[test]
    fn test_nested_transformations() {
        let ty = PlutoType::Array(Box::new(PlutoType::Nullable(Box::new(PlutoType::Int))));
        let result = ty.map_inner_types(&|t| match t {
            PlutoType::Nullable(inner) => PlutoType::Nullable(Box::new(
                if matches!(**inner, PlutoType::Int) {
                    PlutoType::Float
                } else {
                    (**inner).clone()
                }
            )),
            other => other.clone(),
        });
        assert_eq!(
            result,
            PlutoType::Array(Box::new(PlutoType::Nullable(Box::new(PlutoType::Float))))
        );
    }
}

/// Whether a type can cross a domain boundary (the schema-level wire surface).
/// The single predicate shared by `at` placement checking, the serve dispatch
/// filter, the interface hash, and both marshal directions — per
/// docs/design/distributed-model.md, this is checked identically whatever
/// physical plan the boundary gets.
pub(crate) fn wire_supported(t: &PlutoType) -> bool {
    match t {
        PlutoType::Int
        | PlutoType::Float
        | PlutoType::Bool
        | PlutoType::String
        | PlutoType::Class(_)
        | PlutoType::Enum(_) => true,
        PlutoType::Nullable(inner) => wire_supported(inner),
        PlutoType::Array(elem) => wire_supported(elem),
        PlutoType::Set(elem) => wire_supported(elem),
        PlutoType::Map(k, v) => wire_supported(k) && wire_supported(v),
        _ => false,
    }
}

/// Deep transferability for `at` placement boundaries: `wire_supported` plus
/// a transitive walk of class fields and enum variant fields, so a value
/// whose SHAPE cannot marshal (a closure field, a trait field) is rejected
/// by typeck's boundary diagnostic instead of surfacing as a missing
/// generated function during codegen. Deliberately NOT used by the interface
/// hash or the serve dispatch filter — their method set (and thus the hash,
/// PR #335) must stay stable.
pub(crate) fn wire_transferable(t: &PlutoType, env: &crate::typeck::env::TypeEnv) -> bool {
    fn shape_ok(
        t: &PlutoType,
        env: &crate::typeck::env::TypeEnv,
        visiting: &mut std::collections::HashSet<String>,
    ) -> bool {
        match t {
            PlutoType::Int
            | PlutoType::Float
            | PlutoType::Bool
            | PlutoType::String
            // Marshalable in FIELD position (encoded as int); top-level args
            // are still gated by wire_supported.
            | PlutoType::Byte => true,
            PlutoType::Nullable(inner)
            | PlutoType::Array(inner)
            | PlutoType::Set(inner) => shape_ok(inner, env, visiting),
            PlutoType::Map(k, v) => shape_ok(k, env, visiting) && shape_ok(v, env, visiting),
            PlutoType::Class(n) => {
                if !visiting.insert(n.clone()) {
                    return true; // recursive type — already being examined
                }
                // Unknown class: no field info to falsify — leave it to the
                // existing (shallow) acceptance rather than reject blind.
                let ok = env.classes.get(n).is_none_or(|info| {
                    info.fields
                        .iter()
                        .all(|(_, ft, is_injected)| *is_injected || shape_ok(ft, env, visiting))
                });
                visiting.remove(n);
                ok
            }
            PlutoType::Enum(n) => {
                if !visiting.insert(n.clone()) {
                    return true;
                }
                let ok = env.enums.get(n).is_none_or(|info| {
                    info.variants
                        .iter()
                        .all(|(_, fields)| fields.iter().all(|(_, ft)| shape_ok(ft, env, visiting)))
                });
                visiting.remove(n);
                ok
            }
            _ => false,
        }
    }
    wire_supported(t) && shape_ok(t, env, &mut std::collections::HashSet::new())
}

/// Whether a type contains entity (object) types anywhere in its shape —
/// directly, in containers, or TRANSITIVELY through class/enum fields.
/// Top-level entities cross boundaries as identity handles; entities nested
/// inside values are untransferable (a copy would fork their identity).
pub(crate) fn contains_object_type(
    t: &PlutoType,
    env: &crate::typeck::env::TypeEnv,
) -> bool {
    fn walk(
        t: &PlutoType,
        env: &crate::typeck::env::TypeEnv,
        visiting: &mut std::collections::HashSet<String>,
    ) -> bool {
        match t {
            PlutoType::Class(n) => {
                if env.object_types.contains(n) {
                    return true;
                }
                if !visiting.insert(n.clone()) {
                    return false; // recursive type — already being examined
                }
                let hit = env
                    .classes
                    .get(n)
                    .is_some_and(|info| info.fields.iter().any(|(_, ft, _)| walk(ft, env, visiting)));
                visiting.remove(n);
                hit
            }
            PlutoType::Enum(n) => {
                if env.object_types.contains(n) {
                    return true;
                }
                if !visiting.insert(n.clone()) {
                    return false;
                }
                let hit = env.enums.get(n).is_some_and(|info| {
                    info.variants
                        .iter()
                        .any(|(_, fields)| fields.iter().any(|(_, ft)| walk(ft, env, visiting)))
                });
                visiting.remove(n);
                hit
            }
            PlutoType::Nullable(inner)
            | PlutoType::Array(inner)
            | PlutoType::Set(inner) => walk(inner, env, visiting),
            PlutoType::Map(k, v) => walk(k, env, visiting) || walk(v, env, visiting),
            // Symbolic generic reference (pre-resolution): a generic OBJECT
            // base name marks every instantiation as an entity; also look
            // through the args.
            PlutoType::GenericInstance(_, name, args) => {
                env.object_types.contains(name)
                    || args.iter().any(|a| walk(a, env, visiting))
            }
            _ => false,
        }
    }
    let mut visiting = std::collections::HashSet::new();
    walk(t, env, &mut visiting)
}
