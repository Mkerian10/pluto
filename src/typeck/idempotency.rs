//! Dedup-guard discharge for `provides <property>` claims whose property
//! body is a `dedup <key>` atom — rfc-properties.md phase 5.5, the CHECKED
//! fn-level discharge mode.
//!
//! # The claim
//!
//! A method `fn m(mut self, ...) provides idempotent(key = k)` (where
//! `idempotent` carries a `dedup key` atom) claims: **calling `m` twice on
//! the same instance with the same key, the second call performs no
//! additional externally-visible effect.** Stronger, what is actually
//! proven: across any sequence of calls with the same key value, the
//! method's effect region executes **at most once**.
//!
//! # The obligation (the dedup-guard shape)
//!
//! The proof is a guard-*placement* proof — the compiler proves the check
//! runs before the effect; the check's data (the dedup set) lives at
//! runtime. Three conjuncts:
//!
//! 1. **Armed insert before every effect.** Every externally-visible
//!    effect site in the method body must carry, on every path reaching
//!    it, a live `SetInserted(self.F, key)` fact: `self.F.insert(key)`
//!    executed earlier on this path, itself *armed* — performed while a
//!    `SetNotContains(self.F, key)` fact was live, i.e. dominated by a
//!    membership check (`if self.F.contains(key) { return cached / raise }`
//!    or `if !self.F.contains(key) { ... }`) with nothing between check
//!    and insert that could invalidate it (facts discipline = structural
//!    dominance, exactly as in `dominance.rs`).
//! 2. **Key stability.** The membership check and the insert must read the
//!    *entry value* of the instantiated key expression: the key's root
//!    parameter is never reassigned before them, and (for one-level field
//!    keys like `req.id`) nothing that could mutate the field — a call
//!    that may run user code, any field write — intervenes.
//! 3. **Global monotonicity of the dedup set.** The field `self.F` used by
//!    an armed chain is *insert-only*, whole-program: its only mutation
//!    anywhere is `self.F.insert(...)` inside its own class's methods — no
//!    `remove`/`clear`, no reassignment, no foreign mutation, and no
//!    aliasing (binding it to a local, passing it as an argument,
//!    returning it, iterating it) that would reopen the closed write-set.
//!    Constructions must initialize it with a fresh set literal.
//!
//! # Why the property follows (the soundness argument)
//!
//! Suppose the effect region executes in two calls i < j with the same key
//! value K (same instance). By conjunct 1, call j's effect was preceded on
//! its path by an insert of K armed by a check observing `K ∉ self.F`; by
//! conjunct 2 both read the entry key, so the observed element is K
//! itself. By conjunct 1 applied to call i, `self.F.insert(K)` executed in
//! call i before its effect — so K entered the set during call i. By
//! conjunct 3, K can never leave the set afterward (insert-only,
//! unaliased, never reassigned). So at any point of call j, `K ∈ self.F` —
//! contradicting call j's check. Hence at most one effect per key.
//!
//! The *insert-before-effect* direction matters for partial failure: if
//! the insert came after the effect, a raise between them would leave the
//! effect applied but unrecorded, and a retry would re-apply it. With
//! insert-first, a raise between insert and effect makes the key look
//! processed though the effect never ran — the at-most-once direction,
//! which is exactly what retrying from an AMBIGUOUS failure needs (the
//! retry must be harmless, not guaranteed to complete).
//!
//! # The concurrency side-condition
//!
//! The argument above serializes calls i and j. On an `object` (entity),
//! methods are serialized per instance, so the whole check→insert→effect
//! window of one message closes before the next message runs — reentrant
//! same-message self-calls are calls, and calls kill the membership facts
//! (the check must re-dominate). On a value class, no concurrent caller
//! exists to race the window: values do not share (spawn deep-copies, wire
//! transfers copy), so the claim is per-copy — each binding carries its
//! own dedup set, and sequential calls through one binding are covered.
//! Both are accepted; the entity is the shape that matters at a distance.
//!
//! # What counts as an externally-visible effect (conservative)
//!
//! - Every field assignment and index assignment (any target — a local
//!   could alias reachable state).
//! - Every call that may run user code or perform I/O: user
//!   functions/methods, extern fns, `print`, trait calls, `at`, `spawn`,
//!   `serve`, `yield`, select/scope blocks, channel operations.
//! - Every *mutating* builtin collection method (`push`, `pop`, `clear`,
//!   `insert`, `remove`, ...) on any receiver — with one exemption: the
//!   dedup insert of the claim's own key (re-inserting the same element is
//!   a set-level no-op, and it is the guard mechanism itself).
//!
//! `return` and `raise` are not effects: differing *outcomes* on the
//! duplicate call are allowed (return the cached value, raise
//! AlreadyProcessed); only re-applied effects are not. A consequence the
//! proof enforces deliberately: the already-seen branch runs on every
//! duplicate call, so it must be effect-free.
//!
//! Effects inside closure bodies created in a providing method are
//! rejected: the closure may escape and run outside the serialized
//! check→effect window, so the method's guard cannot cover it.
//!
//! # Fragment limits (documented conservatism)
//!
//! - The key must be a parameter of the providing method or a one-level
//!   field of one (`key = k`, `key = req.id`), matched syntactically at
//!   check/insert sites — no alias tracking. Entity-typed key roots never
//!   match (their fields can change concurrently).
//! - A dotted key (`req.id`) dies at every call that may run user code, so
//!   with a dotted key the check must precede any such call and at most
//!   one call-shaped effect can follow the insert. Bare parameter keys are
//!   fully general: `SetInserted` survives calls — soundly, because the
//!   set is globally insert-only (conjunct 3) and locals cannot be changed
//!   by callees.
//! - Loop boundaries drop all facts (the engine's conservative loop rule):
//!   a check→insert→effect chain must sit entirely outside loops or
//!   entirely inside one body.
//!
//! The pass runs standalone after body checking (the `dominance.rs`
//! model): it re-walks bodies with its own fact environment and
//! best-effort local typing; everything it cannot resolve is conservative.

use std::collections::{HashMap, HashSet};

use crate::diagnostics::CompileError;
use crate::parser::ast::{
    Block, CatchHandler, ClassDecl, Expr, Function, MatchPattern, Program, PropertyDecl,
    PropertyProvenance, Stmt, TypeExpr,
};
use crate::span::{Span, Spanned};
use crate::visit::{walk_expr, Visitor};

use super::env::{mangle_method, MethodResolution, TypeEnv};
use super::facts::{immediate_exprs, Fact, FactEnv};
use super::types::PlutoType;

/// Builtin collection/channel/task methods that mutate their receiver or
/// perform communication — effects, and (for set methods on a dedup field)
/// monotonicity violations. Everything non-builtin is call-shaped and
/// already an effect.
///
/// Deliberately NOT derived from the builtin-method registry
/// (typeck/builtins.rs): this list classifies by analysis meaning — it is
/// name-only (no receiver typing) and spans channel/task methods the
/// registry excludes. Keep the two in sync when adding a mutating
/// collection method.
const MUTATING_BUILTIN_METHODS: &[&str] = &[
    "push", "pop", "clear", "reverse", "remove_at", "insert_at", "insert", "remove", "send",
    "try_send", "close", "recv", "try_recv", "get", "detach", "cancel",
];

