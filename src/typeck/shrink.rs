//! Error-set shrinking — phase 3 of the verification RFC
//! (docs/design/rfc-verification.md, "Facts have consequences").
//!
//! Error inference is site-sensitive; this pass makes it *fact*-sensitive:
//! when every direct `raise` of a variant inside a callee is dominated by a
//! guard the caller's flow facts refute, that variant is removed from the
//! *required-handling* set of that call site. If the set becomes empty the
//! call no longer needs `!`/`catch`. The callee's canonical inferred error
//! set (`env.fn_errors`, the global fixed point) is never changed — only
//! what enforcement demands at an individual site.
//!
//! # Scope (conservative by construction)
//!
//! - **Summaries cover direct raises only.** For each free function and
//!   class/object method — generic templates included — each `raise X`
//!   statement is summarized by its dominating guard chain (the `if`
//!   conditions on the path from entry, with polarity). Variants whose
//!   raises arrive via propagation (`!` edges, escaped closures, dynamic
//!   dispatch) never shrink — they are subtracted via
//!   `env.fn_propagated_errors`.
//! - **Generic callees summarize at the template.** A template's guard
//!   chains are syntactic and instantiation rewrites only type annotations,
//!   so one summary (under the template key: `id`, `Lease$renew`) holds for
//!   every instance; call sites on instance-mangled methods
//!   (`Lease$$Held$renew`) resolve back to it. The scope rule is *fragment
//!   survival*: a guard mentioning a type-param-dependent leaf (a param or
//!   `self` field whose declared type mentions a type param) is never
//!   usable. Guards over int params and int fields independent of the type
//!   params — the typestate pattern, whose state params are phantom — work
//!   exactly like concrete code. Generic *callers* still never shrink their
//!   own call sites (their bodies are checked under skolems, per
//!   instantiation facts don't exist).
//! - **Direct calls only.** Shrinking applies to calls of named free
//!   functions and method calls whose receiver is a trackable path of class
//!   (or object) type. Calls through fn-typed values, closures, trait
//!   objects, and `at` placement never shrink (no summary exists / the site
//!   is not evaluated).
//! - **Entry-value semantics.** A summary guard is only usable if, at its
//!   evaluation point, every path it mentions still holds its
//!   function-entry value: the extraction walker invalidates a guard once a
//!   call-like expression may have run (aliases could mutate the receiver)
//!   or a mentioned parameter / any `self` field was assigned. The one
//!   exempt call shape is `p.len()` on a parameter of declared collection
//!   type — a pure builtin read; at the call site it substitutes to the
//!   actual's length term, and guards over one-level parameter field paths
//!   (`p.field`, non-entity class params) substitute to the actual's field
//!   path, usable when the caller holds facts about it.
//! - **Per-raise-site granularity, loop-tolerant.** A variant drops at a
//!   call site iff *every* one of its raise sites is individually refuted.
//!   A loop-resident raise summarizes when its surviving guard chain is
//!   loop-invariant and entry-stable: the whole loop body's kills
//!   (assignments, field writes, call taint) are applied at loop entry —
//!   any iteration may rerun the body before a later iteration's raise —
//!   and guards over paths the loop never disturbs stay usable. Raises
//!   inside match arms, select/scope blocks, catch handlers, and
//!   expression-level blocks are still never summarized (control reaches
//!   them under conditions outside the decidable fragment).
//! - **Runtime raise sources poison their variants.** Direct raises that do
//!   not come from a `raise` statement — `select` without default
//!   (ChannelClosed), propagated `at` (NetworkError), channel ops, unknown
//!   task origins, fallible fn-typed values (all declared errors), fallible
//!   `pow` (MathError) — mark the affected variants unshrinkable. Sites
//!   whose nature is only known post-typecheck are recorded as *deferred*
//!   and resolved against `method_resolutions` / `fallible_value_calls` /
//!   `fallible_builtin_calls` at enforcement time.
//!
//! # Where the decision is made
//!
//! Three hooks, matching the existing architecture:
//!
//! 1. `extract_raise_summaries` — syntactic pre-pass, before any body is
//!    checked (summaries must exist before callers are visited, and bodies
//!    are visited in program order).
//! 2. `record_call_site_shrinking` — during body checking (`check_stmt`,
//!    *before* `apply_stmt_kills`), when the caller's flow facts are live:
//!    substitutes actual arguments / receiver fields into each summary
//!    guard and records variants whose every raise site is Refuted. Within
//!    a statement, calls are visited in evaluation (post-)order; once any
//!    call-like node has executed, field facts are dropped for the
//!    remaining calls of the same statement (a callee may have mutated any
//!    reachable object — the same aliasing rule `apply_stmt_kills` applies
//!    between statements).
//! 3. `required_errors` — at enforcement time (`errors.rs`), computes a
//!    site's required-handling set: the callee's full inferred set minus
//!    the recorded refuted variants, re-filtered against propagation and
//!    deferred poisons (which are only known after inference).
//!
//! # Ergonomics
//!
//! Handling a provably-impossible error stays legal: the `catch applied to
//! infallible` / `'!' applied to infallible` checks keep using the full
//! inferred set, so a site that shrank to empty still accepts `catch`/`!`.
//! No warning is emitted for the dead handler — consistent with the
//! existing treatment of redundant `?` on a flow-narrowed nullable, which
//! is silently tolerated.

use std::collections::{HashMap, HashSet};

use crate::parser::ast::{Block, ClassDecl, Expr, Function, Program, Stmt, TypeExpr};
use crate::span::Spanned;
use crate::visit::{walk_expr, walk_stmt, Visitor};

use super::env::{mangle_method, mangle_name, InstKind, MethodResolution, TypeEnv};
use super::facts::{
    contains_call, eval_condition_with, immediate_exprs, len_path, to_affine, typed_path, Affine,
    FactEnv, Verdict,
};
use super::types::PlutoType;

// ─────────────────────────────────────────────────────────────────────────────
// Summary data
// ─────────────────────────────────────────────────────────────────────────────

