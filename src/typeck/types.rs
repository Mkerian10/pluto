#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
        }
    }
}