/// Builtin set methods that only read — allowed on a dedup field from
/// anywhere.
const PURE_SET_METHODS: &[&str] = &["contains", "len", "to_array"];

/// Builtin free functions that neither run user code nor touch the outside
/// world — the only calls that are not effects.
const EFFECT_FREE_BUILTIN_FNS: &[&str] =
    &["abs", "min", "max", "time_ns", "len", "wrapping_add", "wrapping_sub", "wrapping_mul"];

// ─────────────────────────────────────────────────────────────────────────────
// Obligations and the dedup-field registry
// ─────────────────────────────────────────────────────────────────────────────

/// One dedup claim on a providing method.
#[derive(Debug, Clone)]
struct Obligation {
    /// Resolved property name (`verify.idempotent`).
    property: String,
    /// The instantiated key as a fact path (`k`, `req.id`).
    key_path: String,
    /// The key path's root parameter name.
    key_root: String,
    /// Whether the key is a one-level field path.
    key_dotted: bool,
    /// Still denoting its entry value? Cleared on reassignment of the root
    /// and (dotted keys) on anything that could mutate the field.
    stable: bool,
    provenance: PropertyProvenance,
    /// The provides clause span (method-level diagnostics).
    clause_span: Span,
}

impl Obligation {
    fn blame(&self) -> String {
        self.provenance.blame()
    }
}

/// Dedup fields discovered in armed-chain position: class name → field
/// names. These are the fields conjunct 3 (global monotonicity) governs.
type Registry = HashMap<String, HashSet<String>>;

/// Entry point: discharge every dedup-shaped `provides` claim and enforce
/// the monotonicity of every dedup field, whole-program. Runs after body
/// checking (method resolutions exist), before skolem sweep.
pub(crate) fn check_idempotency(program: &Program, env: &TypeEnv) -> Result<(), CompileError> {
    let props: HashMap<&str, &PropertyDecl> = program
        .properties
        .iter()
        .map(|p| (p.node.name.node.as_str(), &p.node))
        .collect();

    // Collect obligations per providing method.
    let mut method_obls: HashMap<String, Vec<Obligation>> = HashMap::new();
    for class in &program.classes {
        for m in &class.node.methods {
            let mut obls = Vec::new();
            for clause in &m.node.provides {
                let Some(prop) = props.get(clause.node.name.node.as_str()) else {
                    continue; // unknown names were rejected in properties.rs
                };
                if !crate::properties::has_dedup_atoms(prop) {
                    continue;
                }
                let Some(spec) = crate::properties::dedup_key_spec(prop, &clause.node) else {
                    continue; // argument shapes were validated in properties.rs
                };
                let key_path = match &spec.field {
                    Some(f) => format!("{}.{f}", spec.root),
                    None => spec.root.clone(),
                };
                obls.push(Obligation {
                    property: clause.node.name.node.clone(),
                    key_dotted: spec.field.is_some(),
                    key_root: spec.root,
                    key_path,
                    stable: true,
                    provenance: PropertyProvenance {
                        property: clause.node.name.node.clone(),
                        line: spec.atom_line,
                        bindings: crate::properties::render_provides_args(&clause.node),
                    },
                    clause_span: clause.span,
                });
            }
            if !obls.is_empty() {
                let key = mangle_method(&class.node.name.node, &m.node.name.node);
                method_obls.insert(key, obls);
            }
        }
    }
    if method_obls.is_empty() {
        return Ok(());
    }

    // Pre-scan providing methods for the dedup fields their guards name:
    // every `self.F.contains(<key>)` / `self.F.insert(<key>)` where F is a
    // Set-typed own field and the argument is syntactically the claim's
    // key. These fields become globally insert-only.
    let mut registry: Registry = HashMap::new();
    for class in &program.classes {
        for m in &class.node.methods {
            let key = mangle_method(&class.node.name.node, &m.node.name.node);
            let Some(obls) = method_obls.get(&key) else { continue };
            let key_paths: HashSet<&str> =
                obls.iter().map(|o| o.key_path.as_str()).collect();
            let mut scan = DedupFieldScan {
                env,
                class: &class.node,
                key_paths: &key_paths,
                registry: &mut registry,
            };
            scan.visit_block(&m.node.body);
        }
    }

    // No armed chain can exist without a registered field; the per-method
    // analysis below reports the missing-guard diagnostic at the first
    // effect site, so an empty registry is not an error here.

    // Whole-program walk: obligations on providing methods, monotonicity
    // enforcement everywhere.
    for func in &program.functions {
        analyze_body(&func.node.name.node, &func.node, None, env, &registry, Vec::new())?;
    }
    for class in &program.classes {
        for m in &class.node.methods {
            let key = mangle_method(&class.node.name.node, &m.node.name.node);
            let obls = method_obls.get(&key).cloned().unwrap_or_default();
            analyze_body(&key, &m.node, Some(&class.node), env, &registry, obls)?;
        }
    }
    if let Some(app) = &program.app {
        for m in &app.node.methods {
            let key = mangle_method(&app.node.name.node, &m.node.name.node);
            analyze_body(&key, &m.node, None, env, &registry, Vec::new())?;
        }
    }
    for stage in &program.stages {
        for m in &stage.node.methods {
            let key = mangle_method(&stage.node.name.node, &m.node.name.node);
            analyze_body(&key, &m.node, None, env, &registry, Vec::new())?;
        }
    }
    Ok(())
}

/// Collects Set-typed own fields used in guard position against a claim
/// key inside a providing method.
struct DedupFieldScan<'a> {
    env: &'a TypeEnv,
    class: &'a ClassDecl,
    key_paths: &'a HashSet<&'a str>,
    registry: &'a mut Registry,
}

impl DedupFieldScan<'_> {
    fn syntactic_path(expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(n) => Some(n.clone()),
            Expr::FieldAccess { object, field } => match &object.node {
                Expr::Ident(root) => Some(format!("{root}.{}", field.node)),
                _ => None,
            },
            _ => None,
        }
    }
}

impl Visitor for DedupFieldScan<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::MethodCall { object, method, args, .. } = &expr.node {
            if (method.node == "contains" || method.node == "insert") && args.len() == 1 {
                if let Expr::FieldAccess { object: fobj, field } = &object.node {
                    if matches!(&fobj.node, Expr::Ident(s) if s == "self") {
                        let cname = &self.class.name.node;
                        let is_set_field = self
                            .env
                            .classes
                            .get(cname)
                            .and_then(|ci| ci.fields.iter().find(|(n, _, _)| *n == field.node))
                            .is_some_and(|(_, t, _)| matches!(t, PlutoType::Set(_)));
                        let arg_is_key = Self::syntactic_path(&args[0].node)
                            .is_some_and(|p| self.key_paths.contains(p.as_str()));
                        if is_set_field && arg_is_key {
                            self.registry
                                .entry(cname.clone())
                                .or_default()
                                .insert(field.node.clone());
                        }
                    }
                }
            }
        }
        walk_expr(self, expr);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// The body walker
