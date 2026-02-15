use std::collections::{HashMap, HashSet};
use super::types::PlutoType;
use crate::parser::ast::{ContractClause, Lifecycle, TypeExpr};
use crate::span::{Span, Spanned};
use crate::visit::scope_tracker::ScopeTracker;

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
    pub lifecycle: Lifecycle,
}

#[derive(Debug, Clone)]
pub struct TraitInfo {
    pub methods: Vec<(String, FuncSig)>,
    pub default_methods: Vec<String>,
    pub mut_self_methods: HashSet<String>,
    pub static_methods: HashSet<String>,  // Methods without self parameter
    pub method_contracts: HashMap<String, Vec<Spanned<ContractClause>>>,
    /// Temporary storage for raw AST type expressions during registration
    /// Maps method_name -> (param_types, return_type)
    pub method_type_exprs: HashMap<String, (Vec<Spanned<TypeExpr>>, Option<Spanned<TypeExpr>>)>,
}

#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub variants: Vec<(String, Vec<(String, PlutoType)>)>,
    /// Temporary storage for raw AST type expressions during registration
    /// Vec of (variant_name, Vec of (field_name, field_type))
    pub variant_type_exprs: Vec<(String, Vec<(String, Spanned<TypeExpr>)>)>,
}

#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub fields: Vec<(String, PlutoType)>,
}

#[derive(Debug, Clone)]
pub struct GenericFuncSig {
    pub type_params: Vec<String>,
    pub type_param_bounds: HashMap<String, Vec<String>>,  // T -> [Trait1, Trait2]
    pub params: Vec<PlutoType>,      // contains TypeParam
    pub return_type: PlutoType,       // may contain TypeParam
}

#[derive(Debug, Clone)]
pub struct GenericClassInfo {
    pub type_params: Vec<String>,
    pub type_param_bounds: HashMap<String, Vec<String>>,  // T -> [Trait1, Trait2]
    pub fields: Vec<(String, PlutoType, bool)>,  // may contain TypeParam
    pub methods: Vec<String>,
    pub method_sigs: HashMap<String, FuncSig>,  // method_name → sig (may contain TypeParam)
    pub impl_traits: Vec<String>,
    pub mut_self_methods: HashSet<String>,
    pub lifecycle: Lifecycle,
}