/// One `raise` site's dominating guard conjunction. The raise can only
/// execute if every entry evaluates to its polarity (`negated: false` ⇒ the
/// condition was true, `true` ⇒ false), over the callee's *entry* values of
/// its parameters and the receiver's direct fields.
#[derive(Debug, Clone)]
pub struct GuardChain {
    pub conds: Vec<(Expr, bool)>,
}

/// How a variant's direct raises inside a function are guarded.
#[derive(Debug, Clone)]
pub enum VariantRaises {
    /// Every direct raise of the variant has a usable guard chain (one entry
    /// per raise site). The variant may shrink at a site that refutes all of
    /// them.
    Guarded(Vec<GuardChain>),
    /// At least one direct raise source is unguarded, inexpressible, or
    /// reached only after possible mutation. The variant never shrinks.
    Unshrinkable,
}

/// A call-like site inside the callee whose direct-raise contribution is
/// resolution-dependent and only known after type checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredKind {
    /// `f(...)!` — fallible fn-typed value (all declared errors), fallible
    /// `pow` (MathError), closure/named-fn edge (propagation; no poison).
    Call,
    /// `x.m(...)!` — channel ops, unknown task origin, remote boundary, or
    /// a plain class/trait edge (propagation; no poison).
    Method,
}

/// Raise summary of one function body.
#[derive(Debug, Clone, Default)]
pub struct FnRaiseSummary {
    /// Parameter names in declaration order (including `self` for methods).
    pub params: Vec<String>,
    /// Per-variant guard information for direct `raise` statements.
    pub variants: HashMap<String, VariantRaises>,
    /// Deferred poison sites, keyed by the span start that
    /// `method_resolutions` / `fallible_value_calls` /
    /// `fallible_builtin_calls` use ((resolution_fn, span start)).
    pub deferred: Vec<(DeferredKind, usize)>,
    /// The `current_fn` key under which typecheck recorded this body's
    /// span-keyed records (method resolutions, fallible value/builtin
    /// calls). Equal to the summary key for non-generic functions and
    /// methods; for generic class template methods it is the skolem-check
    /// instance key (`Lease$$%S$describe`, or the state-bound
    /// `Lease$$Held$renew` for `where`-constrained methods — mirrors
    /// templates.rs::check_class_template).
    pub resolution_fn: String,
}

impl FnRaiseSummary {
    fn poison(&mut self, variant: &str) {
        self.variants
            .insert(variant.to_string(), VariantRaises::Unshrinkable);
    }

    fn add_guarded(&mut self, variant: &str, chain: GuardChain) {
        match self
            .variants
            .entry(variant.to_string())
            .or_insert_with(|| VariantRaises::Guarded(Vec::new()))
        {
            VariantRaises::Guarded(chains) => chains.push(chain),
            VariantRaises::Unshrinkable => {}
        }
    }

