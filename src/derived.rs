//! Derived analysis data extracted from TypeEnv after type checking.
//!
//! This module bridges the gap between the compiler's transient type analysis
//! (stored in `TypeEnv`) and the serialized PLTO binary format. It captures
//! error sets, resolved function signatures, fallibility, class/trait/enum/error
//! type information, and DI wiring — keyed by AST node UUID — so that consumers
//! (like the SDK) can query type information without re-running the compiler.

use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

use crate::parser::ast::{Lifecycle, Program};
use crate::span::Spanned;
use crate::typeck::env::{mangle_method, TypeEnv};
use crate::typeck::types::PlutoType;
use crate::visit::{walk_expr, Visitor};

/// Map an AST function node to the key used in TypeEnv.
/// `class_name`: `Some("Counter")` for methods, `None` for top-level fns.
pub fn typeenv_key(fn_name: &str, class_name: Option<&str>) -> String {
    match class_name {
        Some(cls) => mangle_method(cls, fn_name),
        None => fn_name.to_string(),
    }
}

/// Derived analysis data for a program, indexed by AST node UUID.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DerivedInfo {
    /// Error sets: function UUID -> list of errors it can raise (sorted by name).
    pub fn_error_sets: BTreeMap<Uuid, Vec<ErrorRef>>,
    /// Resolved function signatures: function UUID -> resolved param/return types.
    pub fn_signatures: BTreeMap<Uuid, ResolvedSignature>,
    /// Resolved class type info: class UUID -> fields, methods, traits, lifecycle.
    #[serde(default)]
    pub class_infos: BTreeMap<Uuid, ResolvedClassInfo>,
    /// Resolved trait type info: trait UUID -> methods with signatures.
    #[serde(default)]
    pub trait_infos: BTreeMap<Uuid, ResolvedTraitInfo>,
    /// Resolved enum type info: enum UUID -> variants with fields.
    #[serde(default)]
    pub enum_infos: BTreeMap<Uuid, ResolvedEnumInfo>,
    /// Resolved error type info: error UUID -> fields with types.
    #[serde(default)]
    pub error_infos: BTreeMap<Uuid, ResolvedErrorInfo>,
    /// DI instantiation order (class UUIDs in topological order).
    #[serde(default)]
    pub di_order: Vec<Uuid>,
    /// Trait UUID -> list of class UUIDs that implement it.
    #[serde(default)]
    pub trait_implementors: BTreeMap<Uuid, Vec<Uuid>>,
    /// Test dependency hashes: test display_name -> hash of all transitive dependencies.
    #[serde(default)]
    pub test_dep_hashes: BTreeMap<String, String>,
    /// SHA-256 hash of the source text this derived data was computed from.
    /// Used for staleness detection. Empty string if not computed.
    #[serde(default)]
    pub source_hash: String,
    /// The assumption surface (rfc-properties.md phase 5, epistemics.md):
    /// every property claim discharged by ASSUMPTION rather than proof or
    /// check — the complete list of things the program rests on that
    /// nobody proved. Rendered by `pluto analyze`.
    #[serde(default)]
    pub assumptions: Vec<AssumedClaim>,
    /// Every property claim discharged in CHECKED mode (rfc-properties.md
    /// phase 5.5): the compiler proved the runtime guard's placement (the
    /// dedup-guard shape); the guard's data is evaluated at runtime.
    /// Rendered by `pluto analyze` alongside the assumption surface.
    #[serde(default)]
    pub checked_claims: Vec<CheckedClaim>,
    /// Number of call sites whose callee `requires` clauses were proven
    /// statically from the caller's flow facts (the runtime entry check is
    /// elided at these sites — src/typeck/requires.rs).
    #[serde(default)]
    pub proven_requires_sites: usize,
    /// Runtime residue of #416 phase 2: int `+`/`-`/`*` sites whose
    /// overflow check remains at runtime (the flow facts could not prove
    /// the result fits in i64).
    #[serde(default)]
    pub arith_sites_checked: usize,
    /// Int arithmetic sites whose overflow check was elided on an interval
    /// proof (src/typeck/arith_fit.rs) — codegen emits the raw op.
    #[serde(default)]
    pub arith_sites_elided: usize,
    /// Runtime residue of #441: `<<`/`>>` sites with a non-constant amount
    /// whose 0..63 range check remains at runtime.
    #[serde(default)]
    pub shift_sites_checked: usize,
    /// Non-constant-amount shift sites whose range check was elided on an
    /// interval proof (src/typeck/arith_fit.rs).
    #[serde(default)]
    pub shift_sites_elided: usize,
    /// The perception surface (rfc-module-semantics.md section 4): every
    /// boundary-crossing type whose single-state invariants are re-checked
    /// at decode (raising wire.WireError on violation). Decoded data is
    /// testimony; these checks are how the receiver upgrades it to
    /// perception. Rendered by `pluto analyze` beside the assumption
    /// surface and the arithmetic residue.
    #[serde(default)]
    pub perception_surface: Vec<PerceptionEntry>,
}