#[derive(Debug, Clone)]
pub struct GenericEnumInfo {
    pub type_params: Vec<String>,
    pub type_param_bounds: HashMap<String, Vec<String>>,  // T -> [Trait1, Trait2]
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

#[derive(Debug, Clone)]
pub enum MethodResolution {
    /// Class method call — resolved to a specific mangled name
    Class { mangled_name: String },
    /// Trait dynamic dispatch — can't know concrete class at compile time
    TraitDynamic { trait_name: String, method_name: String },
    /// Built-in method (array.push, array.len, string.len) — always infallible
    Builtin,
    /// Task.get() — spawned_fn tracks origin for error propagation
    TaskGet { spawned_fn: Option<String> },
    /// Channel send — fallible (ChannelClosed)
    ChannelSend,
    /// Channel recv — fallible (ChannelClosed)
    ChannelRecv,
    /// Channel try_send — fallible (ChannelClosed + ChannelFull)
    ChannelTrySend,
    /// Channel try_recv — fallible (ChannelClosed + ChannelEmpty)
    ChannelTryRecv,
    /// Task.detach() — infallible
    TaskDetach,
    /// Task.cancel() — infallible
    TaskCancel,
}

/// How a field of a scoped class gets its value during a scope block.
#[derive(Debug, Clone)]
pub enum FieldWiring {
    /// Value comes from the Nth seed expression
    Seed(usize),
    /// Value comes from a singleton global (class name)
    Singleton(String),
    /// Value comes from another scoped instance created within this scope block (class name)
    ScopedInstance(String),
}

/// Resolved DI graph for a single scope block — computed in typeck, consumed in codegen.
#[derive(Debug, Clone)]
pub struct ScopeResolution {
    /// Topologically sorted scoped classes to allocate (leaves first)
    pub creation_order: Vec<String>,
    /// Per-class field wirings: class_name → [(field_name, wiring_source)]
    pub field_wirings: HashMap<String, Vec<(String, FieldWiring)>>,
    /// How each binding variable is satisfied
    pub binding_sources: Vec<FieldWiring>,
}

#[derive(Debug)]
pub struct TypeEnv {
    /// Variable bindings in nested scopes
    variables: ScopeTracker<PlutoType>,
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
    pub stages: Vec<(String, ClassInfo)>,
    pub di_order: Vec<String>,
    /// DI singletons that need rwlock synchronization (accessed concurrently from spawn + main)
    pub synchronized_singletons: HashSet<String>,
    /// Per-function error sets: maps function name to set of error type names it can raise.
    /// Populated by the error inference pass.
    pub fn_errors: HashMap<String, HashSet<String>>,
    // Generics
    pub generic_functions: HashMap<String, GenericFuncSig>,
    pub generic_classes: HashMap<String, GenericClassInfo>,
    pub generic_enums: HashMap<String, GenericEnumInfo>,
    pub instantiations: HashSet<Instantiation>,
    pub generic_rewrites: HashMap<(usize, usize), String>,
    /// Method resolutions recorded during type inference, keyed by (current_fn_mangled_name, method.span.start)
    pub method_resolutions: HashMap<(String, usize), MethodResolution>,
    /// Built-in call sites that are fallible, keyed by (current_fn_mangled_name, call_name.span.start)
    pub fallible_builtin_calls: HashSet<(String, usize)>,
    /// Currently being type-checked function's mangled name (set by check_function)
    pub current_fn: Option<String>,
    /// Ambient types declared in the app (for validation)
    pub ambient_types: HashSet<String>,
    /// Nesting depth of loops (for validating break/continue)
    pub loop_depth: u32,
    /// Spawn span → target function name
    pub spawn_target_fns: HashMap<(usize, usize), String>,
    /// Scope-mirrored: variable name → spawned function name (for let bindings only)
    task_origins: ScopeTracker<String>,
    /// Function-level: task variable names whose origin is permanently unknown due to Stmt::Assign
    pub invalidated_task_vars: HashSet<String>,
    /// Closure span → return type (set during typeck, used during closure lifting)
    pub closure_return_types: HashMap<(usize, usize), PlutoType>,
    /// Mangled names of methods that declare `mut self`
    pub mut_self_methods: HashSet<String>,
    /// Scope-mirrored: tracks variables declared with `let` (not `let mut`)
    /// Uses () as value type - presence of key indicates immutability
    immutable_vars: ScopeTracker<()>,
    /// Variable declarations: (var_name, scope_depth) → declaration span
    pub variable_decls: HashMap<(String, usize), Span>,
    /// Variable reads: (var_name, scope_depth)
    pub variable_reads: HashSet<(String, usize)>,
    /// Scope block resolutions: keyed by (span.start, span.end) of the Stmt::Scope node
    pub scope_resolutions: HashMap<(usize, usize), ScopeResolution>,
    /// Stack of active scope binding names (for spawn-safety checks).
    /// Each entry is the set of binding names introduced by one scope block.
    /// Uses () as value type - presence of key indicates binding exists in scope
    pub scope_bindings: ScopeTracker<()>,
    /// Classes whose lifecycle was overridden by app-level directives
    pub lifecycle_overridden: HashSet<String>,
    /// Spans of closures that capture scope bindings (tainted closures)
    pub scope_tainted_closures: HashSet<(usize, usize)>,
    /// Stack of sets: local vars holding tainted closures at each scope-block depth
    /// Uses () as value type - presence of key indicates var is tainted
    pub scope_tainted: ScopeTracker<()>,
    /// Stack: scope_depth at each scope block entry (for detecting outer-variable assignments)
    pub scope_body_depths: Vec<usize>,
    /// Names of functions that are generators (return stream T)
    pub generators: HashSet<String>,
    /// When type-checking a generator body, holds the element type T from `stream T`
    pub current_generator_elem: Option<PlutoType>,
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeEnv {
    pub fn new() -> Self {
        let mut builtins = HashSet::new();
        builtins.insert("print".to_string());
        builtins.insert("time_ns".to_string());
        builtins.insert("abs".to_string());
        builtins.insert("min".to_string());
        builtins.insert("max".to_string());
        builtins.insert("pow".to_string());
        builtins.insert("sqrt".to_string());
        builtins.insert("floor".to_string());
        builtins.insert("ceil".to_string());
        builtins.insert("round".to_string());
        builtins.insert("sin".to_string());
        builtins.insert("cos".to_string());
        builtins.insert("tan".to_string());
        builtins.insert("log".to_string());
        builtins.insert("gc_heap_size".to_string());
        builtins.insert("expect".to_string());
        builtins.insert("bytes_new".to_string());
        Self {
            variables: ScopeTracker::with_initial_scope(),
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
            stages: Vec::new(),
            di_order: Vec::new(),
            synchronized_singletons: HashSet::new(),
            fn_errors: HashMap::new(),
            generic_functions: HashMap::new(),
            generic_classes: HashMap::new(),
            generic_enums: HashMap::new(),
            instantiations: HashSet::new(),
            generic_rewrites: HashMap::new(),
            method_resolutions: HashMap::new(),
            fallible_builtin_calls: HashSet::new(),
            current_fn: None,
            ambient_types: HashSet::new(),
            loop_depth: 0,
            spawn_target_fns: HashMap::new(),
            task_origins: ScopeTracker::with_initial_scope(),
            invalidated_task_vars: HashSet::new(),
            closure_return_types: HashMap::new(),
            mut_self_methods: HashSet::new(),
            immutable_vars: ScopeTracker::with_initial_scope(),
            variable_decls: HashMap::new(),
            variable_reads: HashSet::new(),
            scope_resolutions: HashMap::new(),
            scope_bindings: ScopeTracker::new(),
            lifecycle_overridden: HashSet::new(),
            scope_tainted_closures: HashSet::new(),
            scope_tainted: ScopeTracker::new(),
            scope_body_depths: Vec::new(),
            generators: HashSet::new(),
            current_generator_elem: None,
        }
    }

    pub fn push_scope(&mut self) {
        self.variables.push_scope();
        self.task_origins.push_scope();
        self.immutable_vars.push_scope();
    }

    pub fn pop_scope(&mut self) {
        self.variables.pop_scope();
        self.task_origins.pop_scope();
        self.immutable_vars.pop_scope();
    }

    pub fn define(&mut self, name: String, ty: PlutoType) {
        self.variables.insert(name, ty);
    }

    pub fn lookup(&self, name: &str) -> Option<&PlutoType> {
        self.variables.lookup(name)
    }

    pub fn scope_depth(&self) -> usize {
        self.variables.depth()
    }

    /// Look up a variable and return its type along with the scope depth it was found at (0-indexed from bottom)
    pub fn lookup_with_depth(&self, name: &str) -> Option<(&PlutoType, usize)> {
        self.variables.lookup_with_depth(name)
    }

    pub fn class_implements_trait(&self, class_name: &str, trait_name: &str) -> bool {
        self.classes.get(class_name)
            .map(|c| c.impl_traits.iter().any(|t| t == trait_name))
            .unwrap_or(false)
    }

    pub fn is_fn_fallible(&self, name: &str) -> bool {
        self.fn_errors.get(name).is_some_and(|e| !e.is_empty())
    }

    pub fn is_trait_method_potentially_fallible(&self, trait_name: &str, method_name: &str) -> bool {
        for (class_name, info) in &self.classes {
            if info.impl_traits.iter().any(|t| t == trait_name) {
                let mangled = mangle_method(class_name, method_name);
                if self.is_fn_fallible(&mangled) {
                    return true;
                }
            }
        }
        false
    }

    pub fn resolve_method_fallibility(&self, current_fn: &str, span_start: usize) -> Result<bool, String> {
        let key = (current_fn.to_string(), span_start);
        match self.method_resolutions.get(&key) {
            Some(MethodResolution::Class { mangled_name }) => Ok(self.is_fn_fallible(mangled_name)),
            Some(MethodResolution::TraitDynamic { trait_name, method_name }) => {
                Ok(self.is_trait_method_potentially_fallible(trait_name, method_name))
            }
            Some(MethodResolution::Builtin) => Ok(false),
            Some(MethodResolution::TaskGet { spawned_fn }) => {
                match spawned_fn {
                    Some(fn_name) => Ok(self.is_fn_fallible(fn_name)),
                    None => Ok(true), // conservatively fallible
                }
            }
            Some(MethodResolution::ChannelSend) => Ok(true),
            Some(MethodResolution::ChannelRecv) => Ok(true),
            Some(MethodResolution::ChannelTrySend) => Ok(true),
            Some(MethodResolution::ChannelTryRecv) => Ok(true),
            Some(MethodResolution::TaskDetach) => Ok(false),
            Some(MethodResolution::TaskCancel) => Ok(false),
            None => Err(format!(
                "internal error: unresolved method resolution at span {} in fn '{}'",
                span_start, current_fn
            )),
        }
    }

    pub fn define_task_origin(&mut self, name: String, fn_name: String) {
        self.task_origins.insert(name, fn_name);
    }

    pub fn lookup_task_origin(&self, name: &str) -> Option<&String> {
        if self.invalidated_task_vars.contains(name) {
            return None;
        }
        self.task_origins.lookup(name)
    }

    pub fn mark_immutable(&mut self, name: &str) {
        self.immutable_vars.insert(name.to_string(), ());
    }

    pub fn is_immutable(&self, name: &str) -> bool {
        self.immutable_vars.contains(name)
    }
}

pub fn mangle_method(class_or_app: &str, method: &str) -> String {
    format!("{}${}", class_or_app, method)
}

pub fn mangle_name(base: &str, type_args: &[PlutoType]) -> String {
    let suffixes: Vec<String> = type_args.iter().map(mangle_type).collect();
    format!("{}$${}", base, suffixes.join("$"))
}

fn mangle_type(ty: &PlutoType) -> String {
    match ty {
        PlutoType::Int => "int".into(),
        PlutoType::Float => "float".into(),
        PlutoType::Bool => "bool".into(),
        PlutoType::String => "string".into(),
        PlutoType::Void => "void".into(),
        PlutoType::Class(n) | PlutoType::Enum(n) => n.clone(),
        PlutoType::Array(inner) => format!("arr${}", mangle_type(inner)),
        PlutoType::Fn(ps, r) => {
            let ps: Vec<_> = ps.iter().map(mangle_type).collect();
            format!("fn${}$ret${}", ps.join("$"), mangle_type(r))
        }
        PlutoType::Map(k, v) => format!("map${}${}", mangle_type(k), mangle_type(v)),
        PlutoType::Set(t) => format!("set${}", mangle_type(t)),
        PlutoType::Trait(n) => n.clone(),
        PlutoType::TypeParam(n) => n.clone(),
        PlutoType::Range => "range".into(),
        PlutoType::Error => "error".into(),
        PlutoType::Task(inner) => format!("task${}", mangle_type(inner)),
        PlutoType::Byte => "byte".into(),
        PlutoType::Bytes => "bytes".into(),
        PlutoType::Sender(inner) => format!("sender${}", mangle_type(inner)),
        PlutoType::Receiver(inner) => format!("receiver${}", mangle_type(inner)),
        PlutoType::GenericInstance(_, name, args) => {
            let suffixes: Vec<String> = args.iter().map(mangle_type).collect();
            format!("{}$${}", name, suffixes.join("$"))
        }
        PlutoType::Nullable(inner) => format!("nullable${}", mangle_type(inner)),
        PlutoType::Stream(inner) => format!("stream${}", mangle_type(inner)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== mangle_method tests =====

    #[test]
    fn test_mangle_method_class() {
        assert_eq!(mangle_method("Counter", "increment"), "Counter$increment");
        assert_eq!(mangle_method("User", "getName"), "User$getName");
    }

    #[test]
    fn test_mangle_method_app() {
        assert_eq!(mangle_method("MyApp", "main"), "MyApp$main");
        assert_eq!(mangle_method("WebApp", "start"), "WebApp$start");
    }

    #[test]
    fn test_mangle_method_module_prefixed() {
        assert_eq!(mangle_method("math.Vector", "add"), "math.Vector$add");
        assert_eq!(mangle_method("http.Server", "listen"), "http.Server$listen");
    }

    // ===== mangle_name tests =====

    #[test]
    fn test_mangle_name_single_type_arg() {
        let type_args = vec![PlutoType::Int];
        assert_eq!(mangle_name("identity", &type_args), "identity$$int");
    }

    #[test]
    fn test_mangle_name_multiple_type_args() {
        let type_args = vec![PlutoType::Int, PlutoType::String];
        assert_eq!(mangle_name("Pair", &type_args), "Pair$$int$string");
    }

    #[test]
    fn test_mangle_name_complex_type_args() {
        let type_args = vec![
            PlutoType::Array(Box::new(PlutoType::Int)),
            PlutoType::Map(Box::new(PlutoType::String), Box::new(PlutoType::Float)),
        ];
        assert_eq!(mangle_name("process", &type_args), "process$$arr$int$map$string$float");
    }

    #[test]
    fn test_mangle_name_no_type_args() {
        let type_args = vec![];
        assert_eq!(mangle_name("foo", &type_args), "foo$$");
    }

    // ===== mangle_type tests =====

    #[test]
    fn test_mangle_type_primitives() {
        assert_eq!(mangle_type(&PlutoType::Int), "int");
        assert_eq!(mangle_type(&PlutoType::Float), "float");
        assert_eq!(mangle_type(&PlutoType::Bool), "bool");
        assert_eq!(mangle_type(&PlutoType::String), "string");
        assert_eq!(mangle_type(&PlutoType::Void), "void");
        assert_eq!(mangle_type(&PlutoType::Byte), "byte");
        assert_eq!(mangle_type(&PlutoType::Bytes), "bytes");
        assert_eq!(mangle_type(&PlutoType::Range), "range");
        assert_eq!(mangle_type(&PlutoType::Error), "error");
    }

    #[test]
    fn test_mangle_type_class() {
        assert_eq!(mangle_type(&PlutoType::Class("User".to_string())), "User");
        assert_eq!(mangle_type(&PlutoType::Class("math.Point".to_string())), "math.Point");
    }

    #[test]
    fn test_mangle_type_enum() {
        assert_eq!(mangle_type(&PlutoType::Enum("Option".to_string())), "Option");
        assert_eq!(mangle_type(&PlutoType::Enum("Result".to_string())), "Result");
    }

    #[test]
    fn test_mangle_type_trait() {
        assert_eq!(mangle_type(&PlutoType::Trait("Printable".to_string())), "Printable");
        assert_eq!(mangle_type(&PlutoType::Trait("Comparable".to_string())), "Comparable");
    }

    #[test]
    fn test_mangle_type_type_param() {
        assert_eq!(mangle_type(&PlutoType::TypeParam("T".to_string())), "T");
        assert_eq!(mangle_type(&PlutoType::TypeParam("U".to_string())), "U");
    }

    #[test]
    fn test_mangle_type_array() {
        assert_eq!(mangle_type(&PlutoType::Array(Box::new(PlutoType::Int))), "arr$int");
        assert_eq!(
            mangle_type(&PlutoType::Array(Box::new(PlutoType::String))),
            "arr$string"
        );
    }

    #[test]
    fn test_mangle_type_nested_array() {
        let nested = PlutoType::Array(Box::new(PlutoType::Array(Box::new(PlutoType::Int))));
        assert_eq!(mangle_type(&nested), "arr$arr$int");
    }

    #[test]
    fn test_mangle_type_map() {
        let map = PlutoType::Map(Box::new(PlutoType::String), Box::new(PlutoType::Int));
        assert_eq!(mangle_type(&map), "map$string$int");
    }

    #[test]
    fn test_mangle_type_set() {
        let set = PlutoType::Set(Box::new(PlutoType::Int));
        assert_eq!(mangle_type(&set), "set$int");
    }

    #[test]
    fn test_mangle_type_task() {
        let task = PlutoType::Task(Box::new(PlutoType::Int));
        assert_eq!(mangle_type(&task), "task$int");
    }

    #[test]
    fn test_mangle_type_nullable() {
        let nullable = PlutoType::Nullable(Box::new(PlutoType::String));
        assert_eq!(mangle_type(&nullable), "nullable$string");
    }

    #[test]
    fn test_mangle_type_stream() {
        let stream = PlutoType::Stream(Box::new(PlutoType::Float));
        assert_eq!(mangle_type(&stream), "stream$float");
    }

    #[test]
    fn test_mangle_type_sender() {
        let sender = PlutoType::Sender(Box::new(PlutoType::Int));
        assert_eq!(mangle_type(&sender), "sender$int");
    }

    #[test]
    fn test_mangle_type_receiver() {
        let receiver = PlutoType::Receiver(Box::new(PlutoType::String));
        assert_eq!(mangle_type(&receiver), "receiver$string");
    }

    #[test]
    fn test_mangle_type_fn_no_params() {
        let fn_type = PlutoType::Fn(vec![], Box::new(PlutoType::Void));
        assert_eq!(mangle_type(&fn_type), "fn$$ret$void");
    }

    #[test]
    fn test_mangle_type_fn_one_param() {
        let fn_type = PlutoType::Fn(vec![PlutoType::Int], Box::new(PlutoType::String));
        assert_eq!(mangle_type(&fn_type), "fn$int$ret$string");
    }

    #[test]
    fn test_mangle_type_fn_multiple_params() {
        let fn_type = PlutoType::Fn(
            vec![PlutoType::Int, PlutoType::Float, PlutoType::Bool],
            Box::new(PlutoType::String),
        );
        assert_eq!(mangle_type(&fn_type), "fn$int$float$bool$ret$string");
    }

    #[test]
    fn test_mangle_type_generic_instance_single_arg() {
        use crate::typeck::types::GenericKind;
        let generic = PlutoType::GenericInstance(
            GenericKind::Class,
            "Box".to_string(),
            vec![PlutoType::Int],
        );
        assert_eq!(mangle_type(&generic), "Box$$int");
    }

    #[test]
    fn test_mangle_type_generic_instance_multiple_args() {
        use crate::typeck::types::GenericKind;
        let generic = PlutoType::GenericInstance(
            GenericKind::Class,
            "Pair".to_string(),
            vec![PlutoType::String, PlutoType::Float],
        );
        assert_eq!(mangle_type(&generic), "Pair$$string$float");
    }

    #[test]
    fn test_mangle_type_complex_nested() {
        // Map<string, Array<int>>
        let complex = PlutoType::Map(
            Box::new(PlutoType::String),
            Box::new(PlutoType::Array(Box::new(PlutoType::Int))),
        );
        assert_eq!(mangle_type(&complex), "map$string$arr$int");
    }

    #[test]
    fn test_mangle_type_function_with_complex_params() {
        // fn(Array<int>, Map<string, float>) bool
        let fn_type = PlutoType::Fn(
            vec![
                PlutoType::Array(Box::new(PlutoType::Int)),
                PlutoType::Map(Box::new(PlutoType::String), Box::new(PlutoType::Float)),
            ],
            Box::new(PlutoType::Bool),
        );
        assert_eq!(mangle_type(&fn_type), "fn$arr$int$map$string$float$ret$bool");
    }
}