    /// Does any variant have a usable guard? (Fast path for callers.)
    fn has_guarded(&self) -> bool {
        self.variants
            .values()
            .any(|v| matches!(v, VariantRaises::Guarded(c) if !c.is_empty()))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Extraction (syntactic pre-pass)
// ─────────────────────────────────────────────────────────────────────────────

/// Build raise summaries for every free function and every class/object
/// method, generic templates included (app and stage methods are skipped —
/// their calls never shrink in slice 1). Purely syntactic; runs before body
/// checking so summaries exist for callers visited in any order.
///
/// Generic templates are summarized once, under their template key (`id`,
/// `Lease$renew`): the guard chains are syntactic and `substitute_in_function`
/// rewrites only type annotations, so a template's chains hold verbatim for
/// every instantiation. The one scoping rule is *fragment survival*: a guard
/// that mentions a type-param-dependent leaf (a parameter whose declared type
/// mentions a type param, or a `self` field whose declared type does) is
/// never usable — its value vocabulary changes per instantiation. Int fields
/// and int params independent of the type params — the common typestate
/// shape, where state params are phantom — summarize exactly like concrete
/// code.
pub(crate) fn extract_raise_summaries(program: &Program, env: &mut TypeEnv) {
    let mut out: HashMap<String, FnRaiseSummary> = HashMap::new();
    for func in &program.functions {
        let type_params: HashSet<String> =
            func.node.type_params.iter().map(|tp| tp.node.clone()).collect();
        let name = func.node.name.node.clone();
        let mut summary = summarize_fn(&func.node, &type_params, &HashSet::new());
        // Free-function templates are skolem-checked under their own name
        // (templates.rs::check_function_template), so the resolution key is
        // the summary key for concrete and generic functions alike.
        summary.resolution_fn = name.clone();
        out.insert(name, summary);
    }
    for class in &program.classes {
        summarize_class(&class.node, env, &mut out);
    }
    env.raise_summaries = out;
}

fn summarize_class(class: &ClassDecl, env: &TypeEnv, out: &mut HashMap<String, FnRaiseSummary>) {
    let type_params: HashSet<String> =
        class.type_params.iter().map(|tp| tp.node.clone()).collect();
    // Fields whose declared type mentions a class type param: guards over
    // them leave the surviving fragment vocabulary (their meaning changes
    // per instantiation), so they never validate a guard.
    let dependent_fields: HashSet<String> = class
        .fields
        .iter()
        .filter(|f| type_mentions_params(&f.ty.node, &type_params))
        .map(|f| f.name.node.clone())
        .collect();
    for method in &class.methods {
        let key = mangle_method(&class.name.node, &method.node.name.node);
        let mut summary = summarize_fn(&method.node, &type_params, &dependent_fields);
        summary.resolution_fn = if class.type_params.is_empty() {
            key.clone()
        } else {
            template_resolution_key(class, &method.node.name.node, env)
        };
        out.insert(key, summary);
    }
}

/// The `current_fn` key under which a generic-class template method's
/// span-keyed typecheck records (method resolutions, fallible value/builtin
/// calls) were made: the class instantiated at skolem args, except
/// `where`-constrained params, which were bound to their state types.
/// Mirrors templates.rs::check_class_template (and
/// linearity.rs::template_method_key).
fn template_resolution_key(class: &ClassDecl, method_name: &str, env: &TypeEnv) -> String {
    let type_params: Vec<String> = class.type_params.iter().map(|tp| tp.node.clone()).collect();
    let mut args: Vec<PlutoType> = type_params
        .iter()
        .map(|tp| PlutoType::Class(format!("%{tp}")))
        .collect();
    if let Some(gen_info) = env.generic_classes.get(&class.name.node)
        && let Some(cs) = gen_info.method_state_constraints.get(method_name)
    {
        for (param, state) in cs {
            if let Some(idx) = type_params.iter().position(|p| p == param) {
                args[idx] = if env.enums.contains_key(state) {
                    PlutoType::Enum(state.clone())
                } else {
                    PlutoType::Class(state.clone())
                };
            }
        }
    }
    mangle_method(&mangle_name(&class.name.node, &args), method_name)
}

/// Does this declared type mention any of the in-scope type params?
fn type_mentions_params(ty: &TypeExpr, params: &HashSet<String>) -> bool {
    if params.is_empty() {
        return false;
    }
    match ty {
        TypeExpr::Named(n) => params.contains(n),
        TypeExpr::Qualified { .. } => false,
        TypeExpr::Array(t) | TypeExpr::Nullable(t) | TypeExpr::Stream(t) => {
            type_mentions_params(&t.node, params)
        }
        TypeExpr::Fn {
            params: ps,
            return_type,
            ..
        } => {
            ps.iter().any(|p| type_mentions_params(&p.node, params))
                || type_mentions_params(&return_type.node, params)
        }
        TypeExpr::Generic { type_args, .. } => type_args
            .iter()
            .any(|a| type_mentions_params(&a.node, params)),
        // Should not appear in declared types; be conservative.
        TypeExpr::Infer => true,
    }
}

fn summarize_fn(
    func: &Function,
    type_params: &HashSet<String>,
    dependent_fields: &HashSet<String>,
) -> FnRaiseSummary {
    let mut summary = FnRaiseSummary {
        params: func.params.iter().map(|p| p.name.node.clone()).collect(),
        variants: HashMap::new(),
        deferred: Vec::new(),
        resolution_fn: String::new(),
    };
    let collections = collection_params(func);
    // Params whose declared type mentions a type param (`x: T`, `b: Box<T>`):
    // guards mentioning them never validate. `self` is judged per-field via
    // `dependent_fields`, not here (its own type always mentions the params).
    let dependent_params: HashSet<String> = func
        .params
        .iter()
        .filter(|p| p.name.node != "self" && type_mentions_params(&p.ty.node, type_params))
        .map(|p| p.name.node.clone())
        .collect();
    let mut b = SummaryBuilder {
        out: &mut summary,
        guards: Vec::new(),
        call_tainted: false,
        killed: HashSet::new(),
        fields_killed: false,
        poison_raises: false,
        collections,
        dependent_params,
        dependent_fields,
    };
    b.walk_block(&func.body.node);
    summary
}

/// Parameters whose *declared* type is a builtin collection (`[T]`,
/// `string`, `bytes`, `Map<..>`, `Set<..>`). A `.len()` call on one is a
/// pure read of builtin state — exempt from the call-taint rules during
/// extraction, so guards like `if p.len() == 0 { raise Empty }` stay
/// summarizable. Judged syntactically: extraction runs before typecheck.
fn collection_params(func: &Function) -> HashSet<String> {
    use crate::parser::ast::TypeExpr;
    func.params
        .iter()
        .filter(|p| match &p.ty.node {
            TypeExpr::Array(_) => true,
            TypeExpr::Named(n) => n == "string" || n == "bytes",
            TypeExpr::Generic { name, .. } => name == "Map" || name == "Set",
            _ => false,
        })
        .map(|p| p.name.node.clone())
        .collect()
}

/// Is this expression a `.len()` call on one of the given collection
/// parameters (`p.len()`)? The one call shape extraction treats as pure.
fn is_param_len(expr: &Expr, collections: &HashSet<String>) -> bool {
    let Expr::MethodCall { object, method, args, .. } = expr else {
        return false;
    };
    method.node == "len"
        && args.is_empty()
        && matches!(&object.node, Expr::Ident(p) if collections.contains(p))
}

/// Like `facts::contains_call`, but exempts `p.len()` on declared
/// collection params (see [`is_param_len`]). Purely syntactic — the
/// env-aware twin is `facts::contains_impure_call`, which callers with a
/// `TypeEnv` use instead.
fn contains_call_except_param_len(
    expr: &Spanned<Expr>,
    collections: &HashSet<String>,
) -> bool {
    struct Scan<'a> {
        collections: &'a HashSet<String>,
        found: bool,
    }
    impl Visitor for Scan<'_> {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if self.found {
                return;
            }
            match &expr.node {
                Expr::MethodCall { .. } if is_param_len(&expr.node, self.collections) => {
                    return;
                }
                Expr::Call { .. }
                | Expr::MethodCall { .. }
                | Expr::StaticTraitCall { .. }
                | Expr::At { .. }
                | Expr::Spawn { .. } => {
                    self.found = true;
                    return;
                }
                _ => {}
            }
            walk_expr(self, expr);
        }
    }
    let mut scan = Scan { collections, found: false };
    scan.visit_expr(expr);
    scan.found
}

/// A guard on the stack: condition, polarity for the branch being walked,
/// and whether its mentioned paths still held entry values at evaluation.
struct GuardAtEval {
    cond: Expr,
    valid: bool,
}