/// One perception entry: a boundary-crossing type and the invariant
/// predicates validated at its decode funnel.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PerceptionEntry {
    /// The decoded type's flattened name, e.g. `blob.WriteGrant`.
    pub type_name: String,
    /// Rendered invariant predicates, e.g. `self.epoch >= 0`.
    pub invariants: Vec<String>,
}

impl std::fmt::Display for PerceptionEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} — {}", self.type_name, self.invariants.join("; "))
    }
}

/// One checked claim: the providing method, the property, its
/// instantiation bindings, and the clause's 1-based source line.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CheckedClaim {
    /// The providing declaration, e.g. `method PaymentLedger.apply`.
    pub owner: String,
    /// Resolved property name, e.g. `verify.idempotent`.
    pub property: String,
    /// Rendered instantiation bindings, e.g. `key = req`.
    pub instantiation: String,
    /// 1-based source line of the provides clause in its declaring file.
    pub line: usize,
}

impl std::fmt::Display for CheckedClaim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "checked {}({}) — {} (line {})",
            self.property, self.instantiation, self.owner, self.line
        )
    }
}

/// One assumed claim: owner (the declaration that vouches), the property,
/// its instantiation bindings, and the owner's 1-based source line.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssumedClaim {
    /// The vouching declaration, e.g. `extern fn __pluto_string_len`.
    pub owner: String,
    /// Resolved property name, e.g. `verify.idempotent`.
    pub property: String,
    /// Rendered instantiation bindings, e.g. `key = s`.
    pub instantiation: String,
    /// 1-based source line of the assume clause in its declaring file.
    pub line: usize,
}

impl std::fmt::Display for AssumedClaim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "assume {}({}) — {} (line {})",
            self.property, self.instantiation, self.owner, self.line
        )
    }
}

/// A reference to an error declaration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorRef {
    /// UUID of the error declaration (`None` for extern/unresolved errors).
    pub id: Option<Uuid>,
    /// Error type name (human-readable, for display).
    pub name: String,
}

/// Resolved function signature with concrete types.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedSignature {
    pub param_types: Vec<PlutoType>,
    pub return_type: PlutoType,
    pub is_fallible: bool,
}

/// Resolved class info extracted from TypeEnv.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedClassInfo {
    pub fields: Vec<ResolvedFieldInfo>,
    pub methods: Vec<(String, ResolvedSignature)>,
    pub impl_traits: Vec<String>,
    pub lifecycle: Lifecycle,
    pub is_pub: bool,
    /// Properties the type provides via `satisfies` (rfc-properties.md
    /// slice 2), rendered as `name(arg, ...)` — the exported names
    /// downstream requires/assume matching (phase 5) consumes.
    #[serde(default)]
    pub provided_properties: Vec<String>,
}

/// A single resolved field with its concrete type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedFieldInfo {
    pub name: String,
    pub ty: PlutoType,
    pub is_injected: bool,
}

/// Resolved trait info extracted from TypeEnv.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedTraitInfo {
    pub methods: Vec<(String, ResolvedSignature)>,
    pub default_methods: Vec<String>,
    pub implementors: Vec<Uuid>,
}

