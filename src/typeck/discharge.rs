//! Static invariant discharge — phase 2 of the verification RFC
//! (docs/design/rfc-verification.md), strict mode.
//!
//! Class (and object) `invariant` clauses are compile-time proof
//! obligations: every construction site and every write site must be
//! statically proven to preserve the invariant, or compilation fails.
//! There is no gradual fallback to runtime checks; the only runtime
//! validation that remains is at trust boundaries (wire/marshal decode —
//! see `src/marshal.rs`), where data from outside the compilation unit is
//! testimony, not proof.
//!
//! # The provable fragment
//!
//! An invariant must be decidable by the flow-fact engine (`facts.rs`):
//! `&&`/`||`/`!` over integer comparisons whose sides are linear
//! arithmetic (constant coefficients) over the class's *own* int fields.
//! Anything else — floats, strings, booleans, collections, `.len()`,
//! nested fields, non-linear arithmetic — is rejected at declaration
//! ("invariant is outside the provable fragment"). Generic classes may
//! carry contracts whose vocabulary is *independent of the type
//! parameters* (int fields / int method parameters whose declared type
//! does not mention any parameter): validated once on the template,
//! proven once on the template body under skolem substitution, stamped
//! onto every instantiation (`instantiate_generic_contracts`), and not
//! re-proven per monomorphized copy (`assume_discharged`). A contract
//! mentioning a param-typed field or parameter is rejected with a
//! dedicated diagnostic.
//!
//! # Obligation sites
//!
//! - **Construction**: each struct literal of an invariant-carrying class
//!   must prove the invariant from the field initializers (substituted
//!   into the invariant and evaluated against the flow facts in scope).
//! - **DI construction**: instances synthesized by dependency-injection
//!   wiring (startup singletons and per-injection transients from
//!   `env.di_order`, and scope-block auto-created instances) never pass
//!   through a struct literal — they are allocated zero-initialized and
//!   only their injected (class-typed) dep fields are wired. Their
//!   invariants must therefore hold for the all-int-fields-are-zero state
//!   (`check_di_construction`). Seeded scope instances are ordinary struct
//!   literals and carry the construction obligation above instead.
//! - **Foreign field writes** (`obj.field = v` anywhere outside the
//!   class's own `mut self` methods): the invariant must be proven
//!   *immediately* after the write — external code gets no
//!   temporary-violation window.
//! - **`mut self` method bodies**: writes to `self.field` perform a
//!   symbolic strong update (the field's value is tracked as an affine
//!   form over "ghost" variables — entry values of fields and locals), and
//!   the invariant must be proven at every *boundary*: method exits
//!   (return / fall-through), `raise`, `break`/`continue`, `yield`, any
//!   statement containing a call *that may reach the receiver* (the callee
//!   — or anyone holding an alias — may observe the object: this is the
//!   conservative slice-1 answer to reentrancy, rfc-objects.md open
//!   question 6), loop entry/body-end, and branch joins where a surviving
//!   branch changed the symbolic state. Calls that provably cannot reach
//!   the receiver — builtin/primitive methods, free functions whose
//!   declared parameter types cannot reach the receiver's class
//!   (facts::call_severity, the shared purity predicate) — are NOT
//!   boundaries: with no possible observer or writer, exact two-state
//!   knowledge survives them. A call that may OBSERVE but provably cannot
//!   WRITE the receiver (a non-`mut-self` method of the receiver's own
//!   class, no alias-capable parameter, transitively write-free body —
//!   see `calls_cannot_write_class`, issue #454) is a *half* boundary:
//!   the invariant must still hold at the call, but exact two-state
//!   knowledge survives it. *Between* boundaries the invariant may be
//!   temporarily broken (subtract-then-add works because the symbolic
//!   forms cancel).
//!
//! Raise paths are NOT exempt: for value classes a raised error can carry
//! or share the receiver, so the invariant must hold when a raise leaves
//! the method. (Entity transactional methods that roll back on raise are a
//! future direction.)
//!
//! # What the prover knows
//!
//! At function entry: `requires` clauses (runtime-checked at entry) and
//! the invariants of every class-typed parameter. Inside a `mut self`
//! method the same facts are mirrored into the ghost vocabulary. Guards
//! (`if` conditions) contribute facts in both vocabularies; `assert`
//! establishes facts for the remainder of the block. After a call that may
//! reach the receiver, facts about fields are reduced to invariant-level
//! knowledge (anything finer may have been invalidated by the callee);
//! calls that provably cannot reach it leave the facts untouched.
//!
//! False rejection is the failure mode to fear: every Unknown verdict is a
//! compile error under strict mode, so diagnostics name the invariant, the
//! site, the symbolic state, and the fix (add a guard / requires /
//! assert).

use std::collections::{HashMap, HashSet};

use crate::diagnostics::CompileError;
use crate::parser::ast::{
    expr_contains_old, old_call_arg, BinOp, Block, ContractKind, Expr, Function, Program, Stmt,
    UnaryOp,
};
use crate::span::{Span, Spanned};
use crate::visit::{walk_expr, walk_stmt, Visitor};

use super::env::{mangle_method, TypeEnv};
use super::facts::{
    affine_add_const, affine_bounds, condition_facts, condition_facts_with, contains_impure_call,
    diff_affine, eval_condition_with, facts_from_diff, immediate_exprs, is_comparison, len_path,
    to_affine, to_affine_with, typed_path, Affine, Fact, FactEnv, Interval, RelOp, Verdict,
};
use super::types::PlutoType;

// ─────────────────────────────────────────────────────────────────────────────
// Registration and declaration-time validation
// ─────────────────────────────────────────────────────────────────────────────

/// A validated, provable invariant of a class.
#[derive(Debug, Clone)]
pub struct InvariantSpec {
    pub expr: Expr,
    pub desc: String,
    pub span: Span,
    /// Contains `old(...)` — a *two-state* invariant (rfc-properties.md
    /// atom 2) relating each state transition to its pre-state. Checked at
    /// every `mut self` method boundary (relative to the method's entry) and
    /// at every foreign write site (relative to the pre-write state); never
    /// at construction, DI synthesis, or wire-decode (no pre-state exists
    /// there).
    pub two_state: bool,
    /// Set when the invariant was injected by a property instantiation
    /// (`satisfies`, rfc-properties.md slice 2). Every diagnostic about the
    /// obligation appends `blame()` so the failure shows BOTH the property
    /// body and the failing site (two-sided blame).
    pub provenance: Option<crate::parser::ast::PropertyProvenance>,
}

impl InvariantSpec {
    /// The property-side blame suffix ("" for hand-written invariants).
    fn blame(&self) -> String {
        crate::parser::ast::provenance_blame(&self.provenance)
    }
}

/// A validated, provable `ensures` postcondition of a method
/// (rfc-properties.md atom 1): a two-state relation between the method's
/// exit and entry states, proven at every normal exit (returns and
/// fall-through — raise paths are exempt) and *assumed* by callers after a
/// direct call.
#[derive(Debug, Clone)]
pub struct EnsuresSpec {
    pub expr: Expr,
    pub desc: String,
    pub span: Span,
    /// Parameter names in declaration order, excluding `self` — the
    /// substitution vocabulary for caller-side assumption.
    pub params: Vec<String>,
    /// Set when the clause was injected by a fn-level property
    /// instantiation (`provides increments(...)` — rfc-properties.md
    /// phase 5): two-sided blame on discharge failures.
    pub provenance: Option<crate::parser::ast::PropertyProvenance>,
}

/// Rewrite `old(e)` to `e` for type-checking purposes: `old(e)` has the type
/// of `e` (placement/fragment validity is checked separately).
pub(crate) fn strip_old(expr: &Expr) -> Expr {
    if let Some(inner) = old_call_arg(expr) {
        return strip_old(&inner.node);
    }
    match expr {
        Expr::BinOp { op, lhs, rhs } => Expr::BinOp {
            op: *op,
            lhs: Box::new(Spanned::new(strip_old(&lhs.node), lhs.span)),
            rhs: Box::new(Spanned::new(strip_old(&rhs.node), rhs.span)),
        },
        Expr::UnaryOp { op, operand } => Expr::UnaryOp {
            op: *op,
            operand: Box::new(Spanned::new(strip_old(&operand.node), operand.span)),
        },
        other => other.clone(),
    }
}

/// Type-check and fragment-validate every class invariant, and register the
/// provable specs in `env.class_invariants`. Runs before any body checking
/// so obligations can be enforced at every site.
///
/// Generic classes are supported for contracts whose vocabulary is
/// *independent of the type parameters* (int fields whose declared type does
/// not mention any parameter — exactly the restriction that makes generic
/// raise-summaries sound in `shrink.rs`): the clause is validated once
/// against the template's field vocabulary, registered under the template
/// name in `env.generic_class_invariants`, and stamped verbatim onto every
/// instantiation by `ensure_generic_class_instantiated`. The proof
/// obligations on the template's method bodies are then discharged ONCE,
/// under skolem substitution (`templates::check_class_template` checks each
/// method as a member of `C$$%T`, whose `class_invariants` entry is the
/// stamped copy) — since the vocabulary cannot mention the parameters, one
/// proof covers every instantiation, and monomorphized copies do not
/// re-prove (`env.assume_discharged`).
pub(crate) fn register_invariants(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for class in &program.classes {
        let c = &class.node;
        // `must_release` annotations ride in `invariants` but are typestate
        // markers, not value predicates — they are validated during class
        // registration, not here.
        let invariants: Vec<_> = c
            .invariants
            .iter()
            .filter(|i| i.node.kind == crate::parser::ast::ContractKind::Invariant)
            .collect();
        if invariants.is_empty() {
            continue;
        }
        if !c.type_params.is_empty() {
            // Template registration: fragment-validate against the template's
            // own field vocabulary (type-param-dependent fields rejected with
            // a dedicated diagnostic). Bool-ness is implied by the fragment
            // (the top level must be a boolean combination of comparisons);
            // the skolem template check re-types the expression as well.
            let fields = env
                .generic_classes
                .get(&c.name.node)
                .map(|g| g.fields.clone())
                .unwrap_or_default();
            let mut specs = Vec::new();
            for inv in &invariants {
                let desc = crate::codegen::format_invariant_expr(&inv.node.expr.node);
                validate_provable(&inv.node.expr, &c.name.node, &fields, true, &desc)
                    .map_err(|e| append_blame(e, &inv.node.provenance))?;
                specs.push(InvariantSpec {
                    expr: inv.node.expr.node.clone(),
                    desc,
                    span: inv.node.expr.span,
                    two_state: expr_contains_old(&inv.node.expr.node),
                    provenance: inv.node.provenance.clone(),
                });
            }
            let mut coll = HashSet::new();
            for s in &specs {
                collect_len_fields(&s.expr, &fields, &mut coll);
            }
            if !coll.is_empty() {
                env.invariant_collection_fields.insert(c.name.node.clone(), coll);
            }
            env.generic_class_invariants.insert(c.name.node.clone(), specs);
            continue;
        }
        // Type-check the invariant expressions with `self` in scope.
        env.push_scope();
        env.define_unchecked("self".to_string(), PlutoType::Class(c.name.node.clone()));
        for inv in &invariants {
            // `old(e)` types as `e`; its placement is validated by the
            // provable-fragment check below.
            let stripped = strip_old(&inv.node.expr.node);
            let inv_type =
                super::infer::infer_expr(&stripped, inv.node.expr.span, env, None)?;
            if inv_type != PlutoType::Bool {
                env.pop_scope();
                return Err(append_blame(
                    CompileError::type_err(
                        format!("invariant expression must be bool, found {inv_type}"),
                        inv.node.expr.span,
                    ),
                    &inv.node.provenance,
                ));
            }
        }
        env.pop_scope();

        // Provable-fragment validation. For property-injected clauses the
        // fragment error gains the property-side blame suffix (two-sided
        // blame also covers instantiation-time validation failures).
        let fields = env
            .classes
            .get(&c.name.node)
            .map(|ci| ci.fields.clone())
            .unwrap_or_default();
        let mut specs = Vec::new();
        for inv in &invariants {
            let desc = crate::codegen::format_invariant_expr(&inv.node.expr.node);
            validate_provable(&inv.node.expr, &c.name.node, &fields, false, &desc)
                .map_err(|e| append_blame(e, &inv.node.provenance))?;
            specs.push(InvariantSpec {
                expr: inv.node.expr.node.clone(),
                desc,
                span: inv.node.expr.span,
                two_state: expr_contains_old(&inv.node.expr.node),
                provenance: inv.node.provenance.clone(),
            });
        }
        let mut coll = HashSet::new();
        for s in &specs {
            collect_len_fields(&s.expr, &fields, &mut coll);
        }
        if !coll.is_empty() {
            env.invariant_collection_fields.insert(c.name.node.clone(), coll);
        }
        env.class_invariants.insert(c.name.node.clone(), specs);
    }
    // Instantiations minted before this pass (eager field/signature
    // resolution during registration) predate the copy hook in
    // `ensure_generic_class_instantiated`; stamp their specs now.
    backfill_generic_contracts(env);
    Ok(())
}

/// Does a type's structure mention any type parameter?
fn type_involves_param(ty: &PlutoType) -> bool {
    matches!(ty, PlutoType::TypeParam(_)) || ty.any_inner_type(&type_involves_param)
}

/// Copy a generic template's validated contract specs onto one instantiation:
/// invariants under the mangled class name, ensures under the instantiation's
/// mangled method names (only the methods the instantiation actually has —
/// the typestate gate may exclude `where`-constrained methods). The specs are
/// cloned verbatim: their vocabulary is param-independent by construction, so
/// no substitution is needed. Covers skolem instantiations too (`C$$%T`),
/// which is what puts the proof obligations on the template's own method
/// bodies during `templates::check_class_template`.
pub(crate) fn instantiate_generic_contracts(
    base: &str,
    mangled: &str,
    included_methods: &[String],
    env: &mut TypeEnv,
) {
    if let Some(specs) = env.generic_class_invariants.get(base) {
        let specs = specs.clone();
        env.class_invariants.entry(mangled.to_string()).or_insert(specs);
    }
    // Ghost-tracked collection fields keep their names under monomorphization,
    // so the door set copies verbatim onto the instantiation.
    if let Some(coll) = env.invariant_collection_fields.get(base) {
        let coll = coll.clone();
        env.invariant_collection_fields
            .entry(mangled.to_string())
            .or_insert(coll);
    }
    if let Some(by_method) = env.generic_class_ensures.get(base) {
        let by_method = by_method.clone();
        for m in included_methods {
            if let Some(specs) = by_method.get(m) {
                env.fn_ensures
                    .entry(mangle_method(mangled, m))
                    .or_insert_with(|| specs.clone());
            }
        }
    }
}

/// Stamp contract specs onto every already-recorded class instantiation of a
/// contract-carrying generic template (see `instantiate_generic_contracts`).
fn backfill_generic_contracts(env: &mut TypeEnv) {
    if env.generic_class_invariants.is_empty() && env.generic_class_ensures.is_empty() {
        return;
    }
    let insts: Vec<(String, String)> = env
        .instantiations
        .iter()
        .filter_map(|inst| {
            let super::env::InstKind::Class(base) = &inst.kind else {
                return None;
            };
            if !env.generic_class_invariants.contains_key(base)
                && !env.generic_class_ensures.contains_key(base)
            {
                return None;
            }
            Some((base.clone(), super::env::mangle_name(base, &inst.type_args)))
        })
        .collect();
    for (base, mangled) in insts {
        let methods = env
            .classes
            .get(&mangled)
            .map(|ci| ci.methods.clone())
            .unwrap_or_default();
        instantiate_generic_contracts(&base, &mangled, &methods, env);
    }
}

/// Append the property-side blame suffix to a diagnostic about a
/// property-injected clause (no-op for hand-written clauses).
pub(crate) fn append_blame(
    err: CompileError,
    provenance: &Option<crate::parser::ast::PropertyProvenance>,
) -> CompileError {
    let suffix = crate::parser::ast::provenance_blame(provenance);
    if suffix.is_empty() {
        return err;
    }
    match err {
        CompileError::Type { msg, span } => CompileError::Type { msg: format!("{msg}{suffix}"), span },
        CompileError::Syntax { msg, span } => CompileError::Syntax { msg: format!("{msg}{suffix}"), span },
        other => other,
    }
}

fn fragment_err(reason: String, desc: &str, span: Span) -> CompileError {
    CompileError::type_err(
        format!(
            "invariant '{desc}' is outside the provable fragment: {reason}. Invariants are \
             compile-time proof obligations; they must be built from &&, ||, ! over integer \
             comparisons of linear arithmetic over the class's own int fields \
             (e.g. 'self.balance >= 0', 'self.lo <= self.hi'). Properties outside this \
             fragment cannot be statically discharged and are rejected"
        ),
        span,
    )
}

/// Validate that an invariant condition is within the provable fragment.
/// `fields` is the declaring class's field vocabulary; `generic` marks a
/// generic template, whose contracts may only mention fields with
/// param-independent types.
fn validate_provable(
    expr: &Spanned<Expr>,
    class_name: &str,
    fields: &[(String, PlutoType, bool)],
    generic: bool,
    desc: &str,
) -> Result<(), CompileError> {
    match &expr.node {
        Expr::BinOp { op: BinOp::And | BinOp::Or, lhs, rhs } => {
            validate_provable(lhs, class_name, fields, generic, desc)?;
            validate_provable(rhs, class_name, fields, generic, desc)
        }
        Expr::UnaryOp { op: UnaryOp::Not, operand } => {
            validate_provable(operand, class_name, fields, generic, desc)
        }
        Expr::BinOp {
            op: BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq | BinOp::Eq | BinOp::Neq,
            lhs,
            rhs,
        } => {
            validate_affine_side(lhs, class_name, fields, generic, desc)?;
            validate_affine_side(rhs, class_name, fields, generic, desc)
        }
        _ => Err(fragment_err(
            "it is not an integer comparison (boolean fields, bare literals, and \
             non-comparison expressions are not provable)"
                .to_string(),
            desc,
            expr.span,
        )),
    }
}