struct SummaryBuilder<'a> {
    out: &'a mut FnRaiseSummary,
    /// Stack of enclosing `if` guards, innermost last. The polarity for the
    /// branch currently being walked is pushed per-branch.
    guards: Vec<(GuardAtEval, bool)>,
    /// A call-like expression may have run: aliases could have mutated the
    /// receiver, so `self.f` reads no longer equal entry values.
    call_tainted: bool,
    /// Parameters (and locals) assigned so far.
    killed: HashSet<String>,
    /// Any field assignment happened (aliasing: kills all `self.*` reads).
    fields_killed: bool,
    /// Raises in this region are never summarized (match arms, select/scope
    /// bodies — regions whose reachability conditions leave the fragment).
    poison_raises: bool,
    /// Params of declared collection type: `.len()` on one is a pure read,
    /// exempt from call taint.
    collections: HashSet<String>,
    /// Params whose declared type mentions a type param: guards mentioning
    /// them are never usable (fragment vocabulary does not survive
    /// instantiation).
    dependent_params: HashSet<String>,
    /// Fields of the enclosing generic class whose declared type mentions a
    /// class type param: `self.field` guards over them are never usable.
    dependent_fields: &'a HashSet<String>,
}

impl SummaryBuilder<'_> {
    fn walk_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.walk_stmt(&stmt.node);
        }
    }

    /// Does `expr` mention a path whose entry value may be stale?
    fn mentions_killed(&self, expr: &Expr) -> bool {
        struct Scan<'a> {
            killed: &'a HashSet<String>,
            fields_killed: bool,
            found: bool,
        }
        impl Visitor for Scan<'_> {
            fn visit_expr(&mut self, expr: &Spanned<Expr>) {
                if self.found {
                    return;
                }
                match &expr.node {
                    Expr::Ident(n) => {
                        if self.killed.contains(n) {
                            self.found = true;
                        }
                    }
                    Expr::FieldAccess { .. } => {
                        // Guards only resolve `self.f` leaves, but any field
                        // read goes stale once a field was assigned.
                        if self.fields_killed {
                            self.found = true;
                        }
                    }
                    _ => {}
                }
                walk_expr(self, expr);
            }
        }
        let mut scan = Scan {
            killed: &self.killed,
            fields_killed: self.fields_killed,
            found: false,
        };
        scan.visit_expr(&Spanned::new(expr.clone(), crate::span::Span::dummy()));
        scan.found
    }

    /// Does `expr` mention a type-param-dependent leaf (a dependent param,
    /// or `self.field` on a dependent field)? Such guards never validate: a
    /// template summary must hold for every instantiation, and only leaves
    /// whose types are independent of the type params keep a stable meaning
    /// in the fact fragment.
    fn mentions_dependent(&self, expr: &Expr) -> bool {
        if self.dependent_params.is_empty() && self.dependent_fields.is_empty() {
            return false;
        }
        struct Scan<'a> {
            params: &'a HashSet<String>,
            fields: &'a HashSet<String>,
            found: bool,
        }
        impl Visitor for Scan<'_> {
            fn visit_expr(&mut self, expr: &Spanned<Expr>) {
                if self.found {
                    return;
                }
                match &expr.node {
                    Expr::Ident(n) if self.params.contains(n) => {
                        self.found = true;
                        return;
                    }
                    Expr::FieldAccess { object, field }
                        if matches!(&object.node, Expr::Ident(s) if s == "self")
                            && self.fields.contains(&field.node) =>
                    {
                        self.found = true;
                        return;
                    }
                    _ => {}
                }
                walk_expr(self, expr);
            }
        }
        let mut scan = Scan {
            params: &self.dependent_params,
            fields: self.dependent_fields,
            found: false,
        };
        scan.visit_expr(&Spanned::new(expr.clone(), crate::span::Span::dummy()));
        scan.found
    }

    /// Apply every kill a region may perform — at the region's *entry*. Used
    /// for loop bodies: any iteration may rerun the whole body before a
    /// later iteration's raise, so assignments, field writes, and call taint
    /// anywhere in the body invalidate guards from the first iteration on.
    /// Conservative by construction (closure subtrees included).
    fn apply_region_kills(&mut self, block: &Block) {
        struct Scan<'a> {
            collections: &'a HashSet<String>,
            killed: HashSet<String>,
            fields_killed: bool,
            call_tainted: bool,
        }
        impl Visitor for Scan<'_> {
            fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
                match &stmt.node {
                    Stmt::Let { name, .. } => {
                        self.killed.insert(name.node.clone());
                    }
                    Stmt::Assign { target, .. } => {
                        self.killed.insert(target.node.clone());
                    }
                    Stmt::For { var, .. } => {
                        self.killed.insert(var.node.clone());
                    }
                    Stmt::LetChan { sender, receiver, .. } => {
                        self.killed.insert(sender.node.clone());
                        self.killed.insert(receiver.node.clone());
                    }
                    Stmt::FieldAssign { .. } => self.fields_killed = true,
                    // Channel ops / DI construction / serve behave like calls.
                    Stmt::Select { .. } | Stmt::Scope { .. } | Stmt::Serve { .. } => {
                        self.call_tainted = true
                    }
                    _ => {}
                }
                walk_stmt(self, stmt);
            }
            fn visit_expr(&mut self, expr: &Spanned<Expr>) {
                match &expr.node {
                    Expr::MethodCall { .. } if is_param_len(&expr.node, self.collections) => {}
                    Expr::Call { .. }
                    | Expr::MethodCall { .. }
                    | Expr::StaticTraitCall { .. }
                    | Expr::At { .. }
                    | Expr::Spawn { .. } => self.call_tainted = true,
                    _ => {}
                }
                walk_expr(self, expr);
            }
        }
        let mut scan = Scan {
            collections: &self.collections,
            killed: HashSet::new(),
            fields_killed: false,
            call_tainted: false,
        };
        for stmt in &block.stmts {
            scan.visit_stmt(stmt);
        }
        self.killed.extend(scan.killed);
        self.fields_killed |= scan.fields_killed;
        self.call_tainted |= scan.call_tainted;
    }

    /// Scan an expression for deferred poison sites and raise-like content
    /// nested in expression-level blocks (always poisoned), and taint on
    /// call-like content.
    fn scan_expr(&mut self, expr: &Spanned<Expr>) {
        let mut scan = ExprScan { out: self.out };
        scan.visit_expr(expr);
        if contains_call_except_param_len(expr, &self.collections) {
            self.call_tainted = true;
        }
    }

    fn walk_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Raise {
                error_name, fields, ..
            } => {
                if self.poison_raises {
                    self.out.poison(&error_name.node);
                } else {
                    let conds: Vec<(Expr, bool)> = self
                        .guards
                        .iter()
                        .filter(|(g, _)| g.valid)
                        .map(|(g, negated)| (g.cond.clone(), *negated))
                        .collect();
                    if conds.is_empty() {
                        self.out.poison(&error_name.node);
                    } else {
                        self.out
                            .add_guarded(&error_name.node, GuardChain { conds });
                    }
                }
                for (_, e) in fields {
                    self.scan_expr(e);
                }
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                // Validity is judged at the condition's evaluation point:
                // before the branches run, after everything above.
                let valid = !self.call_tainted
                    && !self.mentions_killed(&condition.node)
                    && !contains_call_except_param_len(condition, &self.collections)
                    && !self.mentions_dependent(&condition.node);
                self.scan_expr(condition);
                self.guards.push((
                    GuardAtEval {
                        cond: condition.node.clone(),
                        valid,
                    },
                    false,
                ));
                self.walk_block(&then_block.node);
                self.guards.pop();
                if let Some(eb) = else_block {
                    self.guards.push((
                        GuardAtEval {
                            cond: condition.node.clone(),
                            valid,
                        },
                        true,
                    ));
                    self.walk_block(&eb.node);
                    self.guards.pop();
                }
            }
            // Loop-resident raises summarize when their surviving guard
            // chain is loop-invariant and entry-stable: the whole body's
            // kills (assignments, field writes, call taint) are applied at
            // loop ENTRY — any iteration may rerun the body before a later
            // iteration's raise — and then the body is walked normally.
            // Guards over paths the loop never disturbs stay usable; a raise
            // whose every conjunct is disturbed poisons as before. The loop
            // condition / iterable is never recorded as a guard (dropping
            // conjuncts only weakens the chain, which is sound).
            Stmt::While { condition, body } => {
                self.scan_expr(condition);
                self.apply_region_kills(&body.node);
                self.walk_block(&body.node);
            }
            Stmt::For {
                var,
                iterable,
                body,
            } => {
                self.scan_expr(iterable);
                self.killed.insert(var.node.clone());
                self.apply_region_kills(&body.node);
                self.walk_block(&body.node);
            }
            Stmt::Match { expr, arms } => {
                self.scan_expr(expr);
                let prev = self.poison_raises;
                self.poison_raises = true;
                for arm in arms {
                    self.walk_block(&arm.body.node);
                }
                self.poison_raises = prev;
            }
            Stmt::Select { arms, default } => {
                if default.is_none() {
                    // Select without default raises ChannelClosed directly.
                    self.out.poison("ChannelClosed");
                }
                // Channel operations behave like calls.
                self.call_tainted = true;
                let prev = self.poison_raises;
                self.poison_raises = true;
                for arm in arms {
                    match &arm.op {
                        crate::parser::ast::SelectOp::Recv { channel, .. } => {
                            self.scan_expr(channel)
                        }
                        crate::parser::ast::SelectOp::Send { channel, value } => {
                            self.scan_expr(channel);
                            self.scan_expr(value);
                        }
                    }
                    self.walk_block(&arm.body.node);
                }
                if let Some(def) = default {
                    self.walk_block(&def.node);
                }
                self.poison_raises = prev;
            }
            Stmt::Scope { seeds, body, .. } => {
                for seed in seeds {
                    self.scan_expr(seed);
                }
                // Scope blocks construct DI instances (calls, effectively).
                self.call_tainted = true;
                let prev = self.poison_raises;
                self.poison_raises = true;
                self.walk_block(&body.node);
                self.poison_raises = prev;
            }
            Stmt::Let { name, value, .. } => {
                self.scan_expr(value);
                self.killed.insert(name.node.clone());
            }
            Stmt::Assign { target, value } => {
                self.scan_expr(value);
                self.killed.insert(target.node.clone());
            }
            Stmt::FieldAssign { object, value, .. } => {
                self.scan_expr(object);
                self.scan_expr(value);
                self.fields_killed = true;
            }
            Stmt::IndexAssign {
                object,
                index,
                value,
            } => {
                self.scan_expr(object);
                self.scan_expr(index);
                self.scan_expr(value);
            }
            Stmt::Return(value) => {
                if let Some(v) = value {
                    self.scan_expr(v);
                }
            }
            Stmt::Yield { value } => self.scan_expr(value),
            Stmt::Assert { expr } => self.scan_expr(expr),
            Stmt::Expr(e) => self.scan_expr(e),
            Stmt::Serve { service, port } => {
                self.scan_expr(service);
                self.scan_expr(port);
                self.call_tainted = true;
            }
            Stmt::LetChan { capacity, .. } => {
                if let Some(c) = capacity {
                    self.scan_expr(c);
                }
            }
            Stmt::Break | Stmt::Continue => {}
        }
    }
}