/// Resolved enum info extracted from TypeEnv.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedEnumInfo {
    pub variants: Vec<ResolvedVariantInfo>,
}

/// A single resolved enum variant with its fields.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedVariantInfo {
    pub name: String,
    pub fields: Vec<ResolvedFieldInfo>,
}

/// Resolved error declaration info extracted from TypeEnv.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedErrorInfo {
    pub fields: Vec<ResolvedFieldInfo>,
}

/// Collects all function names, class names, and enum names that a test transitively depends on.
/// Returns a sorted list for stable hashing.
fn collect_test_dependencies(
    test_fn_name: &str,
    program: &Program,
    visited: &mut std::collections::HashSet<String>,
    deps: &mut Vec<String>,
) {
    if visited.contains(test_fn_name) {
        return;
    }
    visited.insert(test_fn_name.to_string());
    deps.push(test_fn_name.to_string());

    // Find the test function
    let test_fn = program.functions.iter().find(|f| f.node.name.node == test_fn_name);
    if let Some(func) = test_fn {
        // Collect dependencies from the function body
        collect_deps_from_function_body(&func.node, program, visited, deps);
    }
}

/// Visitor that collects test dependencies from expressions.
struct DependencyCollector<'a> {
    program: &'a Program,
    visited: &'a mut HashSet<String>,
    deps: &'a mut Vec<String>,
}

impl Visitor for DependencyCollector<'_> {
    fn visit_expr(&mut self, expr: &Spanned<crate::parser::ast::Expr>) {
        use crate::parser::ast::Expr;

        match &expr.node {
            Expr::Call { name, .. } => {
                let fn_name = &name.node;
                // CRITICAL: Preserve transitive dependency collection
                collect_test_dependencies(fn_name, self.program, self.visited, self.deps);
            }
            Expr::StaticTraitCall { trait_name, method_name, .. } => {
                // NEW: Collect trait method as dependency
                let dep_name = format!("{}::{}", trait_name.node, method_name.node);
                if !self.visited.contains(&dep_name) {
                    self.visited.insert(dep_name.clone());
                    self.deps.push(dep_name);
                    // Note: Trait methods don't have bodies to recurse into (interface only)
                }
            }
            Expr::StructLit { name, .. } => {
                // Track class usage
                let class_name = &name.node;
                if !self.visited.contains(class_name) {
                    self.visited.insert(class_name.clone());
                    self.deps.push(class_name.clone());
                }
            }
            Expr::EnumUnit { enum_name, .. } | Expr::EnumData { enum_name, .. } => {
                // Track enum usage
                let enum_name_str = &enum_name.node;
                if !self.visited.contains(enum_name_str) {
                    self.visited.insert(enum_name_str.clone());
                    self.deps.push(enum_name_str.clone());
                }
            }
            Expr::ClosureCreate { fn_name, .. } => {
                // Track closure dependencies
                collect_test_dependencies(fn_name, self.program, self.visited, self.deps);
            }
            _ => {}
        }
        // Always recurse to find nested dependencies
        walk_expr(self, expr);
    }
}

/// Collect dependencies from a function body using the visitor pattern.
fn collect_deps_from_function_body(
    func: &crate::parser::ast::Function,
    program: &Program,
    visited: &mut HashSet<String>,
    deps: &mut Vec<String>,
) {
    let mut collector = DependencyCollector { program, visited, deps };
    collector.visit_block(&func.body);
}