/// Validate one side of an invariant comparison: linear integer arithmetic
/// over direct int fields of `self`. On a generic template (`generic`),
/// fields whose declared type involves a type parameter are rejected — the
/// contract must hold for every instantiation, so its vocabulary must be
/// independent of the parameters.
fn validate_affine_side(
    expr: &Spanned<Expr>,
    class_name: &str,
    fields: &[(String, PlutoType, bool)],
    generic: bool,
    desc: &str,
) -> Result<(), CompileError> {
    // `old(<affine over own int fields>)` — the pre-state value of its
    // argument (rfc-properties.md atom 2). Nested old() is rejected by
    // contract validation before this runs; validate the argument with the
    // same side grammar.
    if let Some(inner) = old_call_arg(&expr.node) {
        return validate_affine_side(inner, class_name, fields, generic, desc);
    }
    match &expr.node {
        Expr::IntLit(_) => Ok(()),
        Expr::FloatLit(_) => Err(fragment_err(
            "float comparisons are not provable (IEEE semantics are outside the integer \
             fragment)"
                .to_string(),
            desc,
            expr.span,
        )),
        Expr::FieldAccess { object, field } => {
            if !matches!(&object.node, Expr::Ident(s) if s == "self") {
                return Err(fragment_err(
                    "nested field access — only direct int fields of self are provable"
                        .to_string(),
                    desc,
                    expr.span,
                ));
            }
            let fty = fields
                .iter()
                .find(|(n, _, _)| *n == field.node)
                .map(|(_, t, _)| t.clone());
            match fty {
                Some(PlutoType::Int) => Ok(()),
                Some(other) if generic && type_involves_param(&other) => {
                    Err(CompileError::type_err(
                        format!(
                            "this invariant mentions field '{}' whose type involves a type \
                             parameter of '{class_name}' (its declared type is {other}): \
                             contracts on a generic class must hold for every \
                             instantiation, so they may only use int fields whose declared \
                             type does not mention any type parameter",
                            field.node,
                        ),
                        expr.span,
                    ))
                }
                Some(other) => Err(fragment_err(
                    format!(
                        "field '{}' has type {other} — only int fields are provable",
                        field.node
                    ),
                    desc,
                    expr.span,
                )),
                None => Err(CompileError::type_err(
                    format!("class '{class_name}' has no field '{}'", field.node),
                    expr.span,
                )),
            }
        }
        // `self.<coll>.len()` on the class's own array/map/set/bytes field is
        // a ghost int (rfc-number-types.md §4): modeled symbolically with the
        // builtin mutators as transfer functions. Independent of any type
        // parameter (the length of `[T]` is an int), so it is accepted on
        // generic templates too.
        Expr::MethodCall { object, method, args, .. }
            if method.node == "len" && args.is_empty() =>
        {
            match collection_len_field(&object.node, fields) {
                Some(_) => Ok(()),
                None => Err(fragment_err(
                    "'.len()' is provable only on the class's own array/map/set/bytes \
                     fields (e.g. 'self.items.len() > 0'); nested or foreign collections \
                     are not tracked"
                        .to_string(),
                    desc,
                    expr.span,
                )),
            }
        }
        Expr::MethodCall { method, .. } => Err(fragment_err(
            format!(
                "'.{}()' — collection and method facts are not provable",
                method.node
            ),
            desc,
            expr.span,
        )),
        Expr::UnaryOp { op: UnaryOp::Neg, operand } => {
            validate_affine_side(operand, class_name, fields, generic, desc)
        }
        Expr::BinOp { op: BinOp::Add | BinOp::Sub, lhs, rhs } => {
            validate_affine_side(lhs, class_name, fields, generic, desc)?;
            validate_affine_side(rhs, class_name, fields, generic, desc)
        }
        Expr::BinOp { op: BinOp::Mul, lhs, rhs } => {
            if !is_const_int_expr(&lhs.node) && !is_const_int_expr(&rhs.node) {
                return Err(fragment_err(
                    "non-linear arithmetic (a product of two fields) is not provable"
                        .to_string(),
                    desc,
                    expr.span,
                ));
            }
            validate_affine_side(lhs, class_name, fields, generic, desc)?;
            validate_affine_side(rhs, class_name, fields, generic, desc)
        }
        Expr::BinOp { op: BinOp::Div | BinOp::Mod, .. } => Err(fragment_err(
            "division and modulo are not linear arithmetic".to_string(),
            desc,
            expr.span,
        )),
        _ => Err(fragment_err(
            "only int literals, direct int fields of self, and +, -, * by a constant are \
             provable"
                .to_string(),
            desc,
            expr.span,
        )),
    }
}

fn is_const_int_expr(expr: &Expr) -> bool {
    match expr {
        Expr::IntLit(_) => true,
        Expr::UnaryOp { op: UnaryOp::Neg, operand } => is_const_int_expr(&operand.node),
        Expr::BinOp { op: BinOp::Add | BinOp::Sub | BinOp::Mul, lhs, rhs } => {
            is_const_int_expr(&lhs.node) && is_const_int_expr(&rhs.node)
        }
        _ => false,
    }
}

/// Is this field type one the length-ghost model tracks (array/map/set/bytes)?
fn is_tracked_collection(ty: &PlutoType) -> bool {
    matches!(
        ty,
        PlutoType::Array(_) | PlutoType::Map(_, _) | PlutoType::Set(_) | PlutoType::Bytes
    )
}

/// If `object` is exactly `self.<field>` where `<field>` is one of the
/// class's own tracked collection fields, return the field name. Used to
/// recognize `self.<coll>.len()` in the invariant fragment.
fn collection_len_field(object: &Expr, fields: &[(String, PlutoType, bool)]) -> Option<String> {
    let Expr::FieldAccess { object: inner, field } = object else {
        return None;
    };
    if !matches!(&inner.node, Expr::Ident(s) if s == "self") {
        return None;
    }
    let ty = fields.iter().find(|(n, _, _)| *n == field.node).map(|(_, t, _)| t)?;
    is_tracked_collection(ty).then(|| field.node.clone())
}