/// Expression-level scan: records deferred poison sites (`!` over calls and
/// method calls), poisons variants raised by constructs the slice never
/// summarizes (raises inside expression-level blocks, propagated `at`,
/// select-without-default nested in expression blocks), and skips closure
/// and spawn subtrees (their effects accrue to other nodes / are opaque).
struct ExprScan<'a> {
    out: &'a mut FnRaiseSummary,
}

impl Visitor for ExprScan<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        match &expr.node {
            // Closure bodies raise into their own graph node; spawn is opaque
            // to the error system.
            Expr::Closure { .. } | Expr::Spawn { .. } => return,
            Expr::Propagate { expr: inner } => {
                match &inner.node {
                    Expr::Call { name, .. } => {
                        self.out
                            .deferred
                            .push((DeferredKind::Call, name.span.start));
                    }
                    Expr::MethodCall { method, .. } => {
                        self.out
                            .deferred
                            .push((DeferredKind::Method, method.span.start));
                    }
                    Expr::At { .. } => {
                        // A propagated placement boundary raises NetworkError
                        // directly.
                        self.out.poison("NetworkError");
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        walk_expr(self, expr);
    }

    fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
        // Statements reached here live inside expression-level blocks
        // (if/match expressions, catch handler bodies): their raises are
        // never summarized in slice 1.
        match &stmt.node {
            Stmt::Raise { error_name, .. } => self.out.poison(&error_name.node),
            Stmt::Select { default: None, .. } => self.out.poison("ChannelClosed"),
            _ => {}
        }
        walk_stmt(self, stmt);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Call-site evaluation (during body checking)
// ─────────────────────────────────────────────────────────────────────────────

/// Evaluate shrinking for every direct call in this statement's immediate
/// expressions, against the *pre-kill* fact state. Must run before
/// `apply_stmt_kills` (which conservatively drops field facts for the whole
/// statement when it contains any call). Nested statement blocks are not
/// visited — the checker reaches them in flow order with their own facts.
pub(crate) fn record_call_site_shrinking(stmt: &Stmt, env: &mut TypeEnv) {
    if env.raise_summaries.is_empty() {
        return;
    }
    let Some(current_fn) = env.current_fn.clone() else {
        return;
    };
    // Generic CALLERS never shrink their own call sites: their bodies are
    // checked once under skolem types, while facts (and enforcement) would
    // have to hold per instantiation. Generic callees called from concrete
    // code are handled via template summaries (see lookup_summary).
    if current_fn.contains('%') || env.generic_functions.contains_key(&current_fn) {
        return;
    }
    let mut scan = ShrinkScan {
        env,
        current_fn: &current_fn,
        calls_before: false,
        killed_facts: None,
        records: Vec::new(),
    };
    for e in immediate_exprs(stmt) {
        scan.visit_expr(e);
    }
    let records = scan.records;
    for (key, callee, refuted) in records {
        let entry = env
            .call_site_shrunk
            .entry(key)
            .or_insert_with(|| (callee.clone(), HashSet::new()));
        if entry.0 == callee {
            entry.1.extend(refuted);
        }
    }
}

struct ShrinkScan<'a> {
    env: &'a TypeEnv,
    current_fn: &'a str,
    /// A call-like node already completed within this statement: field facts
    /// are no longer trustworthy for subsequent calls.
    calls_before: bool,
    /// Lazily built copy of the fact state with field facts dropped.
    killed_facts: Option<FactEnv>,
    records: Vec<((String, usize), String, HashSet<String>)>,
}

impl ShrinkScan<'_> {
    fn try_shrink(
        &mut self,
        callee: &str,
        span_start: usize,
        args: &[Spanned<Expr>],
        receiver: Option<(&str, bool)>,
    ) {
        let Some((summary, _)) = lookup_summary(self.env, callee) else {
            return;
        };
        if !summary.has_guarded() {
            return;
        }
        // Map callee parameter names to the affine forms of the actual
        // arguments (in the caller's vocabulary). Unresolvable arguments map
        // to None — guards mentioning them stay Unknown.
        let formals: Vec<&String> = summary
            .params
            .iter()
            .filter(|p| p.as_str() != "self")
            .collect();
        if formals.len() != args.len() {
            return;
        }
        let mut arg_aff: HashMap<&str, Option<Affine>> = HashMap::new();
        for (p, a) in formals.iter().zip(args.iter()) {
            arg_aff.insert(p.as_str(), to_affine(&a.node, self.env));
        }
        // Trackable-path views of the actuals, for substituting the
        // callee's `p.field` / `p.len()` guard leaves into the caller's
        // vocabulary: the callee's `p` aliases the actual, so at call time
        // (facts are consulted pre-kill, in execution order) the callee's
        // entry `p.field` equals the caller's `a.field`. Field substitution
        // needs a non-entity class-typed actual (entities never carry field
        // facts); `len()` substitution needs a collection-typed actual.
        let mut arg_field_base: HashMap<&str, String> = HashMap::new();
        let mut arg_len_base: HashMap<&str, String> = HashMap::new();
        for (p, a) in formals.iter().zip(args.iter()) {
            if let Some((apath, aty)) = typed_path(&a.node, self.env) {
                match aty {
                    PlutoType::Class(c)
                        if !self.env.object_types.contains(&c)
                            && !self.env.remote_types.contains(&c)
                            && !self.env.domain_types.contains(&c) =>
                    {
                        arg_field_base.insert(p.as_str(), apath);
                    }
                    PlutoType::Array(_)
                    | PlutoType::String
                    | PlutoType::Bytes
                    | PlutoType::Map(_, _)
                    | PlutoType::Set(_) => {
                        arg_len_base.insert(p.as_str(), apath);
                    }
                    _ => {}
                }
            }
        }
        let resolve = |e: &Expr| -> Option<Affine> {
            match e {
                Expr::Ident(name) => arg_aff.get(name.as_str()).cloned().flatten(),
                Expr::FieldAccess { object, field } => match &object.node {
                    Expr::Ident(s) if s == "self" => match receiver {
                        Some((rpath, true)) => {
                            Some(Affine::term(format!("{rpath}.{}", field.node)))
                        }
                        _ => None,
                    },
                    // One-level field path on a parameter: `p.field` reads
                    // the actual's field in the caller's vocabulary.
                    Expr::Ident(p) => arg_field_base
                        .get(p.as_str())
                        .map(|b| Affine::term(format!("{b}.{}", field.node))),
                    _ => None,
                },
                // `p.len()` on a collection parameter: the actual's length
                // term (carries the automatic >= 0 bound).
                Expr::MethodCall { object, method, args, .. }
                    if method.node == "len" && args.is_empty() =>
                {
                    match &object.node {
                        Expr::Ident(p) if p != "self" => arg_len_base
                            .get(p.as_str())
                            .map(|b| Affine::term(format!("{b}.len()"))),
                        _ => None,
                    }
                }
                _ => None,
            }
        };
        // Borrow juggling: facts() needs &mut self; summary guards need the
        // env borrow. Clone the guarded variants (small) before evaluating.
        let guarded: Vec<(String, Vec<GuardChain>)> = summary
            .variants
            .iter()
            .filter_map(|(v, r)| match r {
                VariantRaises::Guarded(chains) if !chains.is_empty() => {
                    Some((v.clone(), chains.clone()))
                }
                _ => None,
            })
            .collect();
        let facts = if self.calls_before {
            if self.killed_facts.is_none() {
                let mut f = self.env.facts.clone();
                f.kill_fields();
                self.killed_facts = Some(f);
            }
            self.killed_facts.as_ref().expect("just built")
        } else {
            &self.env.facts
        };
        let mut refuted: HashSet<String> = HashSet::new();
        for (variant, chains) in &guarded {
            let all_refuted = chains.iter().all(|chain| {
                // The raise is unreachable iff some conjunct is provably
                // false over the caller's facts.
                chain.conds.iter().any(|(cond, negated)| {
                    let v = eval_condition_with(cond, &resolve, facts);
                    matches!(
                        (v, negated),
                        (Verdict::Refuted, false) | (Verdict::Proven, true)
                    )
                })
            });
            if all_refuted {
                refuted.insert(variant.clone());
            }
        }
        if !refuted.is_empty() {
            self.records.push((
                (self.current_fn.to_string(), span_start),
                callee.to_string(),
                refuted,
            ));
        }
    }
}

impl Visitor for ShrinkScan<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        match &expr.node {
            // A closure body runs later — its calls are not this statement's.
            Expr::Closure { .. } => return,
            // Spawn evaluates its argument expressions now via an opaque
            // runtime path: treat as an executed call, record nothing inside.
            Expr::Spawn { .. } => {
                self.calls_before = true;
                return;
            }
            // Expression-level blocks execute statements the checker visits
            // separately (with their own facts and kills). Do not record
            // their calls here; account for their execution order only.
            Expr::If { .. } | Expr::Match { .. } => {
                if contains_call(expr) {
                    self.calls_before = true;
                }
                return;
            }
            Expr::Catch { expr: inner, handlers } => {
                // The guarded call itself executes unconditionally.
                self.visit_expr(inner);
                // Handler bodies run conditionally, after the call; their
                // own calls are visited by the checker as statements.
                for h in handlers {
                    let handler_has_call = match h {
                        crate::parser::ast::CatchHandler::Wildcard { body, .. }
                        | crate::parser::ast::CatchHandler::Typed { body, .. } => body
                            .node
                            .stmts
                            .iter()
                            .flat_map(|s| immediate_exprs(&s.node))
                            .any(|e| contains_call(e))
                            || body.node.stmts.iter().any(stmt_subtree_has_call),
                        crate::parser::ast::CatchHandler::Shorthand(fb) => contains_call(fb),
                    };
                    if handler_has_call {
                        self.calls_before = true;
                    }
                }
                return;
            }
            _ => {}
        }
        // Children first: a call's receiver and arguments evaluate before
        // the call itself, so post-order matches execution order.
        walk_expr(self, expr);
        match &expr.node {
            Expr::Call { name, args, .. } => {
                // Skip calls through variables (closures, fn-typed values):
                // dynamic targets never shrink.
                if self.env.lookup(&name.node).is_none() {
                    self.try_shrink(&name.node, name.span.start, args, None);
                }
                self.calls_before = true;
            }
            Expr::MethodCall {
                object,
                method,
                args,
                ..
            } => {
                // Builtin `len()` on a collection-typed path is a pure read:
                // it cannot have mutated anything, so it neither shrinks nor
                // invalidates field facts for later calls in the statement.
                if len_path(&expr.node, self.env).is_some() {
                    return;
                }
                if let Some((rpath, PlutoType::Class(cname))) =
                    typed_path(&object.node, self.env)
                {
                    // Entities are shared and mutate concurrently: their
                    // field facts are never tracked, so guards over `self`
                    // fields stay Unknown. Parameter-only guards still work.
                    let fields_ok = !self.env.object_types.contains(&cname)
                        && !self.env.remote_types.contains(&cname)
                        && !self.env.domain_types.contains(&cname);
                    let callee = mangle_method(&cname, &method.node);
                    self.try_shrink(
                        &callee,
                        method.span.start,
                        args,
                        Some((rpath.as_str(), fields_ok)),
                    );
                }
                self.calls_before = true;
            }
            Expr::At { .. } | Expr::StaticTraitCall { .. } => {
                self.calls_before = true;
            }
            _ => {}
        }
    }

    fn visit_stmt(&mut self, _stmt: &Spanned<Stmt>) {
        // Nested statements are visited by the checker in flow order with
        // their own fact state; never from here.
    }
}

