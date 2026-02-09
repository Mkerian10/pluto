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

#[derive(Debug)]
pub struct TypeEnv {
    scopes: Vec<HashMap<String, PlutoType>>,
    pub functions: HashMap<String, FuncSig>,
    pub builtins: HashSet<String>,
    pub classes: HashMap<String, ClassInfo>,
    pub traits: HashMap<String, TraitInfo>,
    pub enums: HashMap<String, EnumInfo>,
    pub extern_fns: HashSet<String>,
    /// Captures for each closure, keyed by (start, end) byte offset of the Expr::Closure node
    pub closure_captures: HashMap<(usize, usize), Vec<(String, PlutoType)>>,
    /// Lifted closure function name → captured variable names and types
    pub closure_fns: HashMap<String, Vec<(String, PlutoType)>>,
    pub app: Option<(String, ClassInfo)>,
    pub di_order: Vec<String>,
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
            extern_fns: HashSet::new(),
            closure_captures: HashMap::new(),
            closure_fns: HashMap::new(),
            app: None,
            di_order: Vec::new(),
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
}