/// Collect every collection field a provable invariant constrains through a
/// `self.<coll>.len()` term (the aliasing-door set, rfc-number-types.md §4).
fn collect_len_fields(expr: &Expr, fields: &[(String, PlutoType, bool)], out: &mut HashSet<String>) {
    match expr {
        Expr::MethodCall { object, method, args, .. }
            if method.node == "len" && args.is_empty() =>
        {
            if let Some(f) = collection_len_field(&object.node, fields) {
                out.insert(f);
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_len_fields(&lhs.node, fields, out);
            collect_len_fields(&rhs.node, fields, out);
        }
        Expr::UnaryOp { operand, .. } => collect_len_fields(&operand.node, fields, out),
        _ => {}
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Ensures registration (rfc-properties.md atom 1)
// ─────────────────────────────────────────────────────────────────────────────

/// Type-check and fragment-validate every `ensures` clause on class/object
/// methods, and register the provable specs in `env.fn_ensures` (keyed by
/// mangled method name). Runs right after `register_invariants`, before any
/// body checking, so obligations exist at every exit and callers can assume
/// the relation.
pub(crate) fn register_ensures(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for class in &program.classes {
        let c = &class.node;
        for method in &c.methods {
            let m = &method.node;
            let clauses: Vec<_> = m
                .contracts
                .iter()
                .filter(|cc| cc.node.kind == ContractKind::Ensures)
                .collect();
            if clauses.is_empty() {
                continue;
            }
            if !m.params.iter().any(|p| p.name.node == "self") {
                return Err(CompileError::type_err(
                    "'ensures' requires a 'self' receiver: the clause relates the \
                     receiver's exit state to its entry state"
                        .to_string(),
                    clauses[0].span,
                ));
            }
            let generic = !c.type_params.is_empty();
            if generic {
                // Template registration (see register_invariants): validate
                // against the template's field and parameter vocabulary —
                // param-typed terms rejected with dedicated diagnostics —
                // and register under the template name; instantiation
                // copies happen in ensure_generic_class_instantiated.
                // Bool-ness is implied by the fragment; the skolem template
                // check re-types the method bodies.
                let fields = env
                    .generic_classes
                    .get(&c.name.node)
                    .map(|g| g.fields.clone())
                    .unwrap_or_default();
                let type_param_names: std::collections::HashSet<String> =
                    c.type_params.iter().map(|tp| tp.node.clone()).collect();
                let mut int_params: Vec<String> = Vec::new();
                let mut param_typed: Vec<String> = Vec::new();
                for p in &m.params {
                    if p.name.node == "self" {
                        continue;
                    }
                    let ty = super::resolve::resolve_type_with_params(
                        &p.ty,
                        env,
                        &type_param_names,
                    )?;
                    if ty == PlutoType::Int {
                        int_params.push(p.name.node.clone());
                    } else if type_involves_param(&ty) {
                        param_typed.push(p.name.node.clone());
                    }
                }
                let params: Vec<String> = m
                    .params
                    .iter()
                    .filter(|p| p.name.node != "self")
                    .map(|p| p.name.node.clone())
                    .collect();
                let mut specs = Vec::new();
                for cl in &clauses {
                    let desc = crate::codegen::format_invariant_expr(&cl.node.expr.node);
                    validate_ensures_provable(
                        &cl.node.expr,
                        &c.name.node,
                        &fields,
                        &int_params,
                        &param_typed,
                        true,
                        &desc,
                    )?;
                    specs.push(EnsuresSpec {
                        expr: cl.node.expr.node.clone(),
                        desc,
                        span: cl.node.expr.span,
                        params: params.clone(),
                        provenance: None,
                    });
                }
                env.generic_class_ensures
                    .entry(c.name.node.clone())
                    .or_default()
                    .insert(m.name.node.clone(), specs);
                continue;
            }
            // Type-check the clauses with self and parameters in scope
            // (`old(e)` types as `e`).
            env.push_scope();
            env.define_unchecked("self".to_string(), PlutoType::Class(c.name.node.clone()));
            let mut int_params: Vec<String> = Vec::new();
            for p in &m.params {
                if p.name.node == "self" {
                    continue;
                }
                let ty = super::resolve::resolve_type(&p.ty, env)?;
                if ty == PlutoType::Int {
                    int_params.push(p.name.node.clone());
                }
                env.define_unchecked(p.name.node.clone(), ty);
            }
            for cl in &clauses {
                let stripped = strip_old(&cl.node.expr.node);
                let ty = super::infer::infer_expr(&stripped, cl.node.expr.span, env, None);
                let ty = match ty {
                    Ok(t) => t,
                    Err(e) => {
                        env.pop_scope();
                        return Err(e);
                    }
                };
                if ty != PlutoType::Bool {
                    env.pop_scope();
                    return Err(CompileError::type_err(
                        format!("ensures expression must be bool, found {ty}"),
                        cl.node.expr.span,
                    ));
                }
            }
            env.pop_scope();

            // Provable-fragment validation.
            let fields = env
                .classes
                .get(&c.name.node)
                .map(|ci| ci.fields.clone())
                .unwrap_or_default();
            let mut specs = Vec::new();
            let params: Vec<String> = m
                .params
                .iter()
                .filter(|p| p.name.node != "self")
                .map(|p| p.name.node.clone())
                .collect();
            for cl in &clauses {
                let desc = crate::codegen::format_invariant_expr(&cl.node.expr.node);
                validate_ensures_provable(
                    &cl.node.expr,
                    &c.name.node,
                    &fields,
                    &int_params,
                    &[],
                    false,
                    &desc,
                )?;
                specs.push(EnsuresSpec {
                    expr: cl.node.expr.node.clone(),
                    desc,
                    span: cl.node.expr.span,
                    params: params.clone(),
                    provenance: cl.node.provenance.clone(),
                });
            }
            env.fn_ensures
                .insert(mangle_method(&c.name.node, &m.name.node), specs);
        }
    }
    // Stamp specs onto instantiations that predate this pass (see
    // register_invariants).
    backfill_generic_contracts(env);
    Ok(())
}

fn ensures_fragment_err(reason: String, desc: &str, span: Span) -> CompileError {
    CompileError::type_err(
        format!(
            "ensures clause '{desc}' is outside the provable fragment: {reason}. Ensures \
             clauses are compile-time proof obligations; they must be built from &&, ||, ! \
             over integer comparisons of linear arithmetic over the class's own int fields, \
             the method's int parameters, and old(...) of those \
             (e.g. 'self.epoch == old(self.epoch) + 1', \
             'self.balance == old(self.balance) - amt'). Properties outside this fragment \
             cannot be statically discharged and are rejected"
        ),
        span,
    )
}

/// Validate that an ensures condition is within the provable fragment:
/// boolean combinations of integer comparisons whose sides are linear
/// arithmetic over the class's own int fields, the method's int parameters,
/// and `old(...)` of those. `len()` terms are deliberately excluded for now:
/// every mutation of a collection is an opaque call that severs the entry
/// relation, so no `len()` ensures could be proven today.
fn validate_ensures_provable(
    expr: &Spanned<Expr>,
    class_name: &str,
    fields: &[(String, PlutoType, bool)],
    int_params: &[String],
    param_typed: &[String],
    generic: bool,
    desc: &str,
) -> Result<(), CompileError> {
    match &expr.node {
        Expr::BinOp { op: BinOp::And | BinOp::Or, lhs, rhs } => {
            validate_ensures_provable(lhs, class_name, fields, int_params, param_typed, generic, desc)?;
            validate_ensures_provable(rhs, class_name, fields, int_params, param_typed, generic, desc)
        }
        Expr::UnaryOp { op: UnaryOp::Not, operand } => {
            validate_ensures_provable(operand, class_name, fields, int_params, param_typed, generic, desc)
        }
        Expr::BinOp {
            op: BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq | BinOp::Eq | BinOp::Neq,
            lhs,
            rhs,
        } => {
            validate_ensures_side(lhs, class_name, fields, int_params, param_typed, generic, desc)?;
            validate_ensures_side(rhs, class_name, fields, int_params, param_typed, generic, desc)
        }
        _ => Err(ensures_fragment_err(
            "it is not an integer comparison (boolean fields, bare literals, and \
             non-comparison expressions are not provable)"
                .to_string(),
            desc,
            expr.span,
        )),
    }
}

fn validate_ensures_side(
    expr: &Spanned<Expr>,
    class_name: &str,
    fields: &[(String, PlutoType, bool)],
    int_params: &[String],
    param_typed: &[String],
    generic: bool,
    desc: &str,
) -> Result<(), CompileError> {
    if let Some(inner) = old_call_arg(&expr.node) {
        // Nested old() is rejected during contract validation.
        return validate_ensures_side(inner, class_name, fields, int_params, param_typed, generic, desc);
    }
    match &expr.node {
        Expr::IntLit(_) => Ok(()),
        Expr::Ident(name) => {
            if int_params.iter().any(|p| p == name) {
                Ok(())
            } else if param_typed.iter().any(|p| p == name) {
                Err(CompileError::type_err(
                    format!(
                        "this ensures clause mentions parameter '{name}' whose type \
                         involves a type parameter of '{class_name}': contracts on a \
                         generic class must hold for every instantiation, so they may \
                         only use int parameters whose declared type does not mention \
                         any type parameter"
                    ),
                    expr.span,
                ))
            } else {
                Err(ensures_fragment_err(
                    format!(
                        "'{name}' is not an int parameter of this method — only the \
                         class's own int fields and the method's int parameters are \
                         provable terms"
                    ),
                    desc,
                    expr.span,
                ))
            }
        }
        Expr::FieldAccess { object, field } => {
            if !matches!(&object.node, Expr::Ident(s) if s == "self") {
                return Err(ensures_fragment_err(
                    "nested field access — only direct int fields of self are provable"
                        .to_string(),
                    desc,
                    expr.span,
                ));
            }
            let fty = fields
                .iter()
                .find(|(n, _, _)| *n == field.node)
                .map(|(_, t, _)| t.clone());
            match fty {
                Some(PlutoType::Int) => Ok(()),
                Some(other) if generic && type_involves_param(&other) => {
                    Err(CompileError::type_err(
                        format!(
                            "this ensures clause mentions field '{}' whose type involves \
                             a type parameter of '{class_name}' (its declared type is \
                             {other}): contracts on a generic class must hold for every \
                             instantiation, so they may only use int fields whose \
                             declared type does not mention any type parameter",
                            field.node,
                        ),
                        expr.span,
                    ))
                }
                Some(other) => Err(ensures_fragment_err(
                    format!(
                        "field '{}' has type {other} — only int fields are provable",
                        field.node
                    ),
                    desc,
                    expr.span,
                )),
                None => Err(CompileError::type_err(
                    format!("class '{class_name}' has no field '{}'", field.node),
                    expr.span,
                )),
            }
        }
        Expr::MethodCall { method, .. } => Err(ensures_fragment_err(
            format!(
                "'.{}()' — collection and method facts are not provable (len() terms \
                 are not yet supported in ensures)",
                method.node
            ),
            desc,
            expr.span,
        )),
        Expr::UnaryOp { op: UnaryOp::Neg, operand } => {
            validate_ensures_side(operand, class_name, fields, int_params, param_typed, generic, desc)
        }
        Expr::BinOp { op: BinOp::Add | BinOp::Sub, lhs, rhs } => {
            validate_ensures_side(lhs, class_name, fields, int_params, param_typed, generic, desc)?;
            validate_ensures_side(rhs, class_name, fields, int_params, param_typed, generic, desc)
        }
        Expr::BinOp { op: BinOp::Mul, lhs, rhs } => {
            if !is_const_int_expr(&lhs.node) && !is_const_int_expr(&rhs.node) {
                return Err(ensures_fragment_err(
                    "non-linear arithmetic (a product of two non-constant terms) is not \
                     provable"
                        .to_string(),
                    desc,
                    expr.span,
                ));
            }
            validate_ensures_side(lhs, class_name, fields, int_params, param_typed, generic, desc)?;
            validate_ensures_side(rhs, class_name, fields, int_params, param_typed, generic, desc)
        }
        Expr::BinOp { op: BinOp::Div | BinOp::Mod, .. } => Err(ensures_fragment_err(
            "division and modulo are not linear arithmetic".to_string(),
            desc,
            expr.span,
        )),
        _ => Err(ensures_fragment_err(
            "only int literals, direct int fields of self, int parameters, old(...) of \
             those, and +, -, * by a constant are provable"
                .to_string(),
            desc,
            expr.span,
        )),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// The per-method proof scope (ghost vocabulary)
// ─────────────────────────────────────────────────────────────────────────────

/// Proof state for the body of a `mut self` method of an invariant-carrying
/// class. Field values are tracked *symbolically*: each int field maps to
/// an affine form over ghost variables (`self.f@N` for a field's value at
/// the last boundary, `x@V` for a local's value at its current version).
/// Ghosts are SSA values — facts about them are never killed, only scoped
/// (the ghost fact store's frames mirror the checker's scopes).
#[derive(Debug, Clone)]
pub struct InvariantScope {
    pub class_name: String,
    method_name: String,
    invariants: Vec<InvariantSpec>,
    /// Ensures postconditions of the method being checked: proof obligations
    /// at every normal exit (returns + reachable fall-through).
    ensures: Vec<EnsuresSpec>,
    /// Facts over the ghost vocabulary.
    pub ghost_facts: FactEnv,
    /// Current symbolic value of each tracked field of the class: int fields
    /// map to their affine value, collection fields (`coll_fields`) map to
    /// their ghost *length* (a `self.<f>.len()@N` term, so `facts::is_len_term`
    /// keeps the automatic `>= 0` bound).
    sym: HashMap<String, Affine>,
    /// All int fields of the class (a subset of the sym key set).
    fields: Vec<String>,
    /// Collection fields (array/map/set/bytes) whose length a class invariant
    /// constrains (rfc-number-types.md §4). Their sym entry holds a length
    /// ghost; the builtin mutators are transfer functions over it. Disjoint
    /// from `fields`.
    coll_fields: Vec<String>,
    /// Fields written since the last boundary (re-anchor). Invariants whose
    /// fields are all untouched hold by assumption and are skipped.
    touched: HashSet<String>,
    /// Current version of each local (bumped on reassignment).
    local_ver: HashMap<String, u32>,
    next_ghost: u32,
}

/// Snapshot of the symbolic state, for branch merge logic.
#[derive(Debug, Clone)]
pub struct SymSnapshot {
    sym: HashMap<String, Affine>,
    touched: HashSet<String>,
    /// Per ensures spec (indexed like `InvariantScope::ensures`): did this
    /// path's end state prove the postcondition under the path's own facts
    /// (branch guards included — evaluated before the branch frame pops)?
    /// Consumed at branch joins (issue #455): a relation every surviving
    /// path proved holds of the joined state, because the runtime join
    /// state IS one of the surviving path states.
    ensures_ok: Vec<bool>,
}

impl InvariantScope {
    fn dirty(&self) -> bool {
        !self.touched.is_empty()
    }

    fn local_ghost(&self, name: &str) -> String {
        let v = self.local_ver.get(name).copied().unwrap_or(0);
        format!("{name}@{v}")
    }

    fn is_coll_field(&self, name: &str) -> bool {
        self.coll_fields.iter().any(|f| f == name)
    }

    /// The anchor-epoch ghost term for a field's current value: `self.f@n`
    /// for an int field, `self.f.len()@n` for a collection field (the
    /// `.len()` marker carries the automatic `>= 0` bound).
    fn field_ghost(&self, field: &str, n: u32) -> String {
        if self.is_coll_field(field) {
            format!("self.{field}.len()@{n}")
        } else {
            format!("self.{field}@{n}")
        }
    }

    /// Resolve a leaf expression into the ghost vocabulary: `self.f` reads
    /// its current symbolic value, an int local reads its current version
    /// ghost, and `xs.len()` on a collection-typed local reads an
    /// epoch-stamped length term. Anything else (foreign paths, calls) is
    /// unresolvable.
    pub(crate) fn resolve(&self, env: &TypeEnv, e: &Expr) -> Option<Affine> {
        match e {
            // `self.<coll>.len()` reads the field's current symbolic length.
            Expr::MethodCall { object, method, args, .. }
                if method.node == "len"
                    && args.is_empty()
                    && matches!(&object.node,
                        Expr::FieldAccess { object: o, field: f }
                            if matches!(&o.node, Expr::Ident(s) if s == "self")
                                && self.is_coll_field(&f.node)) =>
            {
                let Expr::FieldAccess { field, .. } = &object.node else { unreachable!() };
                self.sym.get(&field.node).cloned()
            }
            Expr::FieldAccess { object, field }
                // Only *int* fields read as a bare affine; a bare collection
                // field is not an integer (its length lives behind `.len()`).
                if matches!(&object.node, Expr::Ident(s) if s == "self")
                    && self.fields.iter().any(|f| f == &field.node) =>
            {
                self.sym.get(&field.node).cloned()
            }
            Expr::Ident(name) if name != "self" => {
                let is_int = match env.narrowed_vars.lookup(name) {
                    Some(t) => *t == PlutoType::Int,
                    None => matches!(env.lookup(name), Some(PlutoType::Int)),
                };
                if is_int {
                    Some(Affine::term(self.local_ghost(name)))
                } else {
                    None
                }
            }
            _ => self.resolve_len(env, e),
        }
    }

    /// Two-state resolver: like [`Self::resolve`], but additionally maps
    /// `old(e)` to the affine form of `e` over the *entry* ghost vocabulary
    /// (`self.f@0`, `p@0`). Entry ghosts are SSA values that are never
    /// re-anchored, so the mapping stays valid for the whole body.
    pub(crate) fn resolve_two_state(&self, env: &TypeEnv, e: &Expr) -> Option<Affine> {
        if let Some(inner) = old_call_arg(e) {
            return to_affine_with(&inner.node, &|x| self.resolve_entry(env, x));
        }
        self.resolve(env, e)
    }

    /// Resolve a leaf into the *entry* ghost vocabulary: the value a field or
    /// local had when the method was entered.
    fn resolve_entry(&self, env: &TypeEnv, e: &Expr) -> Option<Affine> {
        match e {
            // `old(self.<coll>.len())` — the field's entry-state length.
            Expr::MethodCall { object, method, args, .. }
                if method.node == "len"
                    && args.is_empty()
                    && matches!(&object.node,
                        Expr::FieldAccess { object: o, field: f }
                            if matches!(&o.node, Expr::Ident(s) if s == "self")
                                && self.is_coll_field(&f.node)) =>
            {
                let Expr::FieldAccess { field, .. } = &object.node else { unreachable!() };
                Some(Affine::term(format!("self.{}.len()@0", field.node)))
            }
            Expr::FieldAccess { object, field }
                if matches!(&object.node, Expr::Ident(s) if s == "self") =>
            {
                self.fields
                    .iter()
                    .any(|f| f == &field.node)
                    .then(|| Affine::term(format!("self.{}@0", field.node)))
            }
            Expr::Ident(name) if name != "self" => {
                let is_int = match env.narrowed_vars.lookup(name) {
                    Some(t) => *t == PlutoType::Int,
                    None => matches!(env.lookup(name), Some(PlutoType::Int)),
                };
                is_int.then(|| Affine::term(format!("{name}@0")))
            }
            _ => None,
        }
    }

    /// `xs.len()` on a collection-typed local, as a ghost term. Ghost facts
    /// are never killed, so the term is stamped with the current anchor
    /// epoch (`next_ghost`): every call boundary re-anchors and bumps the
    /// epoch, making facts recorded about the old term inert — sound even
    /// though a callee may grow or shrink the collection through an alias.
    /// The `.len()` marker keeps the automatic `>= 0` bound
    /// (facts.rs::is_len_term). Length of `self` fields is out of scope
    /// (their collection identity has no ghost tracking).
    fn resolve_len(&self, env: &TypeEnv, e: &Expr) -> Option<Affine> {
        let Expr::MethodCall { object, method, args, .. } = e else {
            return None;
        };
        if method.node != "len" || !args.is_empty() {
            return None;
        }
        let Expr::Ident(name) = &object.node else {
            return None;
        };
        if name == "self" {
            return None;
        }
        let is_collection = |t: &PlutoType| {
            matches!(
                t,
                PlutoType::Array(_)
                    | PlutoType::String
                    | PlutoType::Bytes
                    | PlutoType::Map(_, _)
                    | PlutoType::Set(_)
            )
        };
        let ok = match env.narrowed_vars.lookup(name) {
            Some(t) => is_collection(t),
            None => env.lookup(name).is_some_and(is_collection),
        };
        ok.then(|| {
            Affine::term(format!(
                "{}.len()@{}",
                self.local_ghost(name),
                self.next_ghost
            ))
        })
    }
}

/// Render an affine form over ghosts for diagnostics: `self.f@0` reads as
/// `old(self.f)`, later anchors and locals drop their version suffix.
fn render_affine(a: &Affine) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (path, c) in &a.terms {
        let base = match path.split_once('@') {
            Some((name, "0")) if name.starts_with("self.") => format!("old({name})"),
            Some((name, _)) => name.to_string(),
            None => path.clone(),
        };
        let term = match *c {
            1 => base,
            -1 => format!("-{base}"),
            c => format!("{c}*{base}"),
        };
        if parts.is_empty() {
            parts.push(term);
        } else if term.starts_with('-') {
            parts.push(format!("- {}", &term[1..]));
        } else {
            parts.push(format!("+ {term}"));
        }
    }
    if a.k != 0 || parts.is_empty() {
        if parts.is_empty() {
            parts.push(a.k.to_string());
        } else if a.k > 0 {
            parts.push(format!("+ {}", a.k));
        } else {
            parts.push(format!("- {}", -a.k));
        }
    }
    parts.join(" ")
}

/// The fields an expression mentions as `self.<field>` (including inside
/// `old(...)`).
fn mentioned_fields(expr: &Expr, out: &mut HashSet<String>) {
    if let Some(inner) = old_call_arg(expr) {
        mentioned_fields(&inner.node, out);
        return;
    }
    match expr {
        // `self.<coll>.len()` constrains the collection field `<coll>`.
        Expr::MethodCall { object, method, args, .. }
            if method.node == "len"
                && args.is_empty()
                && matches!(&object.node,
                    Expr::FieldAccess { object: o, .. }
                        if matches!(&o.node, Expr::Ident(s) if s == "self")) =>
        {
            if let Expr::FieldAccess { field, .. } = &object.node {
                out.insert(field.node.clone());
            }
        }
        Expr::FieldAccess { object, field }
            if matches!(&object.node, Expr::Ident(s) if s == "self") =>
        {
            out.insert(field.node.clone());
        }
        Expr::BinOp { lhs, rhs, .. } => {
            mentioned_fields(&lhs.node, out);
            mentioned_fields(&rhs.node, out);
        }
        Expr::UnaryOp { operand, .. } => mentioned_fields(&operand.node, out),
        _ => {}
    }
}

fn spec_fields(spec: &InvariantSpec) -> HashSet<String> {
    let mut out = HashSet::new();
    mentioned_fields(&spec.expr, &mut out);
    out
}

/// Rewrite an invariant expression's `self` root to another variable name
/// (`self.balance >= 0` → `acct.balance >= 0`). Only the provable fragment
/// shapes recurse; anything else is cloned unchanged.
fn rewrite_self(expr: &Expr, root: &str) -> Expr {
    match expr {
        Expr::Ident(s) if s == "self" => Expr::Ident(root.to_string()),
        Expr::FieldAccess { object, field } => Expr::FieldAccess {
            object: Box::new(Spanned::new(rewrite_self(&object.node, root), object.span)),
            field: field.clone(),
        },
        Expr::BinOp { op, lhs, rhs } => Expr::BinOp {
            op: *op,
            lhs: Box::new(Spanned::new(rewrite_self(&lhs.node, root), lhs.span)),
            rhs: Box::new(Spanned::new(rewrite_self(&rhs.node, root), rhs.span)),
        },
        Expr::UnaryOp { op, operand } => Expr::UnaryOp {
            op: *op,
            operand: Box::new(Spanned::new(rewrite_self(&operand.node, root), operand.span)),
        },
        // `self.<coll>.len()` — rewrite the receiver so the parameter's own
        // length term is produced (`p.coll.len()`), not `self`'s.
        Expr::MethodCall { object, method, args, type_args } => Expr::MethodCall {
            object: Box::new(Spanned::new(rewrite_self(&object.node, root), object.span)),
            method: method.clone(),
            args: args.clone(),
            type_args: type_args.clone(),
        },
        other => other.clone(),
    }
}

/// Marshal/wire glue is generated code deserializing external data — its
/// constructions are validated at runtime at the trust boundary instead
/// (see marshal.rs), so static obligations do not apply inside it.
fn is_exempt_fn(env: &TypeEnv) -> bool {
    match &env.current_fn {
        Some(name) => {
            let base = name.rsplit("::").next().unwrap_or(name);
            base.starts_with("__marshal_")
                || base.starts_with("__unmarshal_")
                || base.starts_with("__wire_encode_")
                || base.starts_with("__wire_decode_")
        }
        None => false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Proof primitives
// ─────────────────────────────────────────────────────────────────────────────

/// Prove every invariant whose fields were touched since the last boundary.
/// On success the state is clean (the invariant holds here); the caller
/// decides whether to re-anchor.
fn checkpoint(
    scope: &mut InvariantScope,
    env: &TypeEnv,
    span: Span,
    site: &str,
) -> Result<(), CompileError> {
    if !scope.dirty() {
        return Ok(());
    }
    if env.assume_discharged {
        // Monomorphize re-check of a template-proven body: the obligation
        // was already discharged under skolems (same param-independent
        // vocabulary), so treat the boundary as proven.
        scope.touched.clear();
        return Ok(());
    }
    for spec in &scope.invariants {
        let fields = spec_fields(spec);
        if fields.is_disjoint(&scope.touched) {
            // Untouched since the last boundary — holds by assumption.
            continue;
        }
        let s: &InvariantScope = scope;
        let verdict =
            eval_condition_with(&spec.expr, &|e| s.resolve_two_state(env, e), &s.ghost_facts);
        match verdict {
            Verdict::Proven => {}
            Verdict::Refuted => {
                return Err(CompileError::type_err(
                    format!(
                        "invariant '{}' of class '{}' is violated at {site} in method '{}': {}{}",
                        spec.desc,
                        scope.class_name,
                        scope.method_name,
                        symbolic_state(scope, &fields),
                        spec.blame(),
                    ),
                    span,
                ));
            }
            Verdict::Unknown => {
                return Err(CompileError::type_err(
                    format!(
                        "cannot prove invariant '{}' of class '{}' holds at {site} in \
                         method '{}': {}. The invariant may be broken temporarily between \
                         writes, but must be re-established at every boundary (method \
                         exits, raises, calls, loops, branch joins). Establish the missing \
                         bound before this point with a guard, a 'requires' clause, or an \
                         'assert' (e.g. 'if amt <= self.balance {{ ... }}'){}",
                        spec.desc,
                        scope.class_name,
                        scope.method_name,
                        symbolic_state(scope, &fields),
                        spec.blame(),
                    ),
                    span,
                ));
            }
        }
    }
    scope.touched.clear();
    Ok(())
}

fn symbolic_state(scope: &InvariantScope, fields: &HashSet<String>) -> String {
    let mut parts: Vec<String> = fields
        .iter()
        .filter(|f| scope.touched.contains(*f))
        .map(|f| {
            let rendered = scope
                .sym
                .get(f)
                .map(|a| render_affine(a))
                .unwrap_or_else(|| "<unknown>".to_string());
            if scope.is_coll_field(f) {
                format!("self.{f}.len() = {rendered}")
            } else {
                format!("self.{f} = {rendered}")
            }
        })
        .collect();
    parts.sort();
    if parts.is_empty() {
        "the known facts do not decide it".to_string()
    } else {
        format!("at this point {}", parts.join(", "))
    }
}

/// Is this ghost term an *entry*-state field term (`self.f@0`)? After any
/// re-anchor the current anchor epoch is >= 1, so `@0` field terms
/// unambiguously denote the method's entry state.
fn is_entry_field_term(path: &str) -> bool {
    path.starts_with("self.") && path.ends_with("@0")
}

/// Re-anchor every field on fresh ghosts carrying exactly the
/// invariant-level facts. Used at boundaries after the invariant has been
/// (re-)established, and after calls (which may invalidate anything finer).
///
/// `composed` distinguishes how the new anchor state arose. At a *composed*
/// boundary (a call, a yield, a select/scope block) the anchor state is the
/// result of other code running — code that itself preserves every invariant
/// *segment-wise* (each mut-self method relative to its own entry, each
/// foreign write relative to its pre-state). A two-state invariant therefore
/// survives a composed boundary only through transitivity, so only
/// composition-safe cross-state relations (`<`, `<=`, `==` between an entry
/// term and a current term) are assumed, and the two-state specs' fields are
/// marked touched so the next boundary re-proves the relation from those
/// facts (strictness: what transitivity cannot carry becomes a compile
/// error, never a silent hole). At a non-composed boundary (loop entry/exit)
/// the anchor state is a state the checker itself just proved the full
/// invariant for, so every extracted fact is sound.
fn re_anchor(scope: &mut InvariantScope, env: &TypeEnv, composed: bool) {
    scope.next_ghost += 1;
    let n = scope.next_ghost;
    let all_fields: Vec<String> = scope
        .fields
        .iter()
        .chain(scope.coll_fields.iter())
        .cloned()
        .collect();
    for f in &all_fields {
        let g = scope.field_ghost(f, n);
        scope.sym.insert(f.clone(), Affine::term(g));
    }
    scope.touched.clear();
    let mut to_assume = Vec::new();
    let mut touch: HashSet<String> = HashSet::new();
    {
        let s: &InvariantScope = scope;
        for spec in &s.invariants {
            let facts =
                condition_facts_with(&spec.expr, &|e| s.resolve_two_state(env, e)).then_facts;
            if spec.two_state && composed {
                to_assume.extend(facts.into_iter().filter(|fact| {
                    let cross_state = match fact {
                        Fact::Bound(p, _) => is_entry_field_term(p),
                        Fact::NeConst(p, _) => is_entry_field_term(p),
                        Fact::Rel(a, _, b) => {
                            is_entry_field_term(a) || is_entry_field_term(b)
                        }
                        // Membership facts are produced only by the
                        // idempotency pass, never by invariant conditions.
                        Fact::SetNotContains(..) | Fact::SetInserted(..) => false,
                    };
                    if !cross_state {
                        // Facts over current-anchor terms only: hold at the
                        // post-boundary state directly (every segment
                        // preserved the full invariant at its own exit).
                        return true;
                    }
                    // Cross-state facts survive composition only when the
                    // relation is transitive.
                    matches!(fact, Fact::Rel(_, RelOp::Lt | RelOp::Le | RelOp::Eq, _))
                }));
                touch.extend(spec_fields(spec));
            } else {
                to_assume.extend(facts);
            }
        }
    }
    for f in to_assume {
        scope.ghost_facts.assume(f);
    }
    scope.touched.extend(touch);
}

fn checkpoint_scope(env: &mut TypeEnv, span: Span, site: &str) -> Result<(), CompileError> {
    let Some(mut scope) = env.invariant_scope.take() else {
        return Ok(());
    };
    let r = checkpoint(&mut scope, env, span, site);
    env.invariant_scope = Some(scope);
    r
}

fn re_anchor_scope(env: &mut TypeEnv, composed: bool) {
    let Some(mut scope) = env.invariant_scope.take() else {
        return;
    };
    re_anchor(&mut scope, env, composed);
    env.invariant_scope = Some(scope);
}

/// Prove every ensures clause of the current method at a normal exit
/// (return / reachable fall-through). Raise paths owe nothing — the error
/// contract governs those edges (rfc-properties.md atom 1).
fn prove_ensures(
    scope: &InvariantScope,
    env: &TypeEnv,
    span: Span,
    site: &str,
) -> Result<(), CompileError> {
    if env.assume_discharged {
        // Template-proven (see checkpoint).
        return Ok(());
    }
    for spec in &scope.ensures {
        let verdict = eval_condition_with(
            &spec.expr,
            &|e| scope.resolve_two_state(env, e),
            &scope.ghost_facts,
        );
        let fields = {
            let mut out = HashSet::new();
            mentioned_fields(&spec.expr, &mut out);
            out
        };
        match verdict {
            Verdict::Proven => {}
            Verdict::Refuted => {
                return Err(CompileError::type_err(
                    format!(
                        "ensures clause '{}' of method '{}' of class '{}' is violated at \
                         {site}: {}{}",
                        spec.desc,
                        scope.method_name,
                        scope.class_name,
                        ensures_state(scope, &fields),
                        crate::parser::ast::provenance_blame(&spec.provenance),
                    ),
                    span,
                ));
            }
            Verdict::Unknown => {
                return Err(CompileError::type_err(
                    format!(
                        "cannot prove ensures clause '{}' of method '{}' of class '{}' at \
                         {site}: {}. An ensures clause must hold at every normal exit \
                         (returns and fall-through; raise paths are exempt). Establish the \
                         relation before this exit with a guard, a 'requires' clause, or an \
                         'assert' — and note that a call can invalidate two-state \
                         knowledge (the callee may reach the receiver through an alias){}",
                        spec.desc,
                        scope.method_name,
                        scope.class_name,
                        ensures_state(scope, &fields),
                        crate::parser::ast::provenance_blame(&spec.provenance),
                    ),
                    span,
                ));
            }
        }
    }
    Ok(())
}

/// Render the current symbolic value of every field an ensures clause
/// mentions (touched or not — the exit relation involves them all).
fn ensures_state(scope: &InvariantScope, fields: &HashSet<String>) -> String {
    let mut parts: Vec<String> = fields
        .iter()
        .filter_map(|f| {
            scope
                .sym
                .get(f)
                .map(|a| format!("self.{f} = {}", render_affine(a)))
        })
        .collect();
    parts.sort();
    if parts.is_empty() {
        "the known facts do not decide it".to_string()
    } else {
        format!("at this point {}", parts.join(", "))
    }
}

fn prove_ensures_scope(env: &mut TypeEnv, span: Span, site: &str) -> Result<(), CompileError> {
    let Some(scope) = env.invariant_scope.take() else {
        return Ok(());
    };
    let r = prove_ensures(&scope, env, span, site);
    env.invariant_scope = Some(scope);
    r
}

/// Re-assume invariant-level facts (in the main fact environment) for every
/// in-scope variable of an invariant-carrying class. Sound because every
/// write site is proven and boundary decodes are validated — an object's
/// invariant holds at every statement boundary, except for the enclosing
/// method's own receiver class while its state is dirty (an alias might be
/// mid-violation), which is skipped.
fn reassume_invariants_main(env: &mut TypeEnv) {
    if env.class_invariants.is_empty() {
        return;
    }
    let skip_class = env
        .invariant_scope
        .as_ref()
        .filter(|s| s.dirty())
        .map(|s| s.class_name.clone());
    let vars: Vec<(String, String)> = env
        .iter_variables()
        .filter_map(|(n, t)| match t {
            PlutoType::Class(c)
                if env.class_invariants.contains_key(c)
                    && !env.object_types.contains(c)
                    && Some(c) != skip_class.as_ref() =>
            {
                Some((n.clone(), c.clone()))
            }
            _ => None,
        })
        .collect();
    let mut to_assume = Vec::new();
    for (name, cls) in &vars {
        for spec in &env.class_invariants[cls] {
            let rewritten = rewrite_self(&spec.expr, name);
            to_assume.extend(condition_facts(&rewritten, env).then_facts);
        }
    }
    for f in to_assume {
        env.facts.assume(f);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Function entry / exit
// ─────────────────────────────────────────────────────────────────────────────

/// Set up entry facts and (for `mut self` methods of invariant-carrying
/// classes) the ghost proof scope. Called after parameters are defined.
pub(crate) fn function_entry(
    func: &Function,
    env: &mut TypeEnv,
    class_name: Option<&str>,
) -> Result<(), CompileError> {
    env.invariant_scope = None;
    if is_exempt_fn(env) {
        return Ok(());
    }

    // Main-env facts: requires clauses hold at entry (runtime-enforced),
    // and class-typed parameters satisfy their invariants.
    let mut assume = Vec::new();
    for c in &func.contracts {
        if c.node.kind != ContractKind::Requires || contains_impure_call(&c.node.expr, env) {
            continue;
        }
        assume.extend(condition_facts(&c.node.expr.node, env).then_facts);
    }
    if !env.class_invariants.is_empty() {
        for p in &func.params {
            let cls = match env.lookup(&p.name.node) {
                Some(PlutoType::Class(c)) => c.clone(),
                _ => continue,
            };
            if env.object_types.contains(&cls) {
                continue;
            }
            if let Some(specs) = env.class_invariants.get(&cls) {
                for spec in specs {
                    let rewritten = rewrite_self(&spec.expr, &p.name.node);
                    assume.extend(condition_facts(&rewritten, env).then_facts);
                }
            }
        }
    }
    for f in assume {
        env.facts.assume(f);
    }

    // Ghost proof scope: mut-self methods of invariant-carrying classes, and
    // any self method carrying ensures postconditions (the ensures proof
    // needs the symbolic entry vocabulary even when the class has no
    // invariant; the class invariants ride along as entry assumptions).
    let Some(cn) = class_name else { return Ok(()) };
    let has_self = func.params.iter().any(|p| p.name.node == "self");
    let has_mut_self = func
        .params
        .iter()
        .any(|p| p.name.node == "self" && p.is_mut);
    if !has_self {
        return Ok(());
    }
    let ensures_specs = env
        .fn_ensures
        .get(&mangle_method(cn, &func.name.node))
        .cloned()
        .unwrap_or_default();
    let inv_specs = env.class_invariants.get(cn).cloned().unwrap_or_default();
    if ensures_specs.is_empty() && (inv_specs.is_empty() || !has_mut_self) {
        return Ok(());
    }
    let fields: Vec<String> = env
        .classes
        .get(cn)
        .map(|ci| {
            ci.fields
                .iter()
                .filter(|(_, t, _)| *t == PlutoType::Int)
                .map(|(n, _, _)| n.clone())
                .collect()
        })
        .unwrap_or_default();
    let coll_fields: Vec<String> = env
        .invariant_collection_fields
        .get(cn)
        .map(|s| {
            let mut v: Vec<String> = s.iter().cloned().collect();
            v.sort();
            v
        })
        .unwrap_or_default();
    let mut scope = InvariantScope {
        class_name: cn.to_string(),
        method_name: func.name.node.clone(),
        invariants: inv_specs,
        ensures: ensures_specs,
        ghost_facts: FactEnv::new(),
        sym: HashMap::new(),
        fields,
        coll_fields,
        touched: HashSet::new(),
        local_ver: HashMap::new(),
        next_ghost: 0,
    };
    for f in &scope.fields {
        scope
            .sym
            .insert(f.clone(), Affine::term(format!("self.{f}@0")));
    }
    // Collection fields: the entry length ghost (`self.<f>.len()@0`, a len
    // term carrying the automatic `>= 0` bound).
    for f in scope.coll_fields.clone() {
        let g = scope.field_ghost(&f, 0);
        scope.sym.insert(f, Affine::term(g));
    }
    let mut ghost_assume = Vec::new();
    {
        let s: &InvariantScope = &scope;
        for spec in &s.invariants {
            ghost_assume.extend(
                condition_facts_with(&spec.expr, &|e| s.resolve_two_state(env, e)).then_facts,
            );
        }
        for c in &func.contracts {
            if c.node.kind != ContractKind::Requires || contains_impure_call(&c.node.expr, env) {
                continue;
            }
            ghost_assume
                .extend(condition_facts_with(&c.node.expr.node, &|e| s.resolve(env, e)).then_facts);
        }
    }
    for f in ghost_assume {
        scope.ghost_facts.assume(f);
    }
    env.invariant_scope = Some(scope);
    Ok(())
}

/// Prove the invariant — and, when the fall-through exit is reachable, the
/// ensures postconditions — at the method's end, then drop the proof scope.
/// Called after the body is checked. `fall_through` is false when every
/// path through the body returns or raises (the implicit exit is dead, so
/// no ensures obligation exists there — returns proved theirs already).
pub(crate) fn function_exit(
    env: &mut TypeEnv,
    span: Span,
    fall_through: bool,
) -> Result<(), CompileError> {
    let mut r = checkpoint_scope(env, span, "the end of this method");
    if r.is_ok() && fall_through {
        r = prove_ensures_scope(env, span, "the end of this method");
    }
    env.invariant_scope = None;
    r
}

// ─────────────────────────────────────────────────────────────────────────────
// Write-effect summaries (issue #454 — call-boundary write-freedom)
// ─────────────────────────────────────────────────────────────────────────────
//
// A call boundary has two obligations (module header): (a) the invariant
// must HOLD at the call (the callee, or anyone holding an alias, may
// observe the receiver), and (b) exact two-state knowledge is dropped to
// invariant level (the callee may WRITE the receiver through an alias).
// Obligation (b) is unnecessary when the callee provably cannot write any
// int field of the receiver's class. "Provably" is the hard part: Pluto's
// binding mutability is shallow — `let mut me = self` inside an
// immutable-`self` method mints a mutable alias of the receiver, so the
// callee's `self` mutability annotation ALONE does not bound its write
// effects. The proof therefore rests on a transitive, syntactic
// write-effect summary of the callee's body:
//
// - A `self.f = v` statement writes a field of the summarized method's own
//   class and nothing else (nominal typing: a D instance is never a C
//   instance).
// - ANY other field assignment (`me.f = v`, `self.a.b = v`, laundered
//   aliases included) conservatively counts as a write to every class.
// - Index assignment writes a container *slot* (an element reference or a
//   primitive), never a field of a class instance — there is no operator
//   overloading and no builtin follows references into class fields (the
//   load-bearing survey on `CallSeverity::Collections`).
// - Calls recurse: direct self-calls and field/param-receiver method calls
//   with syntactically known classes add call-graph edges; builtin
//   receivers (collections/primitives) are write-free for class fields by
//   the same survey; builtin free functions run no user code. Everything
//   else — trait dispatch, closures and fn-typed values (shadowed names),
//   `at`, `spawn`, `serve`, static trait calls, unknown receivers — is
//   opaque and poisons the summary.
// - Closure bodies are scanned as if they ran at the definition site
//   (over-conservative: their writes count even though they only run when
//   invoked).
//
// Summaries are built once, before body checking, from the flattened
// program (prelude and modules included). Generic templates are skipped —
// queries against template- or instantiation-mangled names miss and answer
// conservatively. The summaries describe the program AS WRITTEN; bodies
// that would fail later checks (e.g. illegal shadowing) may summarize
// imprecisely, which is harmless because such programs never compile.

/// Syntactic write-effect summary of one function/method body.
#[derive(Debug, Clone, Default)]
pub struct FnWriteSummary {
    /// The class whose fields `self.f = v` statements write (None for free
    /// functions, which have no `self`).
    owner: Option<String>,
    /// Body contains `self.f = v` (object is exactly the `self` ident).
    writes_own_fields: bool,
    /// Body contains any other field assignment — the target object's class
    /// is not syntactically known, so this counts as a write to EVERY class
    /// (laundered aliases land here).
    writes_other_fields: bool,
    /// Body contains a call whose effects cannot be bounded syntactically.
    opaque: bool,
    /// Resolved direct callees: mangled method names and free-function
    /// names, resolved at query time (builtins are write-free; unknown
    /// names are opaque).
    callees: HashSet<String>,
}

/// Build write-effect summaries for every non-generic function and class
/// method. Runs after signature registration and contract registration,
/// before any body checking; skipped entirely when the program carries no
/// contracts (no consumer exists).
pub(crate) fn summarize_write_effects(program: &Program, env: &mut TypeEnv) {
    if env.class_invariants.is_empty()
        && env.fn_ensures.is_empty()
        && env.generic_class_invariants.is_empty()
        && env.generic_class_ensures.is_empty()
    {
        return;
    }
    let mut out: HashMap<String, FnWriteSummary> = HashMap::new();
    for f in &program.functions {
        if !f.node.type_params.is_empty() {
            continue; // generic: queries miss → conservative
        }
        let name = f.node.name.node.clone();
        let summary = summarize_function(&name, &f.node, None, env);
        out.insert(name, summary);
    }
    for class in &program.classes {
        let c = &class.node;
        if !c.type_params.is_empty() {
            continue; // template bodies are proven under skolems; conservative here
        }
        for m in &c.methods {
            if !m.node.type_params.is_empty() {
                continue;
            }
            let key = mangle_method(&c.name.node, &m.node.name.node);
            let summary = summarize_function(&key, &m.node, Some(&c.name.node), env);
            out.insert(key, summary);
        }
    }
    env.fn_write_summaries = out;
}

/// Collect every name the body (or parameter list) can bind locally — the
/// shadow set for classifying `Call { name }` nodes (a call through a local
/// fn-typed value is opaque, not a call to the global of the same name).
fn collect_bound_names(func: &Function) -> HashSet<String> {
    struct Binders {
        names: HashSet<String>,
    }
    impl Visitor for Binders {
        fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
            match &stmt.node {
                Stmt::Let { name, .. } => {
                    self.names.insert(name.node.clone());
                }
                Stmt::LetChan { sender, receiver, .. } => {
                    self.names.insert(sender.node.clone());
                    self.names.insert(receiver.node.clone());
                }
                Stmt::For { var, .. } => {
                    self.names.insert(var.node.clone());
                }
                Stmt::Match { arms, .. } => {
                    for arm in arms {
                        if let crate::parser::ast::MatchPattern::Variant { bindings, .. } =
                            &arm.pattern
                        {
                            for (field, rename) in bindings {
                                let n = rename.as_ref().unwrap_or(field);
                                self.names.insert(n.node.clone());
                            }
                        }
                    }
                }
                Stmt::Select { arms, .. } => {
                    for arm in arms {
                        if let crate::parser::ast::SelectOp::Recv { binding, .. } = &arm.op {
                            self.names.insert(binding.node.clone());
                        }
                    }
                }
                Stmt::Scope { bindings, .. } => {
                    for b in bindings {
                        self.names.insert(b.name.node.clone());
                    }
                }
                _ => {}
            }
            walk_stmt(self, stmt);
        }
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            match &expr.node {
                Expr::Closure { params, .. } => {
                    for p in params {
                        self.names.insert(p.name.node.clone());
                    }
                }
                Expr::Catch { handlers, .. } => {
                    for h in handlers {
                        match h {
                            crate::parser::ast::CatchHandler::Wildcard { var, .. }
                            | crate::parser::ast::CatchHandler::Typed { var, .. } => {
                                self.names.insert(var.node.clone());
                            }
                            // `expr catch fallback` binds nothing.
                            crate::parser::ast::CatchHandler::Shorthand(_) => {}
                        }
                    }
                }
                Expr::Match { arms, .. } => {
                    for arm in arms {
                        if let crate::parser::ast::MatchPattern::Variant { bindings, .. } =
                            &arm.pattern
                        {
                            for (field, rename) in bindings {
                                let n = rename.as_ref().unwrap_or(field);
                                self.names.insert(n.node.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
            walk_expr(self, expr);
        }
    }
    let mut b = Binders { names: HashSet::new() };
    for p in &func.params {
        b.names.insert(p.name.node.clone());
    }
    for stmt in &func.body.node.stmts {
        b.visit_stmt(stmt);
    }
    b.names
}

/// Receiver classification for method calls inside a summarized body —
/// purely syntactic, from declared parameter types and the owner class's
/// field table.
enum RecvKind {
    /// The `self` ident: a sibling method of the owner class.
    SelfRecv,
    /// A collection/primitive: builtin methods only, write-free for class
    /// int fields (load-bearing survey on `CallSeverity::Collections`).
    Builtin,
    /// A known class: edge to that class's method.
    Known(String),
    /// Anything else (unknown locals, chained paths, traits, fn values...).
    Unknown,
}

fn classify_recv_type(t: &PlutoType) -> RecvKind {
    match t {
        PlutoType::Array(_)
        | PlutoType::String
        | PlutoType::Bytes
        | PlutoType::Map(_, _)
        | PlutoType::Set(_)
        | PlutoType::Range
        | PlutoType::Int
        | PlutoType::Float
        | PlutoType::Bool
        | PlutoType::Byte => RecvKind::Builtin,
        PlutoType::Class(c) => RecvKind::Known(c.clone()),
        _ => RecvKind::Unknown,
    }
}

fn summarize_function(
    key: &str,
    func: &Function,
    owner: Option<&str>,
    env: &TypeEnv,
) -> FnWriteSummary {
    // Param name → declared type, aligned positionally with the registered
    // signature (the receiver rides in params[0] for methods).
    let mut param_types: HashMap<String, PlutoType> = HashMap::new();
    if let Some(sig) = env.functions.get(key) {
        if sig.params.len() == func.params.len() {
            for (p, t) in func.params.iter().zip(sig.params.iter()) {
                param_types.insert(p.name.node.clone(), t.clone());
            }
        }
    }
    let bound = collect_bound_names(func);
    let owner_fields: Vec<(String, PlutoType)> = owner
        .and_then(|o| env.classes.get(o))
        .map(|ci| {
            ci.fields
                .iter()
                .map(|(n, t, _)| (n.clone(), t.clone()))
                .collect()
        })
        .unwrap_or_default();

    struct Scan<'a> {
        env: &'a TypeEnv,
        owner: Option<&'a str>,
        param_types: &'a HashMap<String, PlutoType>,
        owner_fields: &'a [(String, PlutoType)],
        bound: &'a HashSet<String>,
        s: FnWriteSummary,
    }
    impl Scan<'_> {
        fn recv_kind(&self, object: &Expr) -> RecvKind {
            match object {
                Expr::Ident(n) if n == "self" => RecvKind::SelfRecv,
                Expr::Ident(n) => match self.param_types.get(n) {
                    // Shadowing a param is illegal (define() rejects it), so
                    // the declared type is the binding's type.
                    Some(t) => classify_recv_type(t),
                    None => RecvKind::Unknown,
                },
                Expr::FieldAccess { object: o, field }
                    if matches!(&o.node, Expr::Ident(s) if s == "self") =>
                {
                    match self
                        .owner_fields
                        .iter()
                        .find(|(n, _)| *n == field.node)
                        .map(|(_, t)| t)
                    {
                        Some(t) => classify_recv_type(t),
                        None => RecvKind::Unknown,
                    }
                }
                _ => RecvKind::Unknown,
            }
        }
    }
    impl Visitor for Scan<'_> {
        fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
            if self.s.opaque && self.s.writes_other_fields {
                return; // already maximal
            }
            match &stmt.node {
                Stmt::FieldAssign { object, .. } => {
                    if matches!(&object.node, Expr::Ident(s) if s == "self") {
                        self.s.writes_own_fields = true;
                    } else {
                        self.s.writes_other_fields = true;
                    }
                }
                // Index assignment writes a container slot, never a field
                // of a class instance — see the section header.
                Stmt::IndexAssign { .. } => {}
                Stmt::Serve { .. } => {
                    // Registers methods for the runtime to invoke later.
                    self.s.opaque = true;
                }
                _ => {}
            }
            walk_stmt(self, stmt);
        }
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if self.s.opaque && self.s.writes_other_fields {
                return;
            }
            match &expr.node {
                Expr::MethodCall { object, method, .. } => {
                    match self.recv_kind(&object.node) {
                        RecvKind::SelfRecv => match self.owner {
                            Some(o) => {
                                self.s
                                    .callees
                                    .insert(mangle_method(o, &method.node));
                            }
                            None => self.s.opaque = true,
                        },
                        RecvKind::Builtin => {}
                        RecvKind::Known(c) => {
                            self.s.callees.insert(mangle_method(&c, &method.node));
                        }
                        RecvKind::Unknown => self.s.opaque = true,
                    }
                }
                Expr::Call { name, .. } => {
                    if self.bound.contains(&name.node) {
                        // May be a call through a local fn value.
                        self.s.opaque = true;
                    } else {
                        self.s.callees.insert(name.node.clone());
                    }
                }
                Expr::StaticTraitCall { .. } | Expr::At { .. } | Expr::Spawn { .. } => {
                    self.s.opaque = true;
                }
                _ => {}
            }
            walk_expr(self, expr);
        }
    }
    let mut scan = Scan {
        env,
        owner,
        param_types: &param_types,
        owner_fields: &owner_fields,
        bound: &bound,
        s: FnWriteSummary { owner: owner.map(str::to_string), ..Default::default() },
    };
    for stmt in &func.body.node.stmts {
        scan.visit_stmt(stmt);
    }
    scan.s
}

/// Can the callee named `start` — and everything it can transitively call —
/// be proven never to write any field of class `cls`? Walks the summarized
/// call graph; any edge that leaves the summarized world (unknown name,
/// opaque construct) answers `false`. A reachable `self.f = v` poisons only
/// when its owner IS `cls` (nominal typing); any other field assignment
/// poisons unconditionally (its target class is unknown — laundered
/// aliases of `cls` included).
pub(crate) fn callee_cannot_write_class(start: &str, cls: &str, env: &TypeEnv) -> bool {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut work: Vec<&str> = vec![start];
    while let Some(f) = work.pop() {
        if !seen.insert(f) {
            continue;
        }
        if env.builtins.contains(f) {
            // Runs no user code and cannot write class fields (the
            // load-bearing survey on CallSeverity::Collections).
            continue;
        }
        let Some(s) = env.fn_write_summaries.get(f) else {
            return false;
        };
        if s.opaque || s.writes_other_fields {
            return false;
        }
        if s.writes_own_fields && s.owner.as_deref() == Some(cls) {
            return false;
        }
        work.extend(s.callees.iter().map(String::as_str));
    }
    true
}

/// The #454 call-boundary refinement predicate: every call-like node of
/// `expr` provably cannot WRITE any int field of class `cls`. When this
/// holds for a whole statement, the call boundary keeps obligation (a) —
/// the invariant is proven at the call, an observer may see the receiver —
/// but skips the two-state havoc of obligation (b): the symbolic field
/// state and all entry-anchored (`old()`) knowledge survive the call.
///
/// # Soundness
///
/// The ghost scope tracks exactly the int fields of `cls`'s receiver. The
/// symbolic state desyncs from the runtime state only if some code that
/// runs DURING the statement writes an int field of a `cls` instance (the
/// receiver is one; alias-coarse by class, as everywhere in this engine).
/// Per call-like node:
/// - severity ≤ `Collections` nodes cannot write any int field of `cls`
///   by the shared classification (builtin receivers, callees whose
///   declared parameter/receiver types cannot reach `cls`);
/// - severity `All` method calls qualify only when the callee is a known,
///   non-`mut-self` method of `cls` itself, no non-receiver parameter's
///   declared type can reach `cls` (no writable alias handed in), AND the
///   transitive write-effect summary proves no reachable statement writes
///   a `cls` field through ANY binding — which closes the shallow-
///   mutability laundering doors (`let mut me = self`) that the signature
///   conditions alone cannot;
/// - everything else (trait dispatch, closures, fn values, `at`, `spawn`,
///   static trait calls, free functions handed possible aliases) fails the
///   predicate and keeps the full havoc.
///
/// A qualifying callee may still RETURN an alias of the receiver, or stash
/// one into a container passed as an argument — returning or stashing does
/// not write. Any later write through such an alias is a separate
/// statement the engine already sees: a foreign field assignment (proven
/// immediately, or rejected inside contract-carrying methods), or another
/// call boundary classified on its own. Deferred execution (closures
/// created by the callee, generator bodies) can only run via a later
/// call-like node, which is classified at ITS statement. Caller locals are
/// by-value ints (unwritable by any callee); ghost *length* terms about
/// collections the callee may mutate are retired by the anchor-epoch bump
/// at the call site.
fn calls_cannot_write_class(expr: &Spanned<Expr>, env: &TypeEnv, cls: &str) -> bool {
    struct Scan<'a> {
        env: &'a TypeEnv,
        cls: &'a str,
        ok: bool,
    }
    impl Scan<'_> {
        /// Does this severity-All method call meet the #454 conditions?
        fn refined_method_ok(
            &self,
            object: &Spanned<Expr>,
            method: &Spanned<String>,
        ) -> bool {
            let Some((_, PlutoType::Class(c))) = typed_path(&object.node, self.env) else {
                return false;
            };
            if c != self.cls {
                return false;
            }
            let mangled = mangle_method(&c, &method.node);
            if self.env.mut_self_methods.contains(&mangled) {
                return false;
            }
            let Some(sig) = self.env.functions.get(&mangled) else {
                return false;
            };
            // params[0] is the receiver; any OTHER parameter that can reach
            // `cls` may hand the callee a writable alias.
            if sig.params.iter().skip(1).any(|t| {
                super::facts::type_reaches_class(t, Some(self.cls), self.env)
            }) {
                return false;
            }
            callee_cannot_write_class(&mangled, self.cls, self.env)
        }
    }
    impl Visitor for Scan<'_> {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if !self.ok {
                return;
            }
            match &expr.node {
                Expr::MethodCall { .. } if len_path(&expr.node, self.env).is_some() => {
                    return; // pure length read (object is a trackable path)
                }
                Expr::MethodCall { object, method, .. } => {
                    let sev = super::facts::method_call_severity(
                        object,
                        method,
                        self.env,
                        Some(self.cls),
                    );
                    if sev == super::facts::CallSeverity::All
                        && !self.refined_method_ok(object, method)
                    {
                        self.ok = false;
                        return;
                    }
                }
                Expr::Call { name, args, .. } => {
                    let leaf = |e: &Expr| typed_path(e, self.env).map(|(_, t)| t);
                    let sev = super::facts::free_call_severity(
                        &name.node,
                        args,
                        self.env,
                        Some(self.cls),
                        &leaf,
                    );
                    if sev == super::facts::CallSeverity::All {
                        self.ok = false;
                        return;
                    }
                }
                Expr::StaticTraitCall { .. } | Expr::At { .. } | Expr::Spawn { .. } => {
                    self.ok = false;
                    return;
                }
                _ => {}
            }
            walk_expr(self, expr);
        }
    }
    let mut scan = Scan { env, cls, ok: true };
    scan.visit_expr(expr);
    scan.ok
}

// ─────────────────────────────────────────────────────────────────────────────
// Statement hooks
// ─────────────────────────────────────────────────────────────────────────────

/// Obligations that need the *pre-statement* fact state. Runs before
/// `apply_stmt_kills`.
pub(crate) fn pre_stmt(stmt: &Stmt, span: Span, env: &mut TypeEnv) -> Result<(), CompileError> {
    if (env.class_invariants.is_empty() && env.fn_ensures.is_empty()) || is_exempt_fn(env) {
        return Ok(());
    }

    // Caller-side ensures assumption (rfc-properties.md atom 1): when this
    // statement's single direct call targets a method with ensures, stage
    // the instantiated relation against the *pre-call* fact state (the
    // statement's kills have not run yet). Assumed by `post_stmt`.
    stage_call_ensures(stmt, env);

    // Call boundary — scaled by the shared purity classification
    // (facts::call_severity with the receiver's class as target):
    //
    // - `All` (the callee may reach the receiver — self-calls, methods on a
    //   possible alias, free functions whose declared params can reach the
    //   class, opaque callees): the callee (or anyone holding an alias) may
    //   observe the object, so the invariant must hold here, and afterwards
    //   only invariant-level facts survive — plus, for a single direct
    //   self-call with ensures, the callee's declared two-state relation.
    // - `Collections` (builtin methods, reach-free functions): the callee
    //   provably cannot read or write the receiver's int fields, so there
    //   is no observer and no writer — exact two-state knowledge survives
    //   (this is what lets `ensures count == old(count) + 1` prove through
    //   a trailing `print()`, and frame ensures prove through builtin calls
    //   on parameters). Only the ghost anchor epoch advances, so ghost
    //   *length* terms recorded before the call go inert (a builtin `push`
    //   on a local collection changes its length).
    // - `Pure`: nothing to do.
    if let Some(cls) = env.invariant_scope.as_ref().map(|s| s.class_name.clone()) {
        let sev = immediate_exprs(stmt)
            .iter()
            .map(|e| super::facts::call_severity(e, env, Some(&cls)))
            .max()
            .unwrap_or(super::facts::CallSeverity::Pure);
        match sev {
            super::facts::CallSeverity::Pure => {}
            super::facts::CallSeverity::Collections => {
                if let Some(s) = env.invariant_scope.as_mut() {
                    s.next_ghost += 1;
                }
            }
            super::facts::CallSeverity::All => {
                // Obligation (a): the callee — or anyone holding an alias —
                // may OBSERVE the receiver, so the invariant must hold here.
                checkpoint_scope(env, span, "this call (the callee may observe the object)")?;
                // Obligation (b) — refined (issue #454): exact two-state
                // knowledge survives the call when every call in this
                // statement provably cannot WRITE the receiver's int fields
                // (see calls_cannot_write_class for the soundness argument).
                // Only the ghost anchor epoch advances, retiring length
                // terms about collections the callee may mutate through its
                // arguments. Otherwise the callee may write the receiver:
                // drop to invariant-level knowledge, layering a single
                // direct self-callee's declared ensures relation on top.
                let read_only = immediate_exprs(stmt)
                    .iter()
                    .all(|e| calls_cannot_write_class(e, env, &cls));
                if read_only {
                    if let Some(s) = env.invariant_scope.as_mut() {
                        s.next_ghost += 1;
                    }
                } else {
                    apply_self_call_ensures(stmt, env);
                }
            }
        }
    }

    // Collection length transfer (rfc-number-types.md §4): `self.<coll>.push()`
    // / `pop()` / ... update the ghost length. Runs after the generic boundary
    // above so the delta composes onto the post-boundary length.
    apply_collection_mutations(stmt, span, env)?;

    match stmt {
        Stmt::FieldAssign { object, field, value } => {
            pre_field_assign(object, field, value, span, env)?;
        }
        Stmt::Let { name, .. } => {
            if let Some(s) = env.invariant_scope.as_mut() {
                *s.local_ver.entry(name.node.clone()).or_insert(0) += 1;
            }
        }
        Stmt::Assign { target, .. } => {
            if let Some(s) = env.invariant_scope.as_mut() {
                *s.local_ver.entry(target.node.clone()).or_insert(0) += 1;
            }
        }
        Stmt::Return(_) => {
            checkpoint_scope(env, span, "this return")?;
            prove_ensures_scope(env, span, "this return")?;
        }
        Stmt::Raise { .. } => checkpoint_scope(env, span, "this raise")?,
        Stmt::Break => checkpoint_scope(env, span, "this break")?,
        Stmt::Continue => checkpoint_scope(env, span, "this continue")?,
        Stmt::Yield { .. } => {
            checkpoint_scope(env, span, "this yield")?;
            re_anchor_scope(env, true);
        }
        Stmt::Select { .. } | Stmt::Scope { .. } => {
            if env.invariant_scope.is_some() && subtree_writes_self(stmt, span) {
                return Err(CompileError::type_err(
                    "writes to fields of the invariant-carrying receiver are not supported \
                     inside select/scope blocks; hoist the write out of the block"
                        .to_string(),
                    span,
                ));
            }
        }
        Stmt::If { .. }
        | Stmt::While { .. }
        | Stmt::For { .. }
        | Stmt::Match { .. }
        | Stmt::IndexAssign { .. }
        | Stmt::LetChan { .. }
        | Stmt::Assert { .. }
        | Stmt::ExpectRaises { .. }
        | Stmt::Serve { .. }
        | Stmt::Expr(_) => {}
    }
    Ok(())
}

/// Fact updates that follow a statement. Runs after the statement is fully
/// checked (so new bindings exist and branch merges are done).
pub(crate) fn post_stmt(stmt: &Stmt, env: &mut TypeEnv) -> Result<(), CompileError> {
    if (env.class_invariants.is_empty() && env.fn_ensures.is_empty()) || is_exempt_fn(env) {
        return Ok(());
    }
    let had_call = immediate_exprs(stmt)
        .iter()
        .any(|e| contains_impure_call(e, env));
    match stmt {
        Stmt::Let { name, value, .. } => {
            // A fresh class-typed binding satisfies its invariants (its
            // construction or producing call was proven/validated).
            if had_call {
                reassume_invariants_main(env);
            }
            ghost_len_binding(env, &name.node, &value.node);
            let cls = match env.lookup(&name.node) {
                Some(PlutoType::Class(c)) => c.clone(),
                _ => return Ok(()),
            };
            if env.class_invariants.contains_key(&cls) && !env.object_types.contains(&cls) {
                let skip = env
                    .invariant_scope
                    .as_ref()
                    .is_some_and(|s| s.dirty() && s.class_name == cls);
                if !skip {
                    let mut to_assume = Vec::new();
                    for spec in &env.class_invariants[&cls] {
                        let rewritten = rewrite_self(&spec.expr, &name.node);
                        to_assume.extend(condition_facts(&rewritten, env).then_facts);
                    }
                    for f in to_assume {
                        env.facts.assume(f);
                    }
                }
            }
        }
        Stmt::FieldAssign { .. } => reassume_invariants_main(env),
        Stmt::Assert { expr } => {
            // A passed assert establishes its condition for the rest of the
            // block — the documented escape hatch when proof falls short.
            // The main fact env already assumed the condition's facts
            // unconditionally (check.rs's Assert arm — asserts feed flow
            // facts in every function, invariants or not); only the ghost
            // mirror for invariant discharge lives here.
            if !contains_impure_call(expr, env) {
                if let Some(scope) = env.invariant_scope.take() {
                    let ghost = condition_facts_with(&expr.node, &|e| scope.resolve(env, e));
                    let mut scope = scope;
                    for f in ghost.then_facts {
                        scope.ghost_facts.assume(f);
                    }
                    env.invariant_scope = Some(scope);
                }
            }
        }
        Stmt::Select { .. } | Stmt::Scope { .. } => {
            re_anchor_scope(env, true);
            reassume_invariants_main(env);
        }
        Stmt::Assign { target, value } => {
            if had_call {
                reassume_invariants_main(env);
            }
            ghost_len_binding(env, &target.node, &value.node);
        }
        Stmt::If { .. }
        | Stmt::Match { .. }
        | Stmt::While { .. }
        | Stmt::For { .. }
        // The block may stop executing at any raising statement, so the
        // post-state is a join like a branch's.
        | Stmt::ExpectRaises { .. } => {
            // Branch-join / loop-exit anchoring: kills are flow events that
            // remove facts from *every* frame, but the invariant-level
            // reassumption a call triggers lands in the then-current
            // (branch-local) frame and pops with it. Without this, a call
            // inside a branch leaves the post-join state with no facts at
            // all about invariant-carrying objects — so a subsequent call's
            // ensures instantiation has no usable pre-state. Every object's
            // invariant holds at every statement boundary, so re-anchoring
            // the join (and the loop exit, whose havoc dropped everything)
            // at invariant level is sound.
            reassume_invariants_main(env);
        }
        Stmt::Return(_)
        | Stmt::IndexAssign { .. }
        | Stmt::Raise { .. }
        | Stmt::LetChan { .. }
        | Stmt::Yield { .. }
        | Stmt::Serve { .. }
        | Stmt::Break
        | Stmt::Continue
        | Stmt::Expr(_) => {
            if had_call {
                reassume_invariants_main(env);
            }
        }
    }
    // Caller-side ensures assumption staged by `pre_stmt`: the statement's
    // kills (and the invariant-level reassumption above) have run — layer
    // the callee's declared post-state relation on top.
    if !env.pending_call_ensures.is_empty() {
        let staged = std::mem::take(&mut env.pending_call_ensures);
        for f in staged {
            env.facts.assume(f);
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Caller-side ensures assumption (rfc-properties.md atom 1)
// ─────────────────────────────────────────────────────────────────────────────

/// Marker prefixed to pre-state (old) terms while splitting an instantiated
/// ensures relation into its pre-state and post-state parts. A control
/// character can never collide with a real path.
const OLD_MARK: char = '\u{1}';

/// Flatten a contract expression's top-level `&&` conjunction.
fn conjuncts(expr: &Expr) -> Vec<&Expr> {
    match expr {
        Expr::BinOp { op: BinOp::And, lhs, rhs } => {
            let mut v = conjuncts(&lhs.node);
            v.extend(conjuncts(&rhs.node));
            v
        }
        other => vec![other],
    }
}

/// The single, unconditionally-executed direct method call of a statement —
/// if the statement contains exactly one impure call-like node overall.
/// Conditional contexts (if/match expression blocks, catch), deferred
/// contexts (closures are exempt from the count — they run later), and
/// opaque contexts (spawn, at, static trait calls) disqualify the
/// statement, as do multiple calls (effect order within one statement is
/// not tracked). Returns a clone of the `Expr::MethodCall` node.
fn single_direct_method_call(stmt: &Stmt, env: &TypeEnv) -> Option<Expr> {
    struct Scan<'e> {
        env: &'e TypeEnv,
        count: usize,
        first: Option<Expr>,
        poisoned: bool,
    }
    impl Visitor for Scan<'_> {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if self.poisoned {
                return;
            }
            match &expr.node {
                // Runs later; its calls are not this statement's.
                Expr::Closure { .. } => return,
                Expr::Spawn { .. } | Expr::At { .. } | Expr::StaticTraitCall { .. } => {
                    self.poisoned = true;
                    return;
                }
                // Conditional execution / conditional continuation.
                Expr::If { .. } | Expr::Match { .. } | Expr::Catch { .. } => {
                    if contains_impure_call(expr, self.env) {
                        self.poisoned = true;
                    }
                    return;
                }
                Expr::MethodCall { .. } => {
                    if len_path(&expr.node, self.env).is_none() {
                        self.count += 1;
                        if self.first.is_none() {
                            self.first = Some(expr.node.clone());
                        }
                    }
                }
                Expr::Call { .. } => {
                    // A free-function call consumes the single-call budget
                    // but is never the assumed call (ensures live on methods).
                    self.count += 1;
                }
                _ => {}
            }
            walk_expr(self, expr);
        }
        fn visit_stmt(&mut self, _stmt: &Spanned<Stmt>) {
            // Nested statements are visited by the checker in flow order.
        }
    }
    let mut scan = Scan { env, count: 0, first: None, poisoned: false };
    for e in immediate_exprs(stmt) {
        scan.visit_expr(e);
        if scan.poisoned {
            return None;
        }
    }
    if scan.poisoned || scan.count != 1 {
        return None;
    }
    scan.first.filter(|f| matches!(f, Expr::MethodCall { .. }))
}

/// Could evaluating this argument yield a value that aliases an instance of
/// class `cname` — and so hand the callee the call's own receiver (issue
/// #417)? Conservative: anything whose type cannot be cheaply determined
/// answers yes. Struct literals are fresh objects (the callee's parameter
/// binds the fresh object, never the receiver), and primitive-typed results
/// can never alias a class instance. Container arguments answer through
/// their element types ([C] hands the callee aliases of every element).
fn arg_may_alias_class(e: &Expr, env: &TypeEnv, cname: &str) -> bool {
    use super::facts::type_reaches_class;
    if let Some((_, t)) = typed_path(e, env) {
        return type_reaches_class(&t, Some(cname), env);
    }
    if len_path(e, env).is_some() {
        return false; // xs.len(): int
    }
    match e {
        Expr::IntLit(_)
        | Expr::FloatLit(_)
        | Expr::BoolLit(_)
        | Expr::StringLit(_)
        | Expr::NoneLit
        | Expr::EnumUnit { .. }
        | Expr::Range { .. }
        | Expr::Closure { .. }
        | Expr::ClosureCreate { .. }
        | Expr::StringInterp { .. } => false,
        // A struct literal is a fresh object: it cannot BE the receiver
        // (writes through the callee's parameter land on the fresh object).
        Expr::StructLit { .. } => false,
        // No operator overloading: operators yield primitives.
        Expr::BinOp { .. } | Expr::UnaryOp { .. } | Expr::CompareChain { .. } => false,
        Expr::NullCoalesce { lhs, rhs } => {
            arg_may_alias_class(&lhs.node, env, cname)
                || arg_may_alias_class(&rhs.node, env, cname)
        }
        Expr::Propagate { expr } | Expr::NullPropagate { expr } => {
            arg_may_alias_class(&expr.node, env, cname)
        }
        Expr::Call { name, .. } => match env.functions.get(&name.node) {
            Some(sig) => type_reaches_class(&sig.return_type, Some(cname), env),
            None => true,
        },
        Expr::MethodCall { object, method, .. } => {
            match typed_path(&object.node, env) {
                Some((_, PlutoType::Class(c2))) => {
                    match env.functions.get(&mangle_method(&c2, &method.node)) {
                        Some(sig) => type_reaches_class(&sig.return_type, Some(cname), env),
                        None => true,
                    }
                }
                _ => true,
            }
        }
        Expr::Index { object, .. } => match typed_path(&object.node, env) {
            Some((_, PlutoType::Array(el))) => type_reaches_class(&el, Some(cname), env),
            Some((_, PlutoType::Map(_, v))) => type_reaches_class(&v, Some(cname), env),
            _ => true,
        },
        Expr::ArrayLit { elements, .. } | Expr::SetLit { elements, .. } => elements
            .iter()
            .any(|el| arg_may_alias_class(&el.node, env, cname)),
        Expr::MapLit { entries, .. } => entries.iter().any(|(k, v)| {
            arg_may_alias_class(&k.node, env, cname)
                || arg_may_alias_class(&v.node, env, cname)
        }),
        Expr::EnumData { fields, .. } => fields
            .iter()
            .any(|(_, v)| arg_may_alias_class(&v.node, env, cname)),
        // Everything else (casts, trait calls, at/spawn, conditionals,
        // catch, bare idents typed_path could not resolve — entities
        // included): conservatively yes.
        Expr::Cast { .. }
        | Expr::StaticTraitCall { .. }
        | Expr::At { .. }
        | Expr::Spawn { .. }
        | Expr::If { .. }
        | Expr::Match { .. }
        | Expr::Catch { .. }
        | Expr::Ident(_)
        | Expr::FieldAccess { .. }
        | Expr::QualifiedAccess { .. } => true,
    }
}

/// Stage the caller-side assumption of a callee's ensures relation (main
/// fact environment — trackable non-entity receivers). Must run against the
/// *pre-call* fact state, before `apply_stmt_kills`; `post_stmt` assumes the
/// staged facts once the statement's kills have been applied.
///
/// The relation is instantiated by substitution (receiver path for `self`,
/// argument affines over caller locals for parameters — mirroring
/// `shrink.rs`), then each comparison conjunct `C + O cmp 0` (post-state
/// part `C`, pre-state part `O`) is folded into post-state facts by bounding
/// `O` against the pre-call facts. Conservative everywhere: unresolvable
/// conjuncts stage nothing.
fn stage_call_ensures(stmt: &Stmt, env: &mut TypeEnv) {
    env.pending_call_ensures.clear();
    if env.fn_ensures.is_empty() {
        return;
    }
    let Some(call) = single_direct_method_call(stmt, env) else {
        return;
    };
    let Expr::MethodCall { object, method, args, .. } = &call else {
        return;
    };
    let Some((rpath, PlutoType::Class(cname))) = typed_path(&object.node, env) else {
        return;
    };
    // Entities mutate concurrently — their fields never carry flow facts.
    if env.object_types.contains(&cname)
        || env.remote_types.contains(&cname)
        || env.domain_types.contains(&cname)
    {
        return;
    }
    // The enclosing method's own receiver is handled in the ghost
    // vocabulary (`apply_self_call_ensures`).
    if rpath == "self"
        && env
            .invariant_scope
            .as_ref()
            .is_some_and(|s| s.class_name == cname)
    {
        return;
    }
    // A statement that rebinds the receiver's root invalidates the path.
    let root = rpath.split('.').next().unwrap_or(&rpath).to_string();
    match stmt {
        Stmt::Let { name, .. } if name.node == root => return,
        Stmt::Assign { target, .. } if target.node == root => return,
        _ => {}
    }
    // Path-denotation stability (issue #417): the staged relation is about
    // the OBJECT the receiver path denotes at call time, but the facts are
    // phrased on the path. A dotted path (`a.f.owner`) can be re-pointed by
    // the callee writing a class-typed field of an intermediate object it
    // reaches — the post-call facts would then describe the wrong object.
    // A bare binding cannot be rebound by any callee, so only those stage.
    if rpath.contains('.') {
        return;
    }
    // Receiver aliasing (issue #417): when any argument may carry a value
    // of the receiver's class, the callee's `other` parameter may BE the
    // receiver. Accepted contract bodies are proven alias-safe (same-class
    // foreign writes are rejected above), so the declared relation still
    // holds of the object — but the conservative skip costs only
    // completeness and keeps the caller-side assumption independent of
    // that argument, matching the engine's alias-coarse discipline.
    if args.iter().any(|a| arg_may_alias_class(&a.node, env, &cname)) {
        return;
    }
    let Some(specs) = env.fn_ensures.get(&mangle_method(&cname, &method.node)) else {
        return;
    };
    let specs = specs.clone();
    let params = &specs[0].params;
    if params.len() != args.len() {
        return;
    }
    // Arguments evaluate before the call; only affines over caller *locals*
    // (which no callee can change) are stable across it.
    let mut arg_aff: HashMap<String, Option<Affine>> = HashMap::new();
    for (p, a) in params.iter().zip(args.iter()) {
        let aff = to_affine(&a.node, env)
            .filter(|af| af.terms.keys().all(|t| !t.contains('.')));
        arg_aff.insert(p.clone(), aff);
    }
    let resolve = |e: &Expr| -> Option<Affine> {
        if let Some(inner) = old_call_arg(e) {
            return to_affine_with(&inner.node, &|x| match x {
                Expr::FieldAccess { object: o, field: f }
                    if matches!(&o.node, Expr::Ident(s) if s == "self") =>
                {
                    Some(Affine::term(format!("{OLD_MARK}{rpath}.{}", f.node)))
                }
                Expr::Ident(p) if p != "self" => arg_aff.get(p).cloned().flatten(),
                _ => None,
            });
        }
        match e {
            Expr::FieldAccess { object: o, field: f }
                if matches!(&o.node, Expr::Ident(s) if s == "self") =>
            {
                Some(Affine::term(format!("{rpath}.{}", f.node)))
            }
            Expr::Ident(p) if p != "self" => arg_aff.get(p).cloned().flatten(),
            _ => None,
        }
    };
    let mut staged: Vec<Fact> = Vec::new();
    for spec in &specs {
        for conj in conjuncts(&spec.expr) {
            let Expr::BinOp { op, lhs, rhs } = conj else { continue };
            if !is_comparison(*op) {
                continue;
            }
            let (Some(l), Some(r)) = (
                to_affine_with(&lhs.node, &resolve),
                to_affine_with(&rhs.node, &resolve),
            ) else {
                continue;
            };
            let Some(d) = diff_affine(&l, &r) else { continue };
            let mut cur = Affine::constant(d.k);
            let mut pre = Affine::constant(0);
            for (t, c) in &d.terms {
                if let Some(stripped) = t.strip_prefix(OLD_MARK) {
                    pre.terms.insert(stripped.to_string(), *c);
                } else {
                    cur.terms.insert(t.clone(), *c);
                }
            }
            if pre.terms.is_empty() {
                // Pure post-state conjunct — holds verbatim after the call.
                staged.extend(facts_from_diff(*op, &d));
                continue;
            }
            let Ok((plo, phi)) = affine_bounds(&pre, &env.facts) else {
                continue;
            };
            // `cur + pre cmp 0` with `pre ∈ [plo, phi]` implies the
            // post-state inequality with the pre-state part replaced by the
            // appropriate bound.
            match op {
                BinOp::Gt | BinOp::GtEq => {
                    if let Some(hi) = phi {
                        if let Some(dd) = affine_add_const(&cur, hi) {
                            staged.extend(facts_from_diff(*op, &dd));
                        }
                    }
                }
                BinOp::Lt | BinOp::LtEq => {
                    if let Some(lo) = plo {
                        if let Some(dd) = affine_add_const(&cur, lo) {
                            staged.extend(facts_from_diff(*op, &dd));
                        }
                    }
                }
                BinOp::Eq => {
                    if plo.is_some() && plo == phi {
                        if let Some(dd) = affine_add_const(&cur, plo.expect("checked")) {
                            staged.extend(facts_from_diff(BinOp::Eq, &dd));
                        }
                    } else {
                        if let Some(hi) = phi {
                            if let Some(dd) = affine_add_const(&cur, hi) {
                                staged.extend(facts_from_diff(BinOp::GtEq, &dd));
                            }
                        }
                        if let Some(lo) = plo {
                            if let Some(dd) = affine_add_const(&cur, lo) {
                                staged.extend(facts_from_diff(BinOp::LtEq, &dd));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    env.pending_call_ensures = staged;
}

/// Ghost-vocabulary twin of [`stage_call_ensures`]: after the call-boundary
/// re-anchor inside a proof scope, a single direct `self.m(...)` call whose
/// callee declares ensures re-establishes the declared relation between the
/// fresh anchor ghosts and the pre-call symbolic state. Equality clauses
/// (`self.f == <expr over old()/params>`) become symbolic strong updates —
/// which is what lets a method's own ensures proof see through calls to
/// sibling methods; other clauses contribute ghost facts.
fn apply_self_call_ensures(stmt: &Stmt, env: &mut TypeEnv) {
    let Some(pre_sym) = env.invariant_scope.as_ref().map(|s| s.sym.clone()) else {
        return;
    };
    re_anchor_scope(env, true);
    let Some(call) = single_direct_method_call(stmt, env) else {
        return;
    };
    let Expr::MethodCall { object, method, args, .. } = &call else {
        return;
    };
    if !matches!(&object.node, Expr::Ident(s) if s == "self") {
        return;
    }
    let Some(scope_ref) = env.invariant_scope.as_ref() else {
        return;
    };
    let receiver_class = scope_ref.class_name.clone();
    let callee = mangle_method(&receiver_class, &method.node);
    let Some(specs) = env.fn_ensures.get(&callee).cloned() else {
        return;
    };
    if specs[0].params.len() != args.len() {
        return;
    }
    // Receiver aliasing (issue #417): mirror of the stage_call_ensures
    // skip — when an argument may carry another binding of the receiver's
    // class, do not pin the callee's relation onto the ghost state.
    if args
        .iter()
        .any(|a| arg_may_alias_class(&a.node, env, &receiver_class))
    {
        return;
    }
    let mut scope = env.invariant_scope.take().expect("checked above");

    // The callee's entry state is the caller's pre-call state; arguments
    // evaluate pre-call over caller locals (unchanged by any callee).
    let mut arg_aff: HashMap<String, Option<Affine>> = HashMap::new();
    for (p, a) in specs[0].params.iter().zip(args.iter()) {
        let aff = to_affine_with(&a.node, &|x| match x {
            Expr::FieldAccess { object: o, field: f }
                if matches!(&o.node, Expr::Ident(s) if s == "self") =>
            {
                pre_sym.get(&f.node).cloned()
            }
            Expr::Ident(_) => scope.resolve(env, x),
            _ => None,
        });
        arg_aff.insert(p.clone(), aff);
    }
    let old_resolve = |x: &Expr| -> Option<Affine> {
        match x {
            Expr::FieldAccess { object: o, field: f }
                if matches!(&o.node, Expr::Ident(s) if s == "self") =>
            {
                pre_sym.get(&f.node).cloned()
            }
            Expr::Ident(p) if p != "self" => arg_aff.get(p).cloned().flatten(),
            _ => None,
        }
    };

    // 1) Equality clauses pin the field's post-call symbolic value.
    for spec in &specs {
        for conj in conjuncts(&spec.expr) {
            let Expr::BinOp { op: BinOp::Eq, lhs, rhs } = conj else { continue };
            let self_field = |e: &Expr| -> Option<String> {
                match e {
                    Expr::FieldAccess { object: o, field: f }
                        if matches!(&o.node, Expr::Ident(s) if s == "self") =>
                    {
                        Some(f.node.clone())
                    }
                    _ => None,
                }
            };
            let (field, value_side) = match (self_field(&lhs.node), self_field(&rhs.node)) {
                (Some(f), None) => (f, &rhs.node),
                (None, Some(f)) => (f, &lhs.node),
                _ => continue,
            };
            if !scope.fields.iter().any(|fl| fl == &field) {
                continue;
            }
            let resolver = |x: &Expr| -> Option<Affine> {
                if let Some(inner) = old_call_arg(x) {
                    return to_affine_with(&inner.node, &old_resolve);
                }
                match x {
                    Expr::Ident(p) if p != "self" => arg_aff.get(p).cloned().flatten(),
                    _ => None,
                }
            };
            if let Some(aff) = to_affine_with(value_side, &resolver) {
                scope.sym.insert(field, aff);
                // `touched` stays as the re-anchor left it: fields of
                // two-state invariants re-prove at the next boundary (now
                // with an exact symbolic value); single-state-only fields
                // hold by the callee's proven exit invariant.
            }
        }
    }

    // 2) Every clause contributes ghost facts relating the (possibly pinned)
    //    current state to the pre-call state.
    let mut to_assume = Vec::new();
    {
        let s: &InvariantScope = &scope;
        let fact_resolver = |x: &Expr| -> Option<Affine> {
            if let Some(inner) = old_call_arg(x) {
                return to_affine_with(&inner.node, &old_resolve);
            }
            match x {
                Expr::FieldAccess { .. } => s.resolve(env, x),
                Expr::Ident(p) if p != "self" => arg_aff.get(p).cloned().flatten(),
                _ => None,
            }
        };
        for spec in &specs {
            to_assume.extend(condition_facts_with(&spec.expr, &fact_resolver).then_facts);
        }
    }
    for f in to_assume {
        scope.ghost_facts.assume(f);
    }
    env.invariant_scope = Some(scope);
}

/// Ghost binding transfer: a direct `xs.len()` binding gives the bound
/// local's current ghost the automatic `>= 0` bound and an equality to the
/// epoch-stamped length term (mirror of the main-env transfer in
/// facts.rs::binding_facts; the local's version was already bumped in
/// `pre_stmt`).
fn ghost_len_binding(env: &mut TypeEnv, name: &str, value: &Expr) {
    let Some(mut scope) = env.invariant_scope.take() else {
        return;
    };
    if let Some(a) = scope.resolve_len(env, value) {
        let ghost = scope.local_ghost(name);
        scope
            .ghost_facts
            .assume(Fact::Bound(ghost.clone(), Interval::at_least(0)));
        if a.terms.len() == 1 && a.k == 0 {
            if let Some((term, &c)) = a.terms.iter().next() {
                if c == 1 {
                    scope
                        .ghost_facts
                        .assume(Fact::Rel(ghost, RelOp::Eq, term.clone()));
                }
            }
        }
    }
    env.invariant_scope = Some(scope);
}

/// Field-write obligations. Inside the class's own `mut self` method a
/// write to `self.field` is a symbolic strong update (proof deferred to the
/// next boundary); any other write to an invariant-carrying class must
/// prove the invariant immediately.
fn pre_field_assign(
    object: &Spanned<Expr>,
    field: &Spanned<String>,
    value: &Spanned<Expr>,
    span: Span,
    env: &mut TypeEnv,
) -> Result<(), CompileError> {
    // An illegal write (immutable binding) is rejected by the mutability
    // checks with a better message — no proof obligation on it.
    if let Some(root) = super::check::root_variable(&object.node) {
        if root != "self" && env.is_immutable(root) {
            return Ok(());
        }
    }
    let resolved = typed_path(&object.node, env);
    let (opath, cls) = match resolved {
        Some((p, PlutoType::Class(c))) => (Some(p), c),
        Some(_) => return Ok(()),
        None => {
            // Untrackable target (index chains, entity-nested paths):
            // determine the class to know whether an obligation exists.
            let t = super::infer::infer_expr(&object.node, object.span, env, None)?;
            match t {
                PlutoType::Class(c) => (None, c),
                _ => return Ok(()),
            }
        }
    };
    // Strong update inside the class's own mut-self method — also when the
    // class carries no invariant (an ensures-only proof scope still tracks
    // field values symbolically).
    if opath.as_deref() == Some("self") {
        if let Some(scope) = env.invariant_scope.as_ref() {
            if scope.class_name == cls {
                // Door (a), applied to reassignment: a covered collection
                // field may only be set from a syntactically fresh
                // expression. Binding an alias in would let later mutations
                // of that alias desync the length ghost (rfc-number-types.md
                // §4 closing-the-doors).
                if scope.is_coll_field(&field.node) && !is_fresh_collection_expr(&value.node) {
                    return Err(aliasing_init_error(&cls, &field.node, "assignment", span));
                }
                let specs = env.class_invariants.get(&cls).cloned().unwrap_or_default();
                let mut scope = env.invariant_scope.take().expect("checked above");
                strong_update(&mut scope, env, &field.node, &value.node, &specs);
                env.invariant_scope = Some(scope);
                return Ok(());
            }
        } else {
            // `self` writes without a proof scope are either illegal
            // (non-mut method — reported by the mutability checks) or a
            // different class's method; nothing to prove here.
            return Ok(());
        }
    }

    // Receiver aliasing (issue #417): inside a contract-carrying method,
    // a write through any OTHER binding of the receiver's own class may go
    // through an alias of `self` (classes alias freely within a task —
    // `c.bump2(c)`, self-referential fields, containers). The ghost scope
    // tracks `self`'s int fields as strong symbolic updates, so a foreign
    // write that may actually land on the receiver would silently desync
    // the symbolic state from the real state and certify false contracts
    // (the entry-anchored `old()` relations included). The engine's alias
    // discipline everywhere else (facts::call_severity, dominance's
    // `kill_field_write`) is alias-coarse by class; the matching answer
    // here is to reject the write outright — havocking instead would push
    // the failure to the next boundary with a diagnostic that no longer
    // names the aliasing binding.
    if let Some(scope) = env.invariant_scope.as_ref() {
        if scope.class_name == cls && scope.fields.iter().any(|f| f == &field.node) {
            let target = opath
                .clone()
                .unwrap_or_else(|| "<expression>".to_string());
            let root = target
                .split('.')
                .next()
                .unwrap_or(target.as_str())
                .to_string();
            let mut clauses: Vec<String> = scope
                .invariants
                .iter()
                .map(|s| format!("invariant '{}'{}", s.desc, s.blame()))
                .collect();
            clauses.extend(scope.ensures.iter().map(|s| {
                format!(
                    "ensures '{}'{}",
                    s.desc,
                    crate::parser::ast::provenance_blame(&s.provenance)
                )
            }));
            let clause_list = clauses.join(", ");
            return Err(CompileError::type_err(
                format!(
                    "cannot discharge the contracts of method '{}' of class '{cls}': \
                     this write to '{target}.{}' goes through '{root}', another '{cls}' \
                     binding that may alias 'self'. A write through an alias would \
                     invalidate the symbolic tracking of 'self.{}' that the proof of \
                     {clause_list} depends on. Perform the mutation through 'self', or \
                     move the aliasing write out of this contract-carrying method",
                    scope.method_name, field.node, field.node
                ),
                span,
            ));
        }
    }

    if env.assume_discharged {
        // Foreign-write obligations in monomorphized copies were proven at
        // the template (see checkpoint).
        return Ok(());
    }
    let Some(specs) = env.class_invariants.get(&cls) else {
        return Ok(());
    };
    let specs = specs.clone();

    // A write to a field no invariant mentions cannot break any invariant.
    let mentioned = specs
        .iter()
        .any(|s| spec_fields(s).contains(&field.node));
    if !mentioned {
        return Ok(());
    }

    let Some(opath) = opath else {
        return Err(CompileError::type_err(
            format!(
                "cannot prove invariant of class '{cls}' is preserved: the assignment \
                 target is not a trackable path. Bind the object to a local variable \
                 first (let mut o = ...; o.{} = ...)",
                field.node
            ),
            span,
        ));
    };

    // Immediate obligation: evaluate each invariant with the assigned field
    // substituted by the new value and the other fields read through the
    // object's path (whose facts are the entry/boundary assumptions). For
    // two-state invariants, `old(...)` at a foreign write site denotes the
    // *pre-write* state (rfc-properties.md open question 2): the current
    // path values, which is exactly what the pre-kill facts describe.
    let val_aff = to_affine(&value.node, env);
    for spec in &specs {
        let resolve = |e: &Expr| {
            if let Some(inner) = old_call_arg(e) {
                return to_affine_with(&inner.node, &|x| match x {
                    Expr::FieldAccess { object: o, field: f }
                        if matches!(&o.node, Expr::Ident(s) if s == "self") =>
                    {
                        Some(Affine::term(format!("{opath}.{}", f.node)))
                    }
                    _ => None,
                });
            }
            match e {
                Expr::FieldAccess { object: o, field: f }
                    if matches!(&o.node, Expr::Ident(s) if s == "self") =>
                {
                    if f.node == field.node {
                        val_aff.clone()
                    } else {
                        Some(Affine::term(format!("{opath}.{}", f.node)))
                    }
                }
                _ => None,
            }
        };
        match eval_condition_with(&spec.expr, &resolve, &env.facts) {
            Verdict::Proven => {}
            Verdict::Refuted => {
                return Err(CompileError::type_err(
                    format!(
                        "this write to '{opath}.{}' violates invariant '{}' of class '{cls}'{}",
                        field.node, spec.desc, spec.blame()
                    ),
                    span,
                ));
            }
            Verdict::Unknown => {
                return Err(CompileError::type_err(
                    format!(
                        "cannot prove invariant '{}' of class '{cls}' after this write to \
                         '{opath}.{}'. Writes from outside the class's own methods must \
                         preserve the invariant immediately; add a guard or 'assert' \
                         establishing the needed bound before the write (e.g. \
                         'if v >= 0 {{ {opath}.{} = v }}'), or move the write into a \
                         'mut self' method of '{cls}'{}",
                        spec.desc, field.node, field.node, spec.blame()
                    ),
                    span,
                ));
            }
        }
    }
    Ok(())
}

/// The exact element count of a collection *literal* (`[a, b]`, `{...}`,
/// `{k: v}`), or `None` for any other (non-literal) fresh producer whose
/// length the prover does not compute.
fn collection_expr_len(expr: &Expr) -> Option<i128> {
    match expr {
        Expr::ArrayLit { elements } => Some(elements.len() as i128),
        Expr::SetLit { elements, .. } => Some(elements.len() as i128),
        Expr::MapLit { entries, .. } => Some(entries.len() as i128),
        // `bytes_new()` → 0; `bytes_filled(n, _)` → n when n is a literal.
        Expr::Call { name, args, .. } if name.node == "bytes_new" && args.is_empty() => Some(0),
        Expr::Call { name, args, .. } if name.node == "bytes_filled" && args.len() == 2 => {
            match &args[0].node {
                Expr::IntLit(n) => Some(*n as i128),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Door (a): is this a *syntactically fresh* collection — one that cannot
/// already be aliased by another binding? Conservative by design: a
/// collection literal, or a non-mutating builtin method that allocates and
/// returns a NEW collection (`slice`, `keys`, `values`, `to_array`). Anything
/// else — a bare variable, a field read, a free-function call (whose result
/// the callee may also retain) — is rejected, since the length ghost is only
/// sound while the class holds the sole reference. Widening this set is a
/// follow-up gated on escape analysis (rfc-number-types.md §4, door 1/3).
fn is_fresh_collection_expr(expr: &Expr) -> bool {
    match expr {
        Expr::ArrayLit { .. } | Expr::SetLit { .. } | Expr::MapLit { .. } => true,
        Expr::MethodCall { method, .. } => {
            matches!(method.node.as_str(), "slice" | "keys" | "values" | "to_array")
        }
        // Bytes have no literal form; these builtins allocate a fresh buffer.
        Expr::Call { name, .. } => matches!(name.node.as_str(), "bytes_new" | "bytes_filled"),
        _ => false,
    }
}

/// Door (a) diagnostic: a covered collection field was initialized or
/// assigned from a possibly-aliased expression.
fn aliasing_init_error(class: &str, field: &str, site: &str, span: Span) -> CompileError {
    CompileError::type_err(
        format!(
            "the collection field '{field}' of '{class}' is constrained by a length \
             invariant, so its {site} must be a fresh collection the class alone owns — a \
             literal (e.g. '[]', '[x, y]') or a builtin that returns a new collection \
             (slice/keys/values/to_array). A bare variable, field read, or function-call \
             result could stay aliased elsewhere and be mutated behind the invariant's \
             back; build the value inline, or copy it with a slice (e.g. 'xs.slice(0, \
             xs.len())')"
        ),
        span,
    )
}

/// A length-changing collection mutator, classified by its effect on the
/// ghost length (rfc-number-types.md §4). Length-preserving mutators
/// (`reverse`, `fill`, `write_*`, `copy_from`) are not listed — they leave
/// the ghost untouched.
#[derive(Clone, Copy)]
enum LenDelta {
    /// `push` / `insert_at` (array), `push` (bytes): exactly +1.
    Inc,
    /// `pop` / `remove_at` (array): exactly −1, requires length ≥ 1.
    Dec,
    /// `clear`: exactly 0.
    Clear,
    /// map/set `insert`: key-newness is unknowable, so the new length is in
    /// [L, L+1] — modeled as monotone non-decrease (`new >= L`). The upper
    /// bound is dropped (conservative: an upper-bound invariant won't prove).
    MaybeInc,
    /// map/set `remove` (key may be absent) and bytes `extend` (adds ≥ 0):
    /// modeled as monotone non-increase (`new <= L`) / non-decrease
    /// respectively; see `apply_len_delta`.
    MaybeDec,
    /// bytes `extend`: adds len(arg) ≥ 0 elements — monotone non-decrease.
    Extend,
}

/// Classify a builtin collection mutator by receiver type and method name.
/// Returns `None` for non-mutating or length-preserving builtins.
fn len_delta_of(recv: &PlutoType, method: &str) -> Option<LenDelta> {
    match (recv, method) {
        (PlutoType::Array(_), "push" | "insert_at") => Some(LenDelta::Inc),
        (PlutoType::Array(_), "pop" | "remove_at") => Some(LenDelta::Dec),
        (PlutoType::Array(_), "clear") => Some(LenDelta::Clear),
        (PlutoType::Bytes, "push") => Some(LenDelta::Inc),
        (PlutoType::Bytes, "extend") => Some(LenDelta::Extend),
        (PlutoType::Map(_, _), "insert") | (PlutoType::Set(_), "insert") => Some(LenDelta::MaybeInc),
        (PlutoType::Map(_, _), "remove") | (PlutoType::Set(_), "remove") => Some(LenDelta::MaybeDec),
        _ => None,
    }
}

fn strong_update(
    scope: &mut InvariantScope,
    env: &TypeEnv,
    field: &str,
    value: &Expr,
    specs: &[InvariantSpec],
) {
    // Collection field reassignment: the field's new length is the length of
    // the (guaranteed-fresh, door (a)) RHS. A literal's length is exact; any
    // other fresh producer is only known `>= 0` (a new len ghost).
    if scope.is_coll_field(field) {
        let len = collection_expr_len(value)
            .map(Affine::constant)
            .unwrap_or_else(|| {
                scope.next_ghost += 1;
                Affine::term(scope.field_ghost(field, scope.next_ghost))
            });
        scope.sym.insert(field.to_string(), len);
        if specs.iter().any(|s| spec_fields(s).contains(field)) {
            scope.touched.insert(field.to_string());
        }
        return;
    }
    if !scope.fields.iter().any(|f| f == field) {
        // Non-int field: no invariant can mention it.
        return;
    }
    let aff = {
        let s: &InvariantScope = scope;
        to_affine_with(value, &|e| s.resolve(env, e))
    };
    let a = match aff {
        Some(a) => a,
        None => {
            // Unresolvable value (call, non-affine): a fresh unconstrained
            // ghost — the next boundary will require a guard to prove.
            scope.next_ghost += 1;
            Affine::term(format!("self.{field}@{}", scope.next_ghost))
        }
    };
    scope.sym.insert(field.to_string(), a);
    if specs.iter().any(|s| spec_fields(s).contains(field)) {
        scope.touched.insert(field.to_string());
    }
}

/// Collect, in traversal order, every length-changing builtin mutator call in
/// `stmt` whose receiver is `self.<covered collection field>`.
fn collection_mutations(
    stmt: &Stmt,
    _span: Span,
    env: &TypeEnv,
    coll_fields: &[String],
) -> Vec<(String, LenDelta, Span)> {
    struct Scan<'a> {
        env: &'a TypeEnv,
        coll_fields: &'a [String],
        out: Vec<(String, LenDelta, Span)>,
    }
    impl Visitor for Scan<'_> {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if let Expr::MethodCall { object, method, .. } = &expr.node {
                if let Some((path, ty)) = typed_path(&object.node, self.env) {
                    if let Some(field) = path.strip_prefix("self.") {
                        if !field.contains('.')
                            && self.coll_fields.iter().any(|f| f == field)
                        {
                            if let Some(d) = len_delta_of(&ty, &method.node) {
                                self.out.push((field.to_string(), d, expr.span));
                            }
                        }
                    }
                }
            }
            walk_expr(self, expr);
        }
    }
    let mut scan = Scan { env, coll_fields, out: Vec::new() };
    // Only this statement's *own* expressions — never the bodies of nested
    // control-flow statements, which the checker visits (and whose mutators it
    // applies) as their own statements.
    for e in immediate_exprs(stmt) {
        scan.visit_expr(e);
    }
    scan.out
}

/// Apply the length-ghost transfer function of every collection mutator in
/// this statement (rfc-number-types.md §4). Runs after the generic call
/// boundary (which may have re-anchored a collection field to invariant
/// level) so the delta composes on top of the post-boundary length, which is
/// sound in every case (see the module header's boundary discussion).
fn apply_collection_mutations(
    stmt: &Stmt,
    span: Span,
    env: &mut TypeEnv,
) -> Result<(), CompileError> {
    let coll_fields = match env.invariant_scope.as_ref() {
        Some(s) if !s.coll_fields.is_empty() => s.coll_fields.clone(),
        _ => return Ok(()),
    };
    let muts = collection_mutations(stmt, span, env, &coll_fields);
    if muts.is_empty() {
        return Ok(());
    }
    let mut scope = env.invariant_scope.take().expect("checked above");
    let r = (|| {
        for (field, delta, mspan) in &muts {
            apply_len_delta(&mut scope, field, *delta, *mspan)?;
        }
        Ok(())
    })();
    env.invariant_scope = Some(scope);
    r
}

/// Transfer function for one length-changing mutator on a covered collection
/// field. `pop`/`remove_at` additionally carry the obligation that the
/// collection is provably non-empty (the delta −1 is unsound otherwise).
fn apply_len_delta(
    scope: &mut InvariantScope,
    field: &str,
    delta: LenDelta,
    span: Span,
) -> Result<(), CompileError> {
    let l = scope
        .sym
        .get(field)
        .cloned()
        .unwrap_or_else(|| Affine::term(scope.field_ghost(field, 0)));
    // A fresh non-negative *delta* term bounded to `[lo, hi]`; the new length
    // is `l + delta`. Because the delta's bound is an exact interval, the
    // affine-bound engine sums it with `l`'s known bounds — so a lower bound
    // on `l` (e.g. the invariant `len > 0`) carries through the insert. (A
    // bare relation `new >= l` would not combine with `l`'s interval.)
    let mut add_delta = |scope: &mut InvariantScope, lo: i64, hi: Option<i64>| -> Option<Affine> {
        scope.next_ghost += 1;
        let d = format!("<delta:{field}>@{}", scope.next_ghost);
        let iv = match hi {
            Some(h) => Interval { lo, hi: h },
            None => Interval::at_least(lo),
        };
        scope.ghost_facts.assume(Fact::Bound(d.clone(), iv));
        // new = l + d (d is fresh, so no coefficient collision).
        let mut new = l.clone();
        *new.terms.entry(d).or_insert(0) += 1;
        Some(new)
    };
    let new = match delta {
        LenDelta::Inc => affine_add_const(&l, 1),
        LenDelta::Clear => Some(Affine::constant(0)),
        LenDelta::Dec => {
            let (lo, _) = affine_bounds(&l, &scope.ghost_facts).unwrap_or((None, None));
            if !matches!(lo, Some(v) if v >= 1) {
                return Err(empty_removal_error(scope, field, span));
            }
            affine_add_const(&l, -1)
        }
        // map/set insert: new = l + d, d in [0, 1] (exact [l, l+1]).
        LenDelta::MaybeInc => add_delta(scope, 0, Some(1)),
        // bytes extend: adds len(arg) >= 0 — new = l + d, d >= 0.
        LenDelta::Extend => add_delta(scope, 0, None),
        // map/set remove (key may be absent): non-increase. A fresh len term
        // (auto `>= 0`) bounded above by `l`; the lower bound is intentionally
        // dropped (a `len > 0` invariant must guard a remove).
        LenDelta::MaybeDec => {
            scope.next_ghost += 1;
            let g = scope.field_ghost(field, scope.next_ghost);
            let gt = Affine::term(g);
            if let Some(d) = diff_affine(&gt, &l) {
                for f in facts_from_diff(BinOp::LtEq, &d) {
                    scope.ghost_facts.assume(f);
                }
            }
            Some(gt)
        }
    };
    let new = new.unwrap_or_else(|| {
        // Arithmetic overflow building the delta: havoc to a fresh len ghost.
        scope.next_ghost += 1;
        Affine::term(scope.field_ghost(field, scope.next_ghost))
    });
    scope.sym.insert(field.to_string(), new);
    scope.touched.insert(field.to_string());
    Ok(())
}

/// Diagnostic for a `pop`/`remove_at` the prover cannot show is safe (the
/// collection may be empty).
fn empty_removal_error(scope: &InvariantScope, field: &str, span: Span) -> CompileError {
    CompileError::type_err(
        format!(
            "cannot prove 'self.{field}' is non-empty before this removal in method '{}' of \
             class '{}': removing from a possibly-empty collection is rejected because the \
             length ghost '-1' would be unsound. Guard the removal with a length check \
             (e.g. 'if self.{field}.len() > 0 {{ ... }}') or a 'requires self.{field}.len() \
             > 0' on the method",
            scope.method_name, scope.class_name
        ),
        span,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Branch and loop hooks
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn branch_snapshot(env: &TypeEnv) -> Option<SymSnapshot> {
    env.invariant_scope.as_ref().map(|s| SymSnapshot {
        sym: s.sym.clone(),
        touched: s.touched.clone(),
        // Evaluate each ensures spec against this path's state and the
        // facts visible HERE (callers take end-of-branch snapshots before
        // popping the branch scope, so guard facts participate). This is
        // the per-path half of the #455 join rule; it never errors — a
        // path that cannot prove the relation simply contributes `false`,
        // and the method's exit proof remains the deciding obligation.
        ensures_ok: s
            .ensures
            .iter()
            .map(|spec| {
                matches!(
                    eval_condition_with(
                        &spec.expr,
                        &|e| s.resolve_two_state(env, e),
                        &s.ghost_facts,
                    ),
                    Verdict::Proven
                )
            })
            .collect(),
    })
}

pub(crate) fn branch_restore(env: &mut TypeEnv, snap: &Option<SymSnapshot>) {
    if let (Some(scope), Some(sn)) = (env.invariant_scope.as_mut(), snap) {
        scope.sym = sn.sym.clone();
        scope.touched = sn.touched.clone();
    }
}

/// Close out one branch: if it survives (falls through) and changed the
/// symbolic state, the invariant must hold at its end. Returns whether the
/// branch is a *surviving changed* path. Call before popping the branch
/// scope (its guard facts must still be visible to the proof).
pub(crate) fn branch_end(
    env: &mut TypeEnv,
    snap: &Option<SymSnapshot>,
    terminates: bool,
    span: Span,
) -> Result<bool, CompileError> {
    let Some(sn) = snap else { return Ok(false) };
    let Some(scope) = env.invariant_scope.as_ref() else {
        return Ok(false);
    };
    let changed = scope.sym != sn.sym;
    if terminates {
        return Ok(false);
    }
    if changed {
        checkpoint_scope(env, span, "the end of this branch")?;
    }
    Ok(changed)
}

/// Merge the branches of an if/match. If any surviving branch changed the
/// symbolic state (and proved the invariant at its end), every other
/// surviving path must also satisfy it — including the implicit
/// fall-through path. The joined state then keeps as much symbolic precision
/// as the surviving branch end-states allow (see [`join_syms`]); what cannot
/// be kept re-anchors on invariant-level facts. If no surviving branch
/// changed anything, the pre-branch state is restored unchanged.
///
/// `surviving` carries the end-state snapshot of every branch that falls
/// through (callers include the pre-branch snapshot for an implicit
/// fall-through path, e.g. an `if` without `else`).
pub(crate) fn branch_join(
    env: &mut TypeEnv,
    snap: Option<SymSnapshot>,
    surviving: Vec<SymSnapshot>,
    any_surviving_changed: bool,
    unchanged_survivor: bool,
    span: Span,
) -> Result<(), CompileError> {
    let Some(sn) = snap else { return Ok(()) };
    if env.invariant_scope.is_none() {
        return Ok(());
    }
    if !any_surviving_changed {
        branch_restore(env, &Some(sn));
        return Ok(());
    }
    if unchanged_survivor && !sn.touched.is_empty() {
        branch_restore(env, &Some(sn.clone()));
        checkpoint_scope(env, span, "this branch join")?;
    }
    let mut scope = env.invariant_scope.take().expect("checked above");
    join_syms(&mut scope, env, &sn, &surviving);
    env.invariant_scope = Some(scope);
    reassume_invariants_main(env);
    Ok(())
}

/// Join the symbolic field states of the surviving branches. Every surviving
/// branch proved the full invariant at its end (`branch_end` /
/// `branch_join`'s fall-through checkpoint), so the joined state is a
/// genuine proven state — no composition is involved and every extracted
/// invariant fact is sound. Per field, in order of precision:
///
/// 1. All survivors agree → keep the exact affine value (the subtree-join
///    twin of subtract-then-add: `if c { x+=1 } else { x+=1 }` stays exact).
/// 2. Every survivor differs from the pre-branch value by a constant →
///    either the constants agree (keep `base + k` exactly) or the spread
///    `[lo, hi]` becomes a relation between the fresh anchor ghost and the
///    pre-branch term (`base <= f@N`, `f@N <= base`, strict when the bound
///    excludes zero) — enough to carry monotonicity (`ensures self.x >=
///    old(self.x)`) through a guarded increment.
/// 3. Otherwise → a fresh unconstrained anchor ghost.
fn join_syms(
    scope: &mut InvariantScope,
    env: &TypeEnv,
    base: &SymSnapshot,
    surviving: &[SymSnapshot],
) {
    scope.next_ghost += 1;
    let n = scope.next_ghost;
    let mut rels: Vec<Fact> = Vec::new();
    let fields: Vec<String> = scope
        .fields
        .iter()
        .chain(scope.coll_fields.iter())
        .cloned()
        .collect();
    for f in &fields {
        let vals: Vec<Option<&Affine>> = surviving.iter().map(|s| s.sym.get(f)).collect();
        // 1) Exact agreement across all survivors.
        if let Some(Some(first)) = vals.first() {
            if !vals.is_empty() && vals.iter().all(|v| *v == Some(*first)) {
                scope.sym.insert(f.clone(), (*first).clone());
                continue;
            }
        }
        let fresh_term = scope.field_ghost(f, n);
        let mut joined: Option<Affine> = None;
        if let Some(base_aff) = base.sym.get(f) {
            let ds: Option<Vec<i128>> = vals
                .iter()
                .map(|v| {
                    v.and_then(|a| diff_affine(a, base_aff))
                        .and_then(|d| d.terms.is_empty().then_some(d.k))
                })
                .collect();
            if let Some(ds) = ds {
                if !ds.is_empty() {
                    let lo = *ds.iter().min().expect("non-empty");
                    let hi = *ds.iter().max().expect("non-empty");
                    if lo == hi {
                        // 2a) Same constant offset on every path.
                        joined = affine_add_const(base_aff, lo);
                    } else if base_aff.terms.len() == 1 && base_aff.k == 0 {
                        // 2b) Constant spread relative to a single-term base.
                        let (t, &c) = base_aff.terms.iter().next().expect("len checked");
                        if c == 1 {
                            if lo >= 1 {
                                rels.push(Fact::Rel(t.clone(), RelOp::Lt, fresh_term.clone()));
                            } else if lo >= 0 {
                                rels.push(Fact::Rel(t.clone(), RelOp::Le, fresh_term.clone()));
                            }
                            if hi <= -1 {
                                rels.push(Fact::Rel(fresh_term.clone(), RelOp::Lt, t.clone()));
                            } else if hi <= 0 {
                                rels.push(Fact::Rel(fresh_term.clone(), RelOp::Le, t.clone()));
                            }
                        }
                    }
                }
            }
        }
        scope
            .sym
            .insert(f.clone(), joined.unwrap_or_else(|| Affine::term(fresh_term)));
    }
    scope.touched.clear();
    for r in rels {
        scope.ghost_facts.assume(r);
    }
    // Invariant-level knowledge holds at the join (every survivor proved it,
    // and the joined state is one of the survivors) — two-state facts
    // included, no transitivity filter needed.
    let mut to_assume = Vec::new();
    {
        let s: &InvariantScope = scope;
        for spec in &s.invariants {
            to_assume.extend(
                condition_facts_with(&spec.expr, &|e| s.resolve_two_state(env, e)).then_facts,
            );
        }
        // Ensures at a join (issue #455): a postcondition that EVERY
        // surviving path proved at its own end — under that path's facts,
        // branch guards included — holds of the joined state, because the
        // runtime join state is one of the surviving path states. This is
        // the exact disjunction rule invariant checking uses, with one
        // difference: paths owe no ensures obligation at their ends, so
        // the relation is *proven opportunistically* per path
        // (branch_snapshot) rather than enforced; a path that fails to
        // prove contributes nothing here and the exit proof decides.
        //
        // Guard: the join-time phrasing of a parameter term (`n@v`) must
        // denote the same ghost every per-path proof used. Versions only
        // move forward and are never restored per branch, so "every
        // mentioned local is still at version 0" guarantees agreement; a
        // reassigned parameter conservatively disables the rule.
        for (i, spec) in s.ensures.iter().enumerate() {
            if surviving.is_empty()
                || !surviving
                    .iter()
                    .all(|sv| sv.ensures_ok.get(i).copied().unwrap_or(false))
            {
                continue;
            }
            if !spec_locals_at_entry_version(&spec.expr, s) {
                continue;
            }
            to_assume.extend(
                condition_facts_with(&spec.expr, &|e| s.resolve_two_state(env, e)).then_facts,
            );
        }
    }
    for f in to_assume {
        scope.ghost_facts.assume(f);
    }
}

/// Is every non-`self` identifier an ensures expression mentions still at
/// local version 0 (never reassigned on any path so far)? See the join
/// rule in [`join_syms`].
fn spec_locals_at_entry_version(expr: &Expr, scope: &InvariantScope) -> bool {
    fn idents(e: &Expr, out: &mut HashSet<String>) {
        if let Some(inner) = old_call_arg(e) {
            idents(&inner.node, out);
            return;
        }
        match e {
            Expr::Ident(n) if n != "self" => {
                out.insert(n.clone());
            }
            Expr::BinOp { lhs, rhs, .. } => {
                idents(&lhs.node, out);
                idents(&rhs.node, out);
            }
            Expr::UnaryOp { operand, .. } => idents(&operand.node, out),
            _ => {}
        }
    }
    let mut names = HashSet::new();
    idents(expr, &mut names);
    names
        .iter()
        .all(|n| scope.local_ver.get(n).copied().unwrap_or(0) == 0)
}

/// Does a loop body interact with the proof state (self-field writes or
/// calls)? Pure-compute loops leave the symbolic state untouched.
pub(crate) fn loop_affects(env: &TypeEnv, body: &Block) -> bool {
    if env.invariant_scope.is_none() {
        return false;
    }
    struct Scan {
        found: bool,
    }
    impl Visitor for Scan {
        fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
            if self.found {
                return;
            }
            if let Stmt::FieldAssign { object, .. } = &stmt.node {
                if super::check::root_variable(&object.node) == Some("self") {
                    self.found = true;
                    return;
                }
            }
            walk_stmt(self, stmt);
        }
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
    for stmt in &body.stmts {
        scan.visit_stmt(stmt);
        if scan.found {
            break;
        }
    }
    scan.found
}

/// Loop entry: the invariant must hold entering a loop whose body interacts
/// with the object (iteration order is not tracked), and each iteration
/// starts from invariant-level knowledge only.
pub(crate) fn loop_enter(
    env: &mut TypeEnv,
    span: Span,
    affects: bool,
) -> Result<(), CompileError> {
    if affects {
        checkpoint_scope(env, span, "entry to this loop")?;
        // Non-composed: every state an iteration can start from (the
        // pre-loop state, or a previous iteration's end) has the full
        // invariant proven against it by a checkpoint.
        re_anchor_scope(env, false);
    }
    Ok(())
}

/// End of the loop body: the invariant must hold before the back-edge.
/// Call before popping the body scope.
pub(crate) fn loop_body_end(
    env: &mut TypeEnv,
    span: Span,
    affects: bool,
) -> Result<(), CompileError> {
    if affects {
        checkpoint_scope(env, span, "the end of the loop body")?;
    }
    Ok(())
}

/// After the loop: back to invariant-level knowledge. Non-composed — the
/// post-loop state is the pre-loop state or some iteration's end, each of
/// which proved the full invariant.
pub(crate) fn loop_exit(env: &mut TypeEnv, affects: bool) {
    if affects {
        re_anchor_scope(env, false);
        reassume_invariants_main(env);
    }
}

/// Does this statement's subtree write a field of `self`?
fn subtree_writes_self(stmt: &Stmt, span: Span) -> bool {
    struct Scan {
        found: bool,
    }
    impl Visitor for Scan {
        fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
            if self.found {
                return;
            }
            if let Stmt::FieldAssign { object, .. } = &stmt.node {
                if super::check::root_variable(&object.node) == Some("self") {
                    self.found = true;
                    return;
                }
            }
            walk_stmt(self, stmt);
        }
    }
    let mut scan = Scan { found: false };
    scan.visit_stmt(&Spanned::new(stmt.clone(), span));
    scan.found
}

// ─────────────────────────────────────────────────────────────────────────────
// Construction obligations
// ─────────────────────────────────────────────────────────────────────────────

/// Prove the class invariants for a struct-literal construction: substitute
/// the field initializers into each invariant and evaluate against the
/// flow facts in scope.
pub(crate) fn check_construction(
    class_name: &str,
    lit_fields: &[(Spanned<String>, Spanned<Expr>)],
    span: Span,
    env: &mut TypeEnv,
) -> Result<(), CompileError> {
    let Some(specs) = env.class_invariants.get(class_name) else {
        return Ok(());
    };
    if is_exempt_fn(env) || env.assume_discharged {
        // Exempt: marshal/wire glue validates at runtime instead; monomorphize
        // re-checks were proven at the template (see checkpoint).
        return Ok(());
    }
    let specs = specs.clone();
    let mut inits: HashMap<String, Option<Affine>> = HashMap::new();
    for (n, v) in lit_fields {
        inits.insert(n.node.clone(), to_affine(&v.node, env));
    }
    // Collection fields constrained by a length invariant: door (a) requires a
    // syntactically fresh initializer, and the invariant proof needs the
    // field's initial length (exact for a literal, only `>= 0` otherwise).
    let coll_fields = env
        .invariant_collection_fields
        .get(class_name)
        .cloned()
        .unwrap_or_default();
    let mut coll_len_inits: HashMap<String, Affine> = HashMap::new();
    for (n, v) in lit_fields {
        if !coll_fields.contains(&n.node) {
            continue;
        }
        if !is_fresh_collection_expr(&v.node) {
            return Err(aliasing_init_error(class_name, &n.node, "initializer", v.span));
        }
        let len = collection_expr_len(&v.node)
            .map(Affine::constant)
            .unwrap_or_else(|| Affine::term(format!("<ctor:{}.{}>.len()", class_name, n.node)));
        coll_len_inits.insert(n.node.clone(), len);
    }
    for spec in &specs {
        // A two-state invariant relates a state *transition* to its
        // pre-state; construction has no pre-state, so there is nothing to
        // prove here (the clause constrains every later write instead).
        if spec.two_state {
            continue;
        }
        let resolve = |e: &Expr| match e {
            // `self.<coll>.len()` resolves to the initializer's length.
            Expr::MethodCall { object, method, args, .. }
                if method.node == "len"
                    && args.is_empty()
                    && matches!(&object.node,
                        Expr::FieldAccess { object: o, field: f }
                            if matches!(&o.node, Expr::Ident(s) if s == "self")
                                && coll_fields.contains(&f.node)) =>
            {
                let Expr::FieldAccess { field, .. } = &object.node else { unreachable!() };
                coll_len_inits.get(&field.node).cloned()
            }
            Expr::FieldAccess { object, field }
                if matches!(&object.node, Expr::Ident(s) if s == "self") =>
            {
                inits.get(&field.node).cloned().flatten()
            }
            _ => None,
        };
        match eval_condition_with(&spec.expr, &resolve, &env.facts) {
            Verdict::Proven => {}
            Verdict::Refuted => {
                return Err(CompileError::type_err(
                    format!(
                        "construction of '{class_name}' violates its invariant '{}'{}",
                        spec.desc,
                        spec.blame()
                    ),
                    span,
                ));
            }
            Verdict::Unknown => {
                return Err(CompileError::type_err(
                    format!(
                        "cannot prove invariant '{}' of class '{class_name}' for this \
                         construction: the field initializers are not statically known to \
                         satisfy it. Establish the needed facts before constructing — a \
                         guard ('if x >= 0 {{ ... }}'), an 'assert', or a 'requires' clause \
                         on the enclosing function — or simplify the initializers to \
                         expressions the prover can bound{}",
                        spec.desc,
                        spec.blame()
                    ),
                    span,
                ));
            }
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// DI construction obligations
// ─────────────────────────────────────────────────────────────────────────────

/// Prove the invariants of a DI-constructed class against the state DI
/// synthesis actually produces. DI wiring never goes through a struct
/// literal: the instance is allocated zero-initialized and only its
/// injected dep fields are wired afterwards. Injected deps are class-typed,
/// so every field an invariant can mention (a non-injected int field)
/// starts at 0 — and since the rest of the program *assumes* the invariant
/// of every live instance, that zero state must satisfy it or the strict
/// guarantee has a hole.
///
/// All int fields resolve to the constant 0, so the verdict is always
/// decidable in practice; Unknown is handled identically for safety (it can
/// only arise from arithmetic-overflow bailouts).
pub(crate) fn check_di_construction(
    class_name: &str,
    site: &str,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    let Some(specs) = env.class_invariants.get(class_name) else {
        return Ok(());
    };
    let no_facts = FactEnv::new();
    for spec in specs {
        // Two-state invariants constrain transitions, not birth states —
        // see check_construction.
        if spec.two_state {
            continue;
        }
        let resolve = |e: &Expr| match e {
            Expr::FieldAccess { object, .. }
                if matches!(&object.node, Expr::Ident(s) if s == "self") =>
            {
                Some(Affine::constant(0))
            }
            _ => None,
        };
        match eval_condition_with(&spec.expr, &resolve, &no_facts) {
            Verdict::Proven => {}
            Verdict::Refuted | Verdict::Unknown => {
                return Err(CompileError::type_err(
                    format!(
                        "class '{class_name}' is constructed by dependency injection \
                         {site}, and its invariant '{}' does not hold for the \
                         zero-initialized state DI synthesis produces (every non-dep int \
                         field starts at 0; DI construction never passes through a struct \
                         literal that could prove otherwise). Either state an invariant \
                         the zero state satisfies (e.g. 'self.count >= 0'), or take \
                         '{class_name}' out of DI wiring: give it a 'scoped' lifecycle and \
                         seed it with proven initial values in a scope block \
                         ('scope({class_name} {{ ... }}) |x: {class_name}| {{ ... }}'){}",
                        spec.desc,
                        spec.blame()
                    ),
                    spec.span,
                ));
            }
        }
    }
    Ok(())
}

/// The startup pass: every singleton and transient in `env.di_order` is
/// zero-constructed by the synthesized app/stage startup wiring (singletons
/// once at startup, transients fresh at each injection point), so each must
/// satisfy its invariants in the zero state. Runs right after
/// `register_invariants` (by which point `validate_di_graph` has finalized
/// the order and inferred effective lifecycles).
///
/// Scoped-effective classes are skipped: a scoped class with non-injected
/// fields is already rejected if it is reachable from startup (captive
/// dependency), and otherwise it is only ever created through a scope block
/// — seeded instances are proven at their struct literal, auto-created ones
/// carry this obligation at the scope block (`check_scope_stmt`). Programs
/// with no app and no stages run no startup wiring, so the obligation does
/// not apply.
pub(crate) fn check_di_constructions(
    program: &Program,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    if env.class_invariants.is_empty() {
        return Ok(());
    }
    if program.app.is_none() && program.stages.is_empty() {
        return Ok(());
    }
    for class_name in &env.di_order {
        if env.classes.get(class_name).map(|c| c.lifecycle)
            == Some(crate::parser::ast::Lifecycle::Scoped)
        {
            continue;
        }
        check_di_construction(class_name, "at startup", env)?;
    }
    // Classes zero-constructed transitively, as the zero-state VALUE of a
    // non-injected field of a DI-wired class (validate_di_graph computed the
    // closure). They are born in the zero state exactly like the DI roots,
    // so they carry the same obligation.
    for class_name in &env.di_zero_closure {
        check_di_construction(class_name, "at startup (as the zero state of a data field of a DI-wired class)", env)?;
    }
    Ok(())
}