/// Does this statement's subtree contain a call-like expression anywhere?
fn stmt_subtree_has_call(stmt: &Spanned<Stmt>) -> bool {
    struct Scan {
        found: bool,
    }
    impl Visitor for Scan {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if self.found {
                return;
            }
            if matches!(
                expr.node,
                Expr::Call { .. }
                    | Expr::MethodCall { .. }
                    | Expr::StaticTraitCall { .. }
                    | Expr::At { .. }
                    | Expr::Spawn { .. }
            ) {
                self.found = true;
                return;
            }
            walk_expr(self, expr);
        }
    }
    let mut scan = Scan { found: false };
    scan.visit_stmt(stmt);
    scan.found
}

// ─────────────────────────────────────────────────────────────────────────────
// Enforcement-time resolution
// ─────────────────────────────────────────────────────────────────────────────

enum PoisonSet {
    All,
    Named(HashSet<String>),
}

/// Find the raise summary for a call-site callee key. Direct lookup covers
/// concrete functions/methods and generic free-function templates (whose
/// call sites and error sets both use the template name). An
/// instance-mangled generic method (`Lease$$Held$renew`) maps back to its
/// template's summary (`Lease$renew`): the template body is identical for
/// every instantiation up to type annotations, and guards never mention
/// type-param-dependent leaves, so the chains hold verbatim. Returns the
/// summary and its error-inference *node key* (the key effect collection
/// used — for propagation-set lookups).
fn lookup_summary<'e>(env: &'e TypeEnv, callee: &str) -> Option<(&'e FnRaiseSummary, String)> {
    if let Some(s) = env.raise_summaries.get(callee) {
        return Some((s, callee.to_string()));
    }
    let template = instance_template_method(env, callee)?;
    env.raise_summaries.get(&template).map(|s| (s, template))
}