// ─────────────────────────────────────────────────────────────────────────────

struct Analyzer<'a> {
    env: &'a TypeEnv,
    /// Resolution key for method-call lookups (`Class$method` or fn name).
    current_fn: String,
    /// `Some(class)` when analyzing a method body (`self`'s class).
    owner_class: Option<String>,
    registry: &'a Registry,
    obligations: Vec<Obligation>,
    facts: FactEnv,
    /// Scoped best-effort types of locals and parameters.
    vars: Vec<HashMap<String, PlutoType>>,
    /// Statements after a terminator: never executed, so effects there owe
    /// nothing (misuse of dedup fields is still reported).
    in_unreachable: bool,
    /// Inside a closure body: arming is disabled (the closure may run
    /// outside the serialized window) and effect diagnostics say so.
    in_closure: bool,
    error: Option<CompileError>,
}

fn analyze_body(
    current_fn: &str,
    func: &Function,
    owner: Option<&ClassDecl>,
    env: &TypeEnv,
    registry: &Registry,
    obligations: Vec<Obligation>,
) -> Result<(), CompileError> {
    let mut a = Analyzer {
        env,
        current_fn: current_fn.to_string(),
        owner_class: owner.map(|c| c.name.node.clone()),
        registry,
        obligations,
        facts: FactEnv::new(),
        vars: vec![HashMap::new()],
        in_unreachable: false,
        in_closure: false,
        error: None,
    };
    for p in &func.params {
        if p.name.node == "self" {
            if let Some(c) = &a.owner_class {
                a.define("self", PlutoType::Class(c.clone()));
            }
            continue;
        }
        if let Some(ty) = a.type_of_type_expr(&p.ty.node) {
            a.define(&p.name.node, ty);
        }
    }
    a.walk_block(&func.body.node);
    match a.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

impl<'a> Analyzer<'a> {
    // ── Scopes and typing (mirrors dominance.rs) ─────────────────────────

    fn define(&mut self, name: &str, ty: PlutoType) {
        self.vars
            .last_mut()
            .expect("var scope stack is never empty")
            .insert(name.to_string(), ty);
    }

    fn undefine(&mut self, name: &str) {
        for scope in self.vars.iter_mut().rev() {
            if scope.remove(name).is_some() {
                return;
            }
        }
    }

    fn lookup(&self, name: &str) -> Option<&PlutoType> {
        self.vars.iter().rev().find_map(|s| s.get(name))
    }

    fn push_scope(&mut self) {
        self.vars.push(HashMap::new());
        self.facts.push_frame();
    }

    fn pop_scope(&mut self) {
        self.vars.pop();
        self.facts.pop_frame();
    }

    fn type_of_type_expr(&self, te: &TypeExpr) -> Option<PlutoType> {
        match te {
            TypeExpr::Named(n) if n == "int" => Some(PlutoType::Int),
            TypeExpr::Named(n) if n == "string" => Some(PlutoType::String),
            TypeExpr::Named(n) if self.env.classes.contains_key(n) => {
                Some(PlutoType::Class(n.clone()))
            }
            _ => None,
        }
    }

    /// Best-effort type of an expression; `None` means untracked.
    fn type_of_expr(&self, expr: &Expr) -> Option<PlutoType> {
        match expr {
            Expr::Ident(name) => self.lookup(name).cloned(),
            Expr::IntLit(_) => Some(PlutoType::Int),
            Expr::StringLit(_) => Some(PlutoType::String),
            Expr::StructLit { name, type_args, .. } if type_args.is_empty() => {
                Some(PlutoType::Class(name.node.clone()))
            }
            Expr::FieldAccess { object, field } => {
                let PlutoType::Class(c) = self.type_of_expr(&object.node)? else {
                    return None;
                };
                let info = self.env.classes.get(&c)?;
                info.fields
                    .iter()
                    .find(|(n, _, _)| *n == field.node)
                    .map(|(_, t, _)| t.clone())
            }
            Expr::MethodCall { method, .. } => {
                let key = (self.current_fn.clone(), method.span.start);
                match self.env.method_resolutions.get(&key)? {
                    MethodResolution::Class { mangled_name } => self
                        .env
                        .functions
                        .get(mangled_name)
                        .map(|s| s.return_type.clone()),
                    _ => None,
                }
            }
            Expr::Call { name, .. } => self
                .env
                .functions
                .get(&name.node)
                .map(|s| s.return_type.clone()),
            Expr::Cast { target_type, .. } => self.type_of_type_expr(&target_type.node),
            Expr::Propagate { expr } | Expr::Catch { expr, .. } => self.type_of_expr(&expr.node),
            _ => None,
        }
    }

    /// Resolve an expression to a typed, trackable path (`self` allowed;
    /// non-self entity/remote/domain roots excluded — their fields can
    /// change concurrently, so facts about them are never sound).
    fn path_of(&self, expr: &Expr) -> Option<(String, PlutoType)> {
        match expr {
            Expr::Ident(name) => {
                let ty = self.lookup(name)?;
                if let PlutoType::Class(c) = ty {
                    if name != "self"
                        && (self.env.object_types.contains(c)
                            || self.env.remote_types.contains(c)
                            || self.env.domain_types.contains(c))
                    {
                        return None;
                    }
                }
                Some((name.clone(), ty.clone()))
            }
            Expr::FieldAccess { object, field } => {
                let (opath, oty) = self.path_of(&object.node)?;
                let PlutoType::Class(c) = oty else { return None };
                let info = self.env.classes.get(&c)?;
                let (_, fty, _) = info.fields.iter().find(|(n, _, _)| *n == field.node)?;
                Some((format!("{opath}.{}", field.node), fty.clone()))
            }
            _ => None,
        }
    }

    fn is_builtin_method(&self, method: &Spanned<String>) -> bool {
        let key = (self.current_fn.clone(), method.span.start);
        matches!(
            self.env.method_resolutions.get(&key),
            Some(MethodResolution::Builtin)
                | Some(MethodResolution::TaskDetach)
                | Some(MethodResolution::TaskCancel)
        )
    }

    // ── Call safety (fact kills — mirrors dominance.rs) ──────────────────

    /// Does this expression contain a call that may run user code? Builtin
    /// collection methods and provably class-free builtin free calls are
    /// exempt (see `facts::CallSeverity`); closure bodies are not scanned
    /// (creation runs nothing).
    fn has_unsafe_call(&self, expr: &Expr) -> bool {
        match expr {
            Expr::MethodCall { object, method, args, .. } => {
                if !self.is_builtin_method(method) {
                    return true;
                }
                self.has_unsafe_call(&object.node)
                    || args.iter().any(|a| self.has_unsafe_call(&a.node))
            }
            Expr::Call { name, args, .. } => {
                let leaf = |e: &Expr| self.path_of(e).map(|(_, t)| t);
                if super::facts::free_call_severity(&name.node, args, self.env, None, &leaf)
                    == super::facts::CallSeverity::All
                {
                    return true;
                }
                args.iter().any(|a| self.has_unsafe_call(&a.node))
            }
            Expr::StaticTraitCall { .. } | Expr::At { .. } | Expr::Spawn { .. } => true,
            Expr::Closure { .. } | Expr::ClosureCreate { .. } => false,
            Expr::BinOp { lhs, rhs, .. } | Expr::NullCoalesce { lhs, rhs } => {
                self.has_unsafe_call(&lhs.node) || self.has_unsafe_call(&rhs.node)
            }
            Expr::CompareChain { operands, .. } => {
                operands.iter().any(|o| self.has_unsafe_call(&o.node))
            }
            Expr::UnaryOp { operand, .. } => self.has_unsafe_call(&operand.node),
            Expr::FieldAccess { object, .. } => self.has_unsafe_call(&object.node),
            Expr::Index { object, index } => {
                self.has_unsafe_call(&object.node) || self.has_unsafe_call(&index.node)
            }
            Expr::StructLit { fields, .. } | Expr::EnumData { fields, .. } => {
                fields.iter().any(|(_, v)| self.has_unsafe_call(&v.node))
            }
            Expr::ArrayLit { elements, .. } | Expr::SetLit { elements, .. } => {
                elements.iter().any(|e| self.has_unsafe_call(&e.node))
            }
            Expr::MapLit { entries, .. } => entries
                .iter()
                .any(|(k, v)| self.has_unsafe_call(&k.node) || self.has_unsafe_call(&v.node)),
            Expr::StringInterp { parts } => parts.iter().any(|p| match p {
                crate::parser::ast::StringInterpPart::Expr(e) => self.has_unsafe_call(&e.node),
                crate::parser::ast::StringInterpPart::Lit(_) => false,
            }),
            Expr::Propagate { expr }
            | Expr::NullPropagate { expr }
            | Expr::Cast { expr, .. } => self.has_unsafe_call(&expr.node),
            Expr::Catch { expr, handlers } => {
                self.has_unsafe_call(&expr.node)
                    || handlers.iter().any(|h| match h {
                        CatchHandler::Shorthand(e) => self.has_unsafe_call(&e.node),
                        CatchHandler::Wildcard { .. } | CatchHandler::Typed { .. } => true,
                    })
            }
            Expr::Range { start, end, .. } => {
                self.has_unsafe_call(&start.node) || self.has_unsafe_call(&end.node)
            }
            Expr::If { condition, .. } => self.has_unsafe_call(&condition.node),
            Expr::Match { expr, arms } => {
                self.has_unsafe_call(&expr.node)
                    || arms.iter().any(|arm| self.has_unsafe_call(&arm.value.node))
            }
            Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::BoolLit(_)
            | Expr::StringLit(_)
            | Expr::Ident(_)
            | Expr::EnumUnit { .. }
            | Expr::NoneLit
            | Expr::QualifiedAccess { .. } => false,
        }
    }

    // ── Effects ──────────────────────────────────────────────────────────

    /// Does this expression contain an externally-visible effect (see
    /// module docs)? Nested statement blocks are NOT scanned — their
    /// statements carry their own effect obligations when walked. `exempt`
    /// is the span of a sanctioned dedup-insert call to skip, if any.
    fn contains_effect(&self, expr: &Spanned<Expr>, exempt: Option<Span>) -> bool {
        if exempt.is_some_and(|s| s == expr.span) {
            return false;
        }
        match &expr.node {
            Expr::MethodCall { object, method, args, .. } => {
                if !self.is_builtin_method(method) {
                    return true;
                }
                if MUTATING_BUILTIN_METHODS.contains(&method.node.as_str()) {
                    return true;
                }
                self.contains_effect(object, exempt)
                    || args.iter().any(|a| self.contains_effect(a, exempt))
            }
            Expr::Call { name, args, .. } => {
                // Any free call is an effect unless it is a known
                // effect-free builtin applied to effect-free arguments.
                let builtin_pure = self.env.builtins.contains(&name.node)
                    && EFFECT_FREE_BUILTIN_FNS.contains(&name.node.as_str())
                    && self.lookup(&name.node).is_none();
                if !builtin_pure {
                    return true;
                }
                args.iter().any(|a| self.contains_effect(a, exempt))
            }
            Expr::StaticTraitCall { .. } | Expr::At { .. } | Expr::Spawn { .. } => true,
            // Creation runs nothing; the body's statements are checked
            // under the closure barrier when walked.
            Expr::Closure { .. } | Expr::ClosureCreate { .. } => false,
            Expr::BinOp { lhs, rhs, .. } | Expr::NullCoalesce { lhs, rhs } => {
                self.contains_effect(lhs, exempt) || self.contains_effect(rhs, exempt)
            }
            Expr::CompareChain { operands, .. } => {
                operands.iter().any(|o| self.contains_effect(o, exempt))
            }
            Expr::UnaryOp { operand, .. } => self.contains_effect(operand, exempt),
            Expr::FieldAccess { object, .. } => self.contains_effect(object, exempt),
            Expr::Index { object, index } => {
                self.contains_effect(object, exempt) || self.contains_effect(index, exempt)
            }
            Expr::StructLit { fields, .. } | Expr::EnumData { fields, .. } => {
                fields.iter().any(|(_, v)| self.contains_effect(v, exempt))
            }
            Expr::ArrayLit { elements, .. } | Expr::SetLit { elements, .. } => {
                elements.iter().any(|e| self.contains_effect(e, exempt))
            }
            Expr::MapLit { entries, .. } => entries
                .iter()
                .any(|(k, v)| self.contains_effect(k, exempt) || self.contains_effect(v, exempt)),
            Expr::StringInterp { parts } => parts.iter().any(|p| match p {
                crate::parser::ast::StringInterpPart::Expr(e) => self.contains_effect(e, exempt),
                crate::parser::ast::StringInterpPart::Lit(_) => false,
            }),
            Expr::Propagate { expr }
            | Expr::NullPropagate { expr }
            | Expr::Cast { expr, .. } => self.contains_effect(expr, exempt),
            Expr::Catch { expr, handlers } => {
                self.contains_effect(expr, exempt)
                    || handlers.iter().any(|h| match h {
                        CatchHandler::Shorthand(e) => self.contains_effect(e, exempt),
                        // Handler blocks are statement blocks: walked (and
                        // effect-checked) statement by statement.
                        CatchHandler::Wildcard { .. } | CatchHandler::Typed { .. } => false,
                    })
            }
            Expr::Range { start, end, .. } => {
                self.contains_effect(start, exempt) || self.contains_effect(end, exempt)
            }
            // Branch blocks are walked as statements; only the scrutinee /
            // condition runs unconditionally here.
            Expr::If { condition, .. } => self.contains_effect(condition, exempt),
            Expr::Match { expr, arms } => {
                self.contains_effect(expr, exempt)
                    || arms.iter().any(|arm| self.contains_effect(&arm.value, exempt))
            }
            Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::BoolLit(_)
            | Expr::StringLit(_)
            | Expr::Ident(_)
            | Expr::EnumUnit { .. }
            | Expr::NoneLit
            | Expr::QualifiedAccess { .. } => false,
        }
    }

    /// The obligation at an effect site: an armed insert of every claim's
    /// key must be live on this path.
    fn require_licensed(&mut self, span: Span) {
        if self.error.is_some() || self.in_unreachable || self.obligations.is_empty() {
            return;
        }
        let owner = self.owner_class.clone().unwrap_or_default();
        let unlicensed: Vec<Obligation> = self
            .obligations
            .iter()
            .filter(|o| !self.facts.set_inserted_for_key(&o.key_path))
            .cloned()
            .collect();
        let Some(ob) = unlicensed.first() else { return };
        let short = ob
            .property
            .rsplit_once('.')
            .map(|(_, s)| s)
            .unwrap_or(&ob.property);
        let set_fields: Vec<String> = self
            .env
            .classes
            .get(&owner)
            .map(|ci| {
                ci.fields
                    .iter()
                    .filter(|(_, t, _)| matches!(t, PlutoType::Set(_)))
                    .map(|(n, _, _)| n.clone())
                    .collect()
            })
            .unwrap_or_default();
        let field_note = if set_fields.is_empty() {
            format!(
                "'{owner}' has no Set-typed field to key the dedup on — add one (e.g. \
                 'seen: Set<string>')"
            )
        } else {
            format!(
                "dedup candidates on '{owner}': '{}'",
                set_fields.join("', '")
            )
        };
        let closure_note = if self.in_closure {
            "\nthis effect is inside a closure body: a closure may escape the method and \
             run outside its serialized dedup window, so the method's guard can never \
             cover it — perform the effect in the method body itself"
        } else {
            ""
        };
        self.error = Some(CompileError::type_err(
            format!(
                "cannot discharge 'provides {short}': this statement is an \
                 externally-visible effect that is not covered by the dedup guard. Every \
                 effect in the providing method must be preceded, on every path, by an \
                 ARMED insert of the key — 'self.<seen>.insert({key})' dominated by a \
                 membership check ('if self.<seen>.contains({key}) {{ return ... }}') — \
                 with nothing in between that could invalidate it (calls that may run \
                 user code, loop boundaries, writes to the key). {field_note}{closure_note}{}",
                ob.blame(),
                key = ob.key_path,
            ),
            span,
        ));
    }

    // ── Membership facts from conditions ─────────────────────────────────

    /// Is `expr` exactly `self.F.contains(<key of some stable obligation>)`
    /// with F a registered dedup field? Returns the fact to assume on the
    /// FALSE branch.
    fn contains_check(&self, expr: &Expr) -> Option<Fact> {
        let Expr::MethodCall { object, method, args, .. } = expr else { return None };
        if method.node != "contains" || args.len() != 1 || !self.is_builtin_method(method) {
            return None;
        }
        let (set_path, set_ty) = self.path_of(&object.node)?;
        if !matches!(set_ty, PlutoType::Set(_)) {
            return None;
        }
        let owner = self.owner_class.as_deref()?;
        let field = set_path.strip_prefix("self.")?;
        if !self.registry.get(owner).is_some_and(|fs| fs.contains(field)) {
            return None;
        }
        let (arg_path, _) = self.path_of(&args[0].node)?;
        let ob = self
            .obligations
            .iter()
            .find(|o| o.key_path == arg_path && o.stable)?;
        Some(Fact::SetNotContains(set_path, ob.key_path.clone()))
    }

    /// Membership facts a condition establishes on each branch (the only
    /// shapes this proof needs: the bare check, its negation, and `&&`/`||`
    /// composition — mirroring `condition_facts_with`).
    fn membership_cond_facts(&self, cond: &Expr) -> (Vec<Fact>, Vec<Fact>) {
        use crate::parser::ast::{BinOp, UnaryOp};
        match cond {
            Expr::BinOp { op: BinOp::And, lhs, rhs } => {
                let (lt, _) = self.membership_cond_facts(&lhs.node);
                let (rt, _) = self.membership_cond_facts(&rhs.node);
                ([lt, rt].concat(), Vec::new())
            }
            Expr::BinOp { op: BinOp::Or, lhs, rhs } => {
                let (_, le) = self.membership_cond_facts(&lhs.node);
                let (_, re) = self.membership_cond_facts(&rhs.node);
                (Vec::new(), [le, re].concat())
            }
            Expr::UnaryOp { op: UnaryOp::Not, operand } => {
                let (t, e) = self.membership_cond_facts(&operand.node);
                (e, t)
            }
            _ => match self.contains_check(cond) {
                // `contains(key)` true ⇒ nothing we track; false ⇒ the key
                // was not in the set at the check.
                Some(fact) => (Vec::new(), vec![fact]),
                None => (Vec::new(), Vec::new()),
            },
        }
    }

    // ── Dedup-field misuse (conjunct 3: global monotonicity) ─────────────

    /// If `expr` is a field access reaching a registered dedup field,
    /// return `(class, field, through_self, typed)`; `typed == false` is
    /// the conservative by-name fallback for untypable receivers.
    fn dedup_field_access(&self, expr: &Expr) -> Option<(String, String, bool, bool)> {
        let Expr::FieldAccess { object, field } = expr else { return None };
        if let Expr::Ident(root) = &object.node {
            if root == "self" {
                let owner = self.owner_class.as_deref()?;
                if self.registry.get(owner).is_some_and(|fs| fs.contains(&field.node)) {
                    return Some((owner.to_string(), field.node.clone(), true, true));
                }
                return None;
            }
        }
        match self.type_of_expr(&object.node) {
            Some(PlutoType::Class(c)) => {
                if self.registry.get(&c).is_some_and(|fs| fs.contains(&field.node)) {
                    Some((c, field.node.clone(), false, true))
                } else {
                    None
                }
            }
            Some(_) => None,
            None => self
                .registry
                .iter()
                .find(|(_, fs)| fs.contains(&field.node))
                .map(|(c, _)| (c.clone(), field.node.clone(), false, false)),
        }
    }

    fn misuse_error(&mut self, msg: String, span: Span) {
        if self.error.is_none() {
            self.error = Some(CompileError::type_err(msg, span));
        }
    }

    /// Enforce the closed write-set of registered dedup fields over an
    /// expression tree: mutation only via `self.F.insert(...)` inside the
    /// owning class, pure reads from anywhere, no bare-value use (aliasing
    /// would reopen the write-set). Nested statement blocks are scanned
    /// when walked.
    fn misuse_scan(&mut self, expr: &Spanned<Expr>) {
        if self.error.is_some() {
            return;
        }
        match &expr.node {
            Expr::MethodCall { object, method, args, .. } => {
                if let Some((class, fname, through_self, _typed)) =
                    self.dedup_field_access(&object.node)
                {
                    let m = method.node.as_str();
                    if PURE_SET_METHODS.contains(&m) {
                        // reads are fine from anywhere
                    } else if m == "insert" {
                        if !through_self {
                            self.misuse_error(
                                format!(
                                    "dedup field '{fname}' of class '{class}' may only be \
                                     inserted through 'self' inside '{class}''s own methods: \
                                     the idempotency proof's write-set must stay closed under \
                                     the class's own control flow"
                                ),
                                expr.span,
                            );
                        }
                    } else {
                        self.misuse_error(
                            format!(
                                "dedup field '{fname}' of class '{class}' is insert-only: \
                                 '{m}' could un-record processed keys and would break every \
                                 idempotency claim keyed on it (a dedup set is monotone — \
                                 keys are never removed)"
                            ),
                            expr.span,
                        );
                    }
                    // The sanctioned receiver path itself is not a bare use;
                    // scan beneath its root and the arguments.
                    if let Expr::FieldAccess { object: fobj, .. } = &object.node {
                        self.misuse_scan(fobj);
                    }
                } else {
                    self.misuse_scan(object);
                }
                for a in args {
                    self.misuse_scan(a);
                }
            }
            Expr::FieldAccess { .. } => {
                if let Some((class, fname, _, _)) = self.dedup_field_access(&expr.node) {
                    self.misuse_error(
                        format!(
                            "dedup field '{fname}' of class '{class}' may not be used as a \
                             value (aliased, passed, returned, or iterated): an alias could \
                             mutate it outside the closed write-set the idempotency proof \
                             depends on. Read it through '.contains(...)' / '.len()', and \
                             mutate it only via 'self.{fname}.insert(...)'"
                        ),
                        expr.span,
                    );
                    return;
                }
                if let Expr::FieldAccess { object, .. } = &expr.node {
                    self.misuse_scan(object);
                }
            }
            Expr::StructLit { name, fields, .. } => {
                if let Some(reg_fields) = self.registry.get(&name.node) {
                    for (fname, value) in fields {
                        if reg_fields.contains(&fname.node)
                            && !matches!(value.node, Expr::SetLit { .. })
                        {
                            self.misuse_error(
                                format!(
                                    "dedup field '{}' of class '{}' must be initialized with \
                                     a fresh set literal (e.g. 'Set<string> {{}}'): sharing a \
                                     pre-existing set would alias the dedup state the \
                                     idempotency proof depends on",
                                    fname.node, name.node
                                ),
                                value.span,
                            );
                        }
                    }
                }
                for (_, v) in fields {
                    self.misuse_scan(v);
                }
            }
            Expr::BinOp { lhs, rhs, .. } | Expr::NullCoalesce { lhs, rhs } => {
                self.misuse_scan(lhs);
                self.misuse_scan(rhs);
            }
            Expr::CompareChain { operands, .. } => {
                for operand in operands {
                    self.misuse_scan(operand);
                }
            }
            Expr::UnaryOp { operand, .. } => self.misuse_scan(operand),
            Expr::Index { object, index } => {
                self.misuse_scan(object);
                self.misuse_scan(index);
            }
            Expr::Call { args, .. }
            | Expr::StaticTraitCall { args, .. }
            | Expr::At { args, .. } => {
                for a in args {
                    self.misuse_scan(a);
                }
            }
            Expr::Spawn { call } => self.misuse_scan(call),
            Expr::EnumData { fields, .. } => {
                for (_, v) in fields {
                    self.misuse_scan(v);
                }
            }
            Expr::ArrayLit { elements, .. } | Expr::SetLit { elements, .. } => {
                for e in elements {
                    self.misuse_scan(e);
                }
            }
            Expr::MapLit { entries, .. } => {
                for (k, v) in entries {
                    self.misuse_scan(k);
                    self.misuse_scan(v);
                }
            }
            Expr::StringInterp { parts } => {
                for p in parts {
                    if let crate::parser::ast::StringInterpPart::Expr(e) = p {
                        self.misuse_scan(e);
                    }
                }
            }
            Expr::Propagate { expr }
            | Expr::NullPropagate { expr }
            | Expr::Cast { expr, .. } => self.misuse_scan(expr),
            Expr::Catch { expr, handlers } => {
                self.misuse_scan(expr);
                for h in handlers {
                    if let CatchHandler::Shorthand(e) = h {
                        self.misuse_scan(e);
                    }
                    // Block handlers are statement blocks: scanned when walked.
                }
            }
            Expr::Range { start, end, .. } => {
                self.misuse_scan(start);
                self.misuse_scan(end);
            }
            Expr::If { condition, .. } => self.misuse_scan(condition),
            Expr::Match { expr, arms } => {
                self.misuse_scan(expr);
                for arm in arms {
                    self.misuse_scan(&arm.value);
                }
            }
            // Closure bodies are statement blocks: scanned when walked.
            Expr::Closure { .. }
            | Expr::ClosureCreate { .. }
            | Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::BoolLit(_)
            | Expr::StringLit(_)
            | Expr::Ident(_)
            | Expr::EnumUnit { .. }
            | Expr::NoneLit
            | Expr::QualifiedAccess { .. } => {}
        }
    }

    // ── Key stability (conjunct 2) ───────────────────────────────────────

    /// The named local was (re)bound: a key rooted there no longer denotes
    /// its entry value.
    fn note_name_bound(&mut self, name: &str) {
        for ob in &mut self.obligations {
            if ob.key_root == name {
                ob.stable = false;
            }
        }
    }

    /// Something that could mutate reachable fields happened (a call that
    /// may run user code, a field/index write): dotted keys no longer
    /// provably denote their entry value.
    fn note_field_mutation(&mut self) {
        for ob in &mut self.obligations {
            if ob.key_dotted {
                ob.stable = false;
            }
        }
    }

    // ── Kills ────────────────────────────────────────────────────────────

    fn kill_path(&mut self, root: &str) {
        self.facts.kill_path(root);
    }

    fn kill_fields(&mut self) {
        self.facts.kill_fields();
    }

    fn havoc(&mut self) {
        self.facts.havoc_all();
    }

    /// Kill rule for `root.field = v` (mirrors dominance.rs): the written
    /// path dies, and so does every fact rooted at another variable of the
    /// same class (a potential alias).
    fn kill_field_write(&mut self, root: &str, class: &str, field: &str) {
        self.kill_path(&format!("{root}.{field}"));
        let same_class: Vec<String> = self
            .vars
            .iter()
            .flat_map(|s| s.iter())
            .filter(|(n, t)| {
                n.as_str() != root && matches!(t, PlutoType::Class(c) if c == class)
            })
            .map(|(n, _)| n.clone())
            .collect();
        for v in same_class {
            self.kill_path(&v);
        }
    }

    // ── Expression walking ───────────────────────────────────────────────

    /// Process one evaluated expression: apply its call kills and walk any
    /// statement-bearing sub-structures.
    fn scan_expr(&mut self, expr: &Spanned<Expr>) {
        if self.error.is_some() {
            return;
        }
        if self.has_unsafe_call(&expr.node) {
            self.kill_fields();
            self.note_field_mutation();
        }
        self.scan_nested(&expr.node);
    }

    fn scan_nested(&mut self, expr: &Expr) {
        if self.error.is_some() {
            return;
        }
        match expr {
            Expr::If { condition, then_block, else_block } => {
                self.scan_nested(&condition.node);
                self.handle_if(condition, &then_block.node, Some(&else_block.node));
            }
            Expr::Match { expr, arms } => {
                self.scan_nested(&expr.node);
                for arm in arms {
                    self.push_scope();
                    self.bind_pattern(&arm.pattern);
                    self.scan_nested(&arm.value.node);
                    self.pop_scope();
                }
            }
            Expr::Closure { params, body, .. } => {
                // Fact barrier: the closure runs later; outer facts are not
                // valid inside, and arming is disabled (the dedup window
                // does not extend into escaped code).
                let saved_facts = std::mem::take(&mut self.facts);
                let saved_closure = self.in_closure;
                let saved_unreachable = self.in_unreachable;
                self.in_closure = true;
                self.in_unreachable = false;
                self.vars.push(HashMap::new());
                for p in params {
                    self.note_name_bound(&p.name.node);
                    if let Some(ty) = self.type_of_type_expr(&p.ty.node) {
                        self.define(&p.name.node, ty);
                    }
                }
                self.walk_block(&body.node);
                self.vars.pop();
                self.facts = saved_facts;
                self.in_closure = saved_closure;
                self.in_unreachable = saved_unreachable;
            }
            Expr::Catch { expr, handlers } => {
                self.scan_nested(&expr.node);
                for h in handlers {
                    match h {
                        CatchHandler::Wildcard { var, body }
                        | CatchHandler::Typed { var, body, .. } => {
                            self.push_scope();
                            self.note_name_bound(&var.node);
                            self.define(&var.node, PlutoType::Error);
                            self.walk_block(&body.node);
                            self.pop_scope();
                        }
                        CatchHandler::Shorthand(e) => self.scan_nested(&e.node),
                    }
                }
            }
            Expr::BinOp { lhs, rhs, .. } | Expr::NullCoalesce { lhs, rhs } => {
                self.scan_nested(&lhs.node);
                self.scan_nested(&rhs.node);
            }
            Expr::CompareChain { operands, .. } => {
                for operand in operands {
                    self.scan_nested(&operand.node);
                }
            }
            Expr::UnaryOp { operand, .. } => self.scan_nested(&operand.node),
            Expr::FieldAccess { object, .. } => self.scan_nested(&object.node),
            Expr::Index { object, index } => {
                self.scan_nested(&object.node);
                self.scan_nested(&index.node);
            }
            Expr::Call { args, .. } => {
                for a in args {
                    self.scan_nested(&a.node);
                }
            }
            Expr::MethodCall { object, args, .. } => {
                self.scan_nested(&object.node);
                for a in args {
                    self.scan_nested(&a.node);
                }
            }
            Expr::StaticTraitCall { args, .. } | Expr::At { args, .. } => {
                for a in args {
                    self.scan_nested(&a.node);
                }
            }
            Expr::Spawn { call } => self.scan_nested(&call.node),
            Expr::StructLit { fields, .. } | Expr::EnumData { fields, .. } => {
                for (_, v) in fields {
                    self.scan_nested(&v.node);
                }
            }
            Expr::ArrayLit { elements, .. } | Expr::SetLit { elements, .. } => {
                for e in elements {
                    self.scan_nested(&e.node);
                }
            }
            Expr::MapLit { entries, .. } => {
                for (k, v) in entries {
                    self.scan_nested(&k.node);
                    self.scan_nested(&v.node);
                }
            }
            Expr::StringInterp { parts } => {
                for p in parts {
                    if let crate::parser::ast::StringInterpPart::Expr(e) = p {
                        self.scan_nested(&e.node);
                    }
                }
            }
            Expr::Propagate { expr }
            | Expr::NullPropagate { expr }
            | Expr::Cast { expr, .. } => self.scan_nested(&expr.node),
            Expr::Range { start, end, .. } => {
                self.scan_nested(&start.node);
                self.scan_nested(&end.node);
            }
            Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::BoolLit(_)
            | Expr::StringLit(_)
            | Expr::Ident(_)
            | Expr::EnumUnit { .. }
            | Expr::ClosureCreate { .. }
            | Expr::NoneLit
            | Expr::QualifiedAccess { .. } => {}
        }
    }

    fn bind_pattern(&mut self, pattern: &MatchPattern) {
        if let MatchPattern::Variant { bindings, .. } = pattern {
            for (name, _) in bindings {
                self.note_name_bound(&name.node);
                self.kill_path(&name.node);
                self.undefine(&name.node);
            }
        }
    }

    // ── Statements ───────────────────────────────────────────────────────

    /// Walk a block flow-sensitively; returns true when it always
    /// terminates.
    fn walk_block(&mut self, block: &Block) -> bool {
        let mut terminated = false;
        for stmt in &block.stmts {
            if self.error.is_some() {
                return terminated;
            }
            if terminated {
                // Unreachable: no effect obligations (the code never runs),
                // but dedup-field misuse is still rejected.
                let saved = self.in_unreachable;
                self.in_unreachable = true;
                self.walk_stmt(stmt);
                self.in_unreachable = saved;
                continue;
            }
            terminated = self.walk_stmt(stmt);
        }
        terminated
    }

    /// Is this statement exactly a sanctioned dedup-insert,
    /// `self.F.insert(<arg>)` with F registered for the owner? Returns
    /// `(set_path, key_arg_path_if_stable_obligation_key, insert_span)`.
    fn as_dedup_insert(&self, stmt: &Stmt) -> Option<(String, Option<String>, Span)> {
        let Stmt::Expr(e) = stmt else { return None };
        let Expr::MethodCall { object, method, args, .. } = &e.node else { return None };
        if method.node != "insert" || args.len() != 1 || !self.is_builtin_method(method) {
            return None;
        }
        let (set_path, set_ty) = self.path_of(&object.node)?;
        if !matches!(set_ty, PlutoType::Set(_)) {
            return None;
        }
        let owner = self.owner_class.as_deref()?;
        let field = set_path.strip_prefix("self.")?;
        if !self.registry.get(owner).is_some_and(|fs| fs.contains(field)) {
            return None;
        }
        let key_arg = self.path_of(&args[0].node).and_then(|(p, _)| {
            self.obligations
                .iter()
                .any(|o| o.key_path == p && o.stable)
                .then_some(p)
        });
        Some((set_path, key_arg, e.span))
    }

    /// Returns true when the statement always terminates the block.
    fn walk_stmt(&mut self, stmt: &Spanned<Stmt>) -> bool {
        if self.error.is_some() {
            return false;
        }

        // Conjunct 3 first: dedup-field misuse anywhere in this statement's
        // expressions (nested blocks re-enter walk_stmt).
        for e in immediate_exprs(&stmt.node) {
            self.misuse_scan(e);
        }
        if self.error.is_some() {
            return false;
        }

        // The sanctioned dedup insert: `self.F.insert(arg)` as a statement.
        // Armed when a not-contains fact for the (stable) key is live;
        // unarmed inserts are effects like any other mutation, except the
        // benign re-insert of an already-armed key (a set-level no-op).
        if let Some((set_path, key_arg, span)) = self.as_dedup_insert(&stmt.node) {
            if let Stmt::Expr(e) = &stmt.node {
                if let Expr::MethodCall { args, .. } = &e.node {
                    // Effects nested in the argument are not exempt.
                    if args.iter().any(|a| self.contains_effect(a, None)) {
                        self.require_licensed(span);
                    }
                    self.scan_expr(&args[0]);
                }
            }
            if self.error.is_some() {
                return false;
            }
            let armed =
                !self.in_closure && {
                    let k = key_arg.as_deref();
                    self.facts.apply_set_insert(&set_path, k)
                };
            if !armed {
                let benign = key_arg
                    .as_deref()
                    .is_some_and(|k| self.facts.set_inserted_for_key(k));
                if !benign {
                    self.require_licensed(span);
                }
            }
            return false;
        }

        // Conjunct 1: effect licensing, against the facts live at statement
        // entry (the effect runs before this statement's kills land).
        let effectful = match &stmt.node {
            Stmt::FieldAssign { .. }
            | Stmt::IndexAssign { .. }
            | Stmt::Yield { .. }
            | Stmt::Serve { .. }
            | Stmt::Select { .. }
            | Stmt::Scope { .. } => true,
            _ => immediate_exprs(&stmt.node)
                .iter()
                .any(|e| self.contains_effect(e, None)),
        };
        if effectful {
            self.require_licensed(stmt.span);
            if self.error.is_some() {
                return false;
            }
        }

        match &stmt.node {
            Stmt::Let { name, value, .. } => {
                self.scan_expr(value);
                self.note_name_bound(&name.node);
                self.kill_path(&name.node);
                self.bind_value(&name.node, value);
                false
            }
            Stmt::Assign { target, value } => {
                self.scan_expr(value);
                self.note_name_bound(&target.node);
                self.kill_path(&target.node);
                self.bind_value(&target.node, value);
                false
            }
            Stmt::FieldAssign { object, field, value } => {
                self.scan_expr(object);
                self.scan_expr(value);
                self.note_field_mutation();
                if matches!(&object.node, Expr::Ident(s) if s == "self") {
                    if let Some(owner) = self.owner_class.clone() {
                        self.kill_field_write("self", &owner, &field.node);
                    } else {
                        self.kill_fields();
                    }
                } else {
                    match self.type_of_expr(&object.node) {
                        Some(PlutoType::Class(class)) => match self.path_of(&object.node) {
                            Some((path, _)) => self.kill_field_write(&path, &class, &field.node),
                            None => self.kill_fields(),
                        },
                        _ => self.kill_fields(),
                    }
                }
                false
            }
            Stmt::IndexAssign { object, index, value } => {
                self.scan_expr(object);
                self.scan_expr(index);
                self.scan_expr(value);
                self.note_field_mutation();
                self.kill_fields();
                false
            }
            Stmt::If { condition, then_block, else_block } => {
                self.scan_expr(condition);
                self.handle_if(condition, &then_block.node, else_block.as_ref().map(|b| &b.node))
            }
            Stmt::While { condition, body } => {
                self.scan_expr(condition);
                self.havoc();
                self.push_scope();
                self.walk_block(&body.node);
                self.pop_scope();
                self.havoc();
                false
            }
            Stmt::For { var, iterable, body } => {
                self.scan_expr(iterable);
                self.havoc();
                self.push_scope();
                self.note_name_bound(&var.node);
                self.kill_path(&var.node);
                self.undefine(&var.node);
                self.walk_block(&body.node);
                self.pop_scope();
                self.havoc();
                false
            }
            Stmt::Match { expr, arms } => {
                self.scan_expr(expr);
                let mut all_terminate = !arms.is_empty();
                for arm in arms {
                    self.push_scope();
                    self.bind_pattern(&arm.pattern);
                    let t = self.walk_block(&arm.body.node);
                    self.pop_scope();
                    all_terminate &= t;
                }
                all_terminate
            }
            Stmt::Return(value) => {
                if let Some(v) = value {
                    self.scan_expr(v);
                }
                true
            }
            Stmt::Raise { fields, .. } => {
                for (_, v) in fields {
                    self.scan_expr(v);
                }
                true
            }
            Stmt::Break | Stmt::Continue => true,
            Stmt::Expr(e) => {
                self.scan_expr(e);
                false
            }
            Stmt::Assert { expr } => {
                if self.has_unsafe_call(&expr.node) {
                    self.kill_fields();
                    self.note_field_mutation();
                }
                self.scan_nested(&expr.node);
                false
            }
            Stmt::LetChan { sender, receiver, capacity, .. } => {
                if let Some(c) = capacity {
                    self.scan_expr(c);
                }
                self.note_name_bound(&sender.node);
                self.note_name_bound(&receiver.node);
                self.kill_path(&sender.node);
                self.kill_path(&receiver.node);
                self.undefine(&sender.node);
                self.undefine(&receiver.node);
                false
            }
            Stmt::Select { arms, default, after } => {
                self.havoc();
                self.note_field_mutation();
                for arm in arms {
                    self.push_scope();
                    self.walk_block(&arm.body.node);
                    self.pop_scope();
                }
                if let Some(d) = default {
                    self.push_scope();
                    self.walk_block(&d.node);
                    self.pop_scope();
                }
                if let Some(a) = after {
                    self.push_scope();
                    self.walk_block(&a.body.node);
                    self.pop_scope();
                }
                self.havoc();
                false
            }
            Stmt::Scope { seeds, body, .. } => {
                for s in seeds {
                    self.scan_expr(s);
                }
                self.havoc();
                self.note_field_mutation();
                self.push_scope();
                self.walk_block(&body.node);
                self.pop_scope();
                self.havoc();
                false
            }
            Stmt::Yield { value } => {
                self.scan_expr(value);
                self.kill_fields();
                self.note_field_mutation();
                false
            }
            Stmt::Serve { service, port } => {
                self.scan_expr(service);
                self.scan_expr(port);
                self.kill_fields();
                self.note_field_mutation();
                false
            }
        }
    }

    /// `let`/`=` binding: record the best-effort type.
    fn bind_value(&mut self, name: &str, value: &Spanned<Expr>) {
        self.undefine(name);
        if let Some(ty) = self.type_of_expr(&value.node) {
            self.define(name, ty);
        }
    }

    fn handle_if(
        &mut self,
        condition: &Spanned<Expr>,
        then_block: &Block,
        else_block: Option<&Block>,
    ) -> bool {
        let (then_facts, else_facts) =
            if self.has_unsafe_call(&condition.node) || self.in_closure {
                (Vec::new(), Vec::new())
            } else {
                self.membership_cond_facts(&condition.node)
            };
        let then_terminates = super::block_always_terminates(then_block);
        let else_terminates = else_block.is_some_and(super::block_always_terminates);
        let mark = self.facts.kill_mark();

        self.push_scope();
        for f in &then_facts {
            self.facts.assume(f.clone());
        }
        self.walk_block(then_block);
        self.pop_scope();

        if let Some(eb) = else_block {
            self.push_scope();
            for f in &else_facts {
                self.facts.assume(f.clone());
            }
            self.walk_block(eb);
            self.pop_scope();
        }

        // Guard narrowing: if exactly one path terminates, the surviving
        // path's facts hold for the rest of the enclosing block — unless a
        // branch killed their paths meanwhile.
        let surviving = if then_terminates && !else_terminates {
            Some(&else_facts)
        } else if else_terminates && !then_terminates {
            Some(&then_facts)
        } else {
            None
        };
        if let Some(facts) = surviving {
            for f in facts {
                if !self.facts.killed_since(mark, f) {
                    self.facts.assume(f.clone());
                }
            }
        }
        then_terminates && else_terminates
    }
}