/// Compute stable dependency hashes for all tests
fn compute_test_dependency_hashes(program: &Program) -> BTreeMap<String, String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut test_dep_hashes = BTreeMap::new();

    for test_info in &program.test_info {
        let mut visited = std::collections::HashSet::new();
        let mut deps = Vec::new();

        // Collect all transitive dependencies
        collect_test_dependencies(&test_info.fn_name, program, &mut visited, &mut deps);

        // Sort for stable hashing
        deps.sort();

        // Hash the sorted dependency list along with function bodies
        let mut hasher = DefaultHasher::new();
        // The test's pin config is part of its identity: editing a
        // [schedule:/seed:] bracket must re-run the test even when the
        // body is unchanged.
        test_info.schedule.hash(&mut hasher);
        test_info.seed.hash(&mut hasher);
        test_info.iteration.hash(&mut hasher);
        if let Some(tests_decl) = &program.tests {
            tests_decl.node.strategy.hash(&mut hasher);
            tests_decl.node.seed.hash(&mut hasher);
            tests_decl.node.iterations.hash(&mut hasher);
        }
        for dep_name in &deps {
            // Hash the name
            dep_name.hash(&mut hasher);

            // Hash the function body if it's a function
            if let Some(func) = program.functions.iter().find(|f| f.node.name.node == *dep_name) {
                // Hash the function's body (simplified - using debug representation)
                // In production, you might want a more sophisticated AST hash
                format!("{:?}", func.node.body).hash(&mut hasher);
            }
            // Hash class definitions
            else if let Some(class) = program.classes.iter().find(|c| c.node.name.node == *dep_name) {
                format!("{:?}", class.node.fields).hash(&mut hasher);
                format!("{:?}", class.node.methods).hash(&mut hasher);
            }
            // Hash enum definitions
            else if let Some(enum_decl) = program.enums.iter().find(|e| e.node.name.node == *dep_name) {
                format!("{:?}", enum_decl.node.variants).hash(&mut hasher);
            }
        }

        let hash_value = hasher.finish();
        test_dep_hashes.insert(
            test_info.display_name.clone(),
            format!("{:x}", hash_value),
        );
    }

    test_dep_hashes
}