/// `Lease$$Held$renew` → `Lease$renew`, via the instantiation registry: the
/// callee must be `<mangled instance class>$<method>` for a recorded class
/// instantiation, with a '$'-free method segment (method names cannot
/// contain '$'; nested-generic argument manglings can, and are rejected by
/// that requirement rather than mis-split).
fn instance_template_method(env: &TypeEnv, callee: &str) -> Option<String> {
    if !callee.contains("$$") {
        return None;
    }
    for inst in &env.instantiations {
        if let InstKind::Class(name) = &inst.kind {
            let mangled_class = mangle_name(name, &inst.type_args);
            if let Some(rest) = callee.strip_prefix(mangled_class.as_str())
                && let Some(method) = rest.strip_prefix('$')
                && !method.is_empty()
                && !method.contains('$')
            {
                return Some(mangle_method(name, method));
            }
        }
    }
    None
}

/// Resolve the callee's deferred poison sites now that typecheck recorded
/// resolutions. Returns the set of variants that may be raised directly by
/// non-`raise` sources (or All when the set is unbounded). Typecheck-keyed
/// records (resolutions, fallible value/builtin calls) live under the
/// summary's `resolution_fn`; closure call sites were recorded during effect
/// collection under the inference `node_key`.
fn deferred_poisons(
    env: &TypeEnv,
    summary: &FnRaiseSummary,
    node_key: &str,
) -> PoisonSet {
    let mut named: HashSet<String> = HashSet::new();
    for (kind, span_start) in &summary.deferred {
        let key = (summary.resolution_fn.clone(), *span_start);
        match kind {
            DeferredKind::Call => {
                if env.fallible_value_calls.contains(&key) {
                    // Propagating through an opaque fallible value widens by
                    // every declared error.
                    return PoisonSet::All;
                }
                if env.closure_call_sites.contains_key(&key)
                    || env
                        .closure_call_sites
                        .contains_key(&(node_key.to_string(), *span_start))
                {
                    // Closure edge: propagation, covered by
                    // fn_propagated_errors.
                    continue;
                }
                if env.fallible_builtin_calls.contains(&key) {
                    named.insert("MathError".to_string());
                }
                // Named-function edge: propagation.
            }
            DeferredKind::Method => match env.method_resolutions.get(&key) {
                Some(MethodResolution::ChannelSend) | Some(MethodResolution::ChannelRecv) => {
                    named.insert("ChannelClosed".to_string());
                }
                Some(MethodResolution::ChannelTrySend) => {
                    named.insert("ChannelClosed".to_string());
                    named.insert("ChannelFull".to_string());
                }
                Some(MethodResolution::ChannelTryRecv) => {
                    named.insert("ChannelClosed".to_string());
                    named.insert("ChannelEmpty".to_string());
                }
                Some(MethodResolution::TaskGet { spawned_fn: None }) => {
                    // Unknown task origin widens by every declared error.
                    return PoisonSet::All;
                }
                Some(MethodResolution::RemoteClass { .. }) => {
                    named.insert("NetworkError".to_string());
                }
                // Edges (propagation) or no direct contribution.
                Some(MethodResolution::TaskGet { spawned_fn: Some(_) })
                | Some(MethodResolution::Class { .. })
                | Some(MethodResolution::TraitDynamic { .. })
                | Some(MethodResolution::Builtin)
                | Some(MethodResolution::TaskDetach)
                | Some(MethodResolution::TaskCancel)
                | None => {}
            },
        }
    }
    PoisonSet::Named(named)
}

