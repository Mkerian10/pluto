use std::collections::{HashMap, HashSet};
use super::types::PlutoType;

#[derive(Debug, Clone)]
pub struct FuncSig {
    pub params: Vec<PlutoType>,
    pub return_type: PlutoType,
}

#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub fields: Vec<(String, PlutoType, bool)>,  // (name, type, is_injected)
    pub methods: Vec<String>,
    pub impl_traits: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TraitInfo {
    pub methods: Vec<(String, FuncSig)>,
    pub default_methods: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub variants: Vec<(String, Vec<(String, PlutoType)>)>,
}

#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub fields: Vec<(String, PlutoType)>,
}

#[derive(Debug, Clone)]
pub struct GenericFuncSig {
    pub type_params: Vec<String>,
    pub params: Vec<PlutoType>,      // contains TypeParam
    pub return_type: PlutoType,       // may contain TypeParam
}

#[derive(Debug, Clone)]
pub struct GenericClassInfo {
    pub type_params: Vec<String>,
    pub fields: Vec<(String, PlutoType, bool)>,  // may contain TypeParam
    pub methods: Vec<String>,
    pub method_sigs: HashMap<String, FuncSig>,  // method_name → sig (may contain TypeParam)
    pub impl_traits: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GenericEnumInfo {
    pub type_params: Vec<String>,
    pub variants: Vec<(String, Vec<(String, PlutoType)>)>,  // may contain TypeParam
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Instantiation {
    pub kind: InstKind,
    pub type_args: Vec<PlutoType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InstKind {
    Function(String),
    Class(String),
    Enum(String),
}

#[derive(Debug)]
pub struct TypeEnv {
    scopes: Vec<HashMap<String, PlutoType>>,
    pub functions: HashMap<String, FuncSig>,
    pub builtins: HashSet<String>,
    pub classes: HashMap<String, ClassInfo>,
    pub traits: HashMap<String, TraitInfo>,
    pub enums: HashMap<String, EnumInfo>,
    pub errors: HashMap<String, ErrorInfo>,
    pub extern_fns: HashSet<String>,
    /// Captures for each closure, keyed by (start, end) byte offset of the Expr::Closure node
    pub closure_captures: HashMap<(usize, usize), Vec<(String, PlutoType)>>,
    /// Lifted closure function name → captured variable names and types
    pub closure_fns: HashMap<String, Vec<(String, PlutoType)>>,
    pub app: Option<(String, ClassInfo)>,
    pub di_order: Vec<String>,
    /// Per-function error sets: maps function name to set of error type names it can raise.
    /// Populated by the error inference pass.
    pub fn_errors: HashMap<String, HashSet<String>>,
    // Generics
    pub generic_functions: HashMap<String, GenericFuncSig>,
    pub generic_classes: HashMap<String, GenericClassInfo>,
    pub generic_enums: HashMap<String, GenericEnumInfo>,
    pub instantiations: HashSet<Instantiation>,
    pub generic_rewrites: HashMap<(usize, usize), String>,
}

impl TypeEnv {
    pub fn new() -> Self {
        let mut builtins = HashSet::new();
        builtins.insert("print".to_string());
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            builtins,
            classes: HashMap::new(),
            traits: HashMap::new(),
            enums: HashMap::new(),
            errors: HashMap::new(),
            extern_fns: HashSet::new(),
            closure_captures: HashMap::new(),
            closure_fns: HashMap::new(),
            app: None,
            di_order: Vec::new(),
            fn_errors: HashMap::new(),
            generic_functions: HashMap::new(),
            generic_classes: HashMap::new(),
            generic_enums: HashMap::new(),
            instantiations: HashSet::new(),
            generic_rewrites: HashMap::new(),
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn define(&mut self, name: String, ty: PlutoType) {
        self.scopes.last_mut().unwrap().insert(name, ty);
    }

    pub fn lookup(&self, name: &str) -> Option<&PlutoType> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }

    pub fn scope_depth(&self) -> usize {
        self.scopes.len()
    }

    /// Look up a variable and return its type along with the scope depth it was found at (0-indexed from bottom)
    pub fn lookup_with_depth(&self, name: &str) -> Option<(&PlutoType, usize)> {
        for (i, scope) in self.scopes.iter().enumerate().rev() {
            if let Some(ty) = scope.get(name) {
                return Some((ty, i));
            }
        }
        None
    }

    pub fn class_implements_trait(&self, class_name: &str, trait_name: &str) -> bool {
        self.classes.get(class_name)
            .map(|c| c.impl_traits.iter().any(|t| t == trait_name))
            .unwrap_or(false)
    }

    pub fn is_fn_fallible(&self, name: &str) -> bool {
        self.fn_errors.get(name).map_or(false, |e| !e.is_empty())
    }
}

pub fn mangle_name(base: &str, type_args: &[PlutoType]) -> String {
    let suffixes: Vec<String> = type_args.iter().map(mangle_type).collect();
    format!("{}__{}", base, suffixes.join("_"))
}

fn mangle_type(ty: &PlutoType) -> String {
    match ty {
        PlutoType::Int => "int".into(),
        PlutoType::Float => "float".into(),
        PlutoType::Bool => "bool".into(),
        PlutoType::String => "string".into(),
        PlutoType::Void => "void".into(),
        PlutoType::Class(n) | PlutoType::Enum(n) => n.clone(),
        PlutoType::Array(inner) => format!("arr_{}", mangle_type(inner)),
        PlutoType::Fn(ps, r) => {
            let ps: Vec<_> = ps.iter().map(mangle_type).collect();
            format!("fn_{}_ret_{}", ps.join("_"), mangle_type(r))
        }
        PlutoType::Trait(n) => n.clone(),
        PlutoType::TypeParam(n) => n.clone(),
        PlutoType::Error => "error".into(),
    }
}