impl DerivedInfo {
    /// Compute hash of source text for staleness detection using SHA-256.
    pub fn compute_source_hash(source: &str) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(source.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Check if this derived data is stale compared to the given source.
    /// Returns true if the source hash doesn't match (data is outdated).
    pub fn is_stale(&self, source: &str) -> bool {
        if self.source_hash.is_empty() {
            // No hash stored - treat as stale (needs analysis)
            return true;
        }
        let current_hash = Self::compute_source_hash(source);
        self.source_hash != current_hash
    }

    /// Build derived info by walking the program AST and extracting type data from TypeEnv.
    ///
    /// The `source` parameter is used to compute a staleness hash for the derived data.
    pub fn build(env: &TypeEnv, program: &Program, source: &str) -> Self {
        // Build a lookup from error name -> UUID using the program's error declarations
        let error_uuid_map: BTreeMap<&str, Uuid> = program
            .errors
            .iter()
            .map(|e| (e.node.name.node.as_str(), e.node.id))
            .collect();

        let mut fn_error_sets = BTreeMap::new();
        let mut fn_signatures = BTreeMap::new();

        // Process top-level functions
        for f in &program.functions {
            let name = &f.node.name.node;

            // Skip lifted closures
            if name.starts_with("__closure_") || env.closure_fns.contains_key(name) {
                continue;
            }

            let key = typeenv_key(name, None);
            Self::collect_fn_data(
                f.node.id,
                &key,
                env,
                &error_uuid_map,
                &mut fn_error_sets,
                &mut fn_signatures,
            );
        }

        // Process class methods
        for class in &program.classes {
            let class_name = &class.node.name.node;
            for method in &class.node.methods {
                let key = typeenv_key(&method.node.name.node, Some(class_name));
                Self::collect_fn_data(
                    method.node.id,
                    &key,
                    env,
                    &error_uuid_map,
                    &mut fn_error_sets,
                    &mut fn_signatures,
                );
            }
        }

        // Process app methods
        if let Some(app) = &program.app {
            let app_name = &app.node.name.node;
            for method in &app.node.methods {
                let key = typeenv_key(&method.node.name.node, Some(app_name));
                Self::collect_fn_data(
                    method.node.id,
                    &key,
                    env,
                    &error_uuid_map,
                    &mut fn_error_sets,
                    &mut fn_signatures,
                );
            }
        }

        // Process stage methods
        for stage in &program.stages {
            let stage_name = &stage.node.name.node;
            for method in &stage.node.methods {
                let key = typeenv_key(&method.node.name.node, Some(stage_name));
                Self::collect_fn_data(
                    method.node.id,
                    &key,
                    env,
                    &error_uuid_map,
                    &mut fn_error_sets,
                    &mut fn_signatures,
                );
            }
        }

        // Build name->UUID maps for classes, traits, enums
        let class_name_to_uuid: BTreeMap<&str, Uuid> = program
            .classes
            .iter()
            .map(|c| (c.node.name.node.as_str(), c.node.id))
            .collect();

        // Build class_infos
        let mut class_infos = BTreeMap::new();
        for class in &program.classes {
            let class_name = &class.node.name.node;
            if let Some(ci) = env.classes.get(class_name.as_str()) {
                let fields: Vec<ResolvedFieldInfo> = ci
                    .fields
                    .iter()
                    .map(|(name, ty, is_injected)| ResolvedFieldInfo {
                        name: name.clone(),
                        ty: ty.clone(),
                        is_injected: *is_injected,
                    })
                    .collect();

                let methods: Vec<(String, ResolvedSignature)> = ci
                    .methods
                    .iter()
                    .filter_map(|method_name| {
                        let key = typeenv_key(method_name, Some(class_name));
                        env.functions.get(&key).map(|sig| {
                            let is_fallible = env
                                .fn_errors
                                .get(&key)
                                .is_some_and(|errs| !errs.is_empty());
                            (
                                method_name.clone(),
                                ResolvedSignature {
                                    param_types: sig.params.clone(),
                                    return_type: sig.return_type.clone(),
                                    is_fallible,
                                },
                            )
                        })
                    })
                    .collect();

                let provided_properties = env
                    .class_properties
                    .get(class_name)
                    .map(|props| props.iter().map(|p| p.to_string()).collect())
                    .unwrap_or_default();

                class_infos.insert(
                    class.node.id,
                    ResolvedClassInfo {
                        fields,
                        methods,
                        impl_traits: ci.impl_traits.clone(),
                        lifecycle: ci.lifecycle,
                        is_pub: class.node.is_pub,
                        provided_properties,
                    },
                );
            }
        }

        // Build trait_infos + trait_implementors
        let mut trait_infos = BTreeMap::new();
        let mut trait_implementors: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();

        for tr in &program.traits {
            let trait_name = &tr.node.name.node;
            if let Some(ti) = env.traits.get(trait_name.as_str()) {
                let methods: Vec<(String, ResolvedSignature)> = ti
                    .methods
                    .iter()
                    .map(|(name, sig)| {
                        let is_fallible =
                            env.is_trait_method_potentially_fallible(trait_name, name);
                        (
                            name.clone(),
                            ResolvedSignature {
                                param_types: sig.params.clone(),
                                return_type: sig.return_type.clone(),
                                is_fallible,
                            },
                        )
                    })
                    .collect();

                // Find implementors by scanning classes
                let mut implementors = Vec::new();
                for (cls_name, cls_info) in &env.classes {
                    if cls_info.impl_traits.iter().any(|t| t == trait_name)
                        && let Some(&uuid) = class_name_to_uuid.get(cls_name.as_str())
                    {
                        implementors.push(uuid);
                    }
                }
                implementors.sort();

                let trait_uuid = tr.node.id;
                trait_implementors.insert(trait_uuid, implementors.clone());

                trait_infos.insert(
                    trait_uuid,
                    ResolvedTraitInfo {
                        methods,
                        default_methods: ti.default_methods.clone(),
                        implementors,
                    },
                );
            }
        }

        // Build enum_infos
        let mut enum_infos = BTreeMap::new();
        for en in &program.enums {
            let enum_name = &en.node.name.node;
            if let Some(ei) = env.enums.get(enum_name.as_str()) {
                let variants: Vec<ResolvedVariantInfo> = ei
                    .variants
                    .iter()
                    .map(|(name, fields)| ResolvedVariantInfo {
                        name: name.clone(),
                        fields: fields
                            .iter()
                            .map(|(fname, fty)| ResolvedFieldInfo {
                                name: fname.clone(),
                                ty: fty.clone(),
                                is_injected: false,
                            })
                            .collect(),
                    })
                    .collect();

                enum_infos.insert(en.node.id, ResolvedEnumInfo { variants });
            }
        }

        // Build error_infos
        let mut error_infos = BTreeMap::new();
        for err in &program.errors {
            let err_name = &err.node.name.node;
            if let Some(erri) = env.errors.get(err_name.as_str()) {
                let fields: Vec<ResolvedFieldInfo> = erri
                    .fields
                    .iter()
                    .map(|(name, ty)| ResolvedFieldInfo {
                        name: name.clone(),
                        ty: ty.clone(),
                        is_injected: false,
                    })
                    .collect();

                error_infos.insert(err.node.id, ResolvedErrorInfo { fields });
            }
        }

        // Build di_order: map class names to UUIDs
        let di_order: Vec<Uuid> = env
            .di_order
            .iter()
            .filter_map(|name| class_name_to_uuid.get(name.as_str()).copied())
            .collect();

        // Compute test dependency hashes
        let test_dep_hashes = compute_test_dependency_hashes(program);

        // Compute source hash for staleness detection
        let source_hash = Self::compute_source_hash(source);

        // The assumption surface: every extern `assume` clause, attributed
        // to its declaration. (In-unit provides are PROVEN/CHECKED or
        // rejected at compile time, so externs are the only assumption
        // source today.)
        let mut assumptions: Vec<AssumedClaim> = program
            .extern_fns
            .iter()
            .flat_map(|ext| {
                ext.node.assumes.iter().map(|clause| AssumedClaim {
                    owner: format!("extern fn {}", ext.node.name.node),
                    property: clause.node.name.node.clone(),
                    instantiation: crate::properties::render_provides_args(&clause.node),
                    line: clause.node.line,
                })
            })
            .collect();
        assumptions.sort_by(|a, b| (&a.owner, &a.property).cmp(&(&b.owner, &b.property)));

        // Checked claims (rfc-properties.md phase 5.5): every in-unit
        // provides of a dedup-shaped property — the guard-placement proof
        // discharged these, with the guard's data living at runtime.
        let dedup_props: std::collections::HashSet<&str> = program
            .properties
            .iter()
            .filter(|p| crate::properties::has_dedup_atoms(&p.node))
            .map(|p| p.node.name.node.as_str())
            .collect();
        let mut checked_claims: Vec<CheckedClaim> = Vec::new();
        for class in &program.classes {
            let cname = &class.node.name.node;
            for m in &class.node.methods {
                for clause in &m.node.provides {
                    if dedup_props.contains(clause.node.name.node.as_str()) {
                        checked_claims.push(CheckedClaim {
                            owner: format!("method {cname}.{}", m.node.name.node),
                            property: clause.node.name.node.clone(),
                            instantiation: crate::properties::render_provides_args(&clause.node),
                            line: clause.node.line,
                        });
                    }
                }
            }
        }
        checked_claims.sort_by(|a, b| (&a.owner, &a.property).cmp(&(&b.owner, &b.property)));

        let proven_requires_sites = env.proven_requires_sites.len();
        let arith_sites_elided = env.proven_fit_spans.len();
        let arith_sites_checked = env
            .arith_fit_candidates
            .len()
            .saturating_sub(arith_sites_elided);
        // Perception surface: a class contributes when it carries value
        // invariants AND sits in the boundary seed set (the types phase B
        // generates decode funnels for). Computed from the seed, not from
        // generated functions, so light pipelines (analyze) report the same
        // surface as full compiles; check_decode_coverage separately
        // guarantees real compiles actually generated every funnel.
        let boundary_classes = crate::marshal::perception_boundary_classes(program);
        let mut perception_surface: Vec<PerceptionEntry> = Vec::new();
        for class in &program.classes {
            let cname = &class.node.name.node;
            if !boundary_classes.contains(cname) {
                continue;
            }
            let invariants = crate::marshal::decode_validated_invariants(&class.node);
            if invariants.is_empty() {
                continue;
            }
            perception_surface.push(PerceptionEntry {
                type_name: cname.clone(),
                invariants,
            });
        }
        perception_surface.sort_by(|a, b| a.type_name.cmp(&b.type_name));

        let shift_sites_elided = env.proven_shift_spans.len();
        let shift_sites_checked = env
            .shift_check_candidates
            .len()
            .saturating_sub(shift_sites_elided);

        DerivedInfo {
            fn_error_sets,
            fn_signatures,
            class_infos,
            trait_infos,
            enum_infos,
            error_infos,
            di_order,
            trait_implementors,
            test_dep_hashes,
            source_hash,
            assumptions,
            checked_claims,
            proven_requires_sites,
            arith_sites_checked,
            arith_sites_elided,
            shift_sites_checked,
            shift_sites_elided,
            perception_surface,
        }
    }

    fn collect_fn_data(
        id: Uuid,
        key: &str,
        env: &TypeEnv,
        error_uuid_map: &BTreeMap<&str, Uuid>,
        fn_error_sets: &mut BTreeMap<Uuid, Vec<ErrorRef>>,
        fn_signatures: &mut BTreeMap<Uuid, ResolvedSignature>,
    ) {
        // Error set
        let error_set = if let Some(errors) = env.fn_errors.get(key) {
            let mut refs: Vec<ErrorRef> = errors
                .iter()
                .map(|err_name| ErrorRef {
                    id: error_uuid_map.get(err_name.as_str()).copied(),
                    name: err_name.clone(),
                })
                .collect();
            refs.sort_by(|a, b| a.name.cmp(&b.name));
            refs
        } else {
            vec![]
        };

        let is_fallible = !error_set.is_empty();
        fn_error_sets.insert(id, error_set);

        // Resolved signature
        if let Some(sig) = env.functions.get(key) {
            fn_signatures.insert(
                id,
                ResolvedSignature {
                    param_types: sig.params.clone(),
                    return_type: sig.return_type.clone(),
                    is_fallible,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typeenv_key_top_level() {
        assert_eq!(typeenv_key("main", None), "main");
        assert_eq!(typeenv_key("add", None), "add");
    }

    #[test]
    fn typeenv_key_method() {
        assert_eq!(typeenv_key("increment", Some("Counter")), "Counter$increment");
        assert_eq!(typeenv_key("main", Some("MyApp")), "MyApp$main");
    }

    #[test]
    fn typeenv_key_module_prefixed() {
        // Module-prefixed functions keep their name as-is (prefix is part of the name)
        assert_eq!(typeenv_key("math.add", None), "math.add");
    }

    #[test]
    fn default_has_empty_collections() {
        let d = DerivedInfo::default();
        assert!(d.fn_error_sets.is_empty());
        assert!(d.fn_signatures.is_empty());
        assert!(d.class_infos.is_empty());
        assert!(d.trait_infos.is_empty());
        assert!(d.enum_infos.is_empty());
        assert!(d.error_infos.is_empty());
        assert!(d.di_order.is_empty());
        assert!(d.trait_implementors.is_empty());
    }

    #[test]
    fn test_staleness_tracking() {
        let source = "fn main() {}";
        let hash = DerivedInfo::compute_source_hash(source);

        let mut derived = DerivedInfo::default();
        derived.source_hash = hash;

        // Same source → not stale
        assert!(!derived.is_stale(source));

        // Modified source → stale
        assert!(derived.is_stale("fn main() { let x = 1 }"));

        // No hash (empty string) → stale
        let derived_no_hash = DerivedInfo::default();
        assert!(derived_no_hash.is_stale(source));
    }

    #[test]
    fn test_source_hash_deterministic() {
        let source = "fn add(a: int, b: int) int { a + b }";
        let hash1 = DerivedInfo::compute_source_hash(source);
        let hash2 = DerivedInfo::compute_source_hash(source);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_source_hash_different_for_different_source() {
        let source1 = "fn main() {}";
        let source2 = "fn main() { let x = 1 }";
        let hash1 = DerivedInfo::compute_source_hash(source1);
        let hash2 = DerivedInfo::compute_source_hash(source2);
        assert_ne!(hash1, hash2);
    }
}