/// The errors a call site is still *required* to handle: the callee's full
/// inferred set minus the variants proven unreachable at this site —
/// re-filtered against propagation (a variant that can also arrive through a
/// `!` edge never shrinks) and deferred runtime-raise poisons.
///
/// For an instance-mangled generic callee (`Lease$$Held$renew`) the summary,
/// propagation set, and deferred sites all live under the template's keys:
/// the instance node's only propagation edge is the instance→template bridge
/// (whose propagated set is the template's *full* error set — an artifact of
/// error-set copying, not a real `!` edge), so the template node's own
/// propagated set is the correct filter.
pub(crate) fn required_errors(
    env: &TypeEnv,
    current_fn: &str,
    span_start: usize,
    callee: &str,
) -> HashSet<String> {
    let full = env.fn_errors.get(callee).cloned().unwrap_or_default();
    if full.is_empty() {
        return full;
    }
    let Some((recorded_callee, refuted)) = env
        .call_site_shrunk
        .get(&(current_fn.to_string(), span_start))
    else {
        return full;
    };
    if recorded_callee != callee {
        return full;
    }
    let Some((summary, node_key)) = lookup_summary(env, callee) else {
        return full;
    };
    let poisons = deferred_poisons(env, summary, &node_key);
    let empty = HashSet::new();
    let propagated = env.fn_propagated_errors.get(&node_key).unwrap_or(&empty);
    full.into_iter()
        .filter(|v| {
            let removable = refuted.contains(v)
                && !propagated.contains(v)
                && match &poisons {
                    PoisonSet::All => false,
                    PoisonSet::Named(s) => !s.contains(v),
                };
            !removable
        })
        .collect()
}
