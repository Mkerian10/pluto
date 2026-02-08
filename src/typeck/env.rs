use std::collections::HashMap;
use super::types::PlutoType;

#[derive(Debug, Clone)]
pub struct FuncSig {
    pub params: Vec<PlutoType>,
    pub return_type: PlutoType,
}

pub struct TypeEnv {
    scopes: Vec<HashMap<String, PlutoType>>,
    pub functions: HashMap<String, FuncSig>,
}

impl TypeEnv {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
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
}
