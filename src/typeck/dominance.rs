//! `guarded_by` dominance proofs — properties RFC atom 3, phase 3
//! (docs/design/rfc-properties.md).
//!
//! A field may carry a guard clause:
//!
//! ```text
//! object BlobAuthority {
//!     epoch: int
//!     data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
//! }
//! ```
//!
//! Semantics: every write site of the field — the whole-program closed
//! write-set — must be **dominated**, in its containing method's control
//! flow, by a conditional the fact engine proves implies the predicate
//! instantiated with some in-scope value of the binder type. For Blob, the
//! fence `if tok != self.epoch { raise StaleGrant {...} }` dominating
//! `self.data = ...` leaves the fall-through fact `tok == self.epoch`
//! (with `tok` bound from `grant.token`), which proves
//! `grant.token == self.epoch` — the predicate with `g := grant`.
//!
//! # Dominance via flow facts (the algorithm)
//!
//! Pluto's control flow is structured (if/match/loops — no goto), so this
//! pass does not build a CFG or run a general dominator algorithm. Instead
//! it reuses the fact engine's assumption discipline, which *is* structural
//! dominance: a fact extracted from a conditional is visible exactly at the
//! program points the conditional dominates (the taken branch, or the rest
//! of the block after a branch whose other path terminates), and is killed
//! by anything that could invalidate it in between (reassignment, writes to
//! the compared fields, calls that may mutate reachable state, loop
//! back-edges). "The predicate is provable from the facts live at the write
//! site" is therefore precisely "a dominating check implies the predicate,
//! with nothing in between that could unsay it".
//!
//! The pass runs standalone after body checking (the `linearity.rs` model):
//! it re-walks every function/method body with its own fact environment and
//! a best-effort local typing of parameters and `let` bindings. Everything
//! it cannot resolve is conservative: unknown callee ⇒ facts killed,
//! unknown type ⇒ not a binder candidate, unknown verdict ⇒ compile error.
//!
//! # The fact vocabulary
//!
//! Terms are int-typed paths: locals (`tok`), own fields of the receiver
//! (`self.epoch`), and one-level int fields of class-typed locals
//! (`grant.token`). Entity-typed roots other than `self` are excluded — a
//! shared entity's fields can change between any two statements of this
//! method (only the receiver's own serialization lock protects `self.*`,
//! and reentrant self-calls are covered by the call kill rule). A `let`
//! binding whose value is a pure trackable term is tracked as an *alias*
//! (`tok` resolves to `grant.token`), so guards phrased through a fence
//! local connect to predicates phrased through the binder — the alias dies
//! as soon as any kill touches the underlying path.
//!
//! # Kill rules (mirroring facts.rs, adapted to this vocabulary)
//!
//! - Reassignment kills the variable's facts and facts under it.
//! - `self.f = v` kills `self.f` facts and every fact rooted at another
//!   variable of the same class (a possible alias of the receiver); facts
//!   about other `self.*` fields survive (a write to field f cannot change
//!   field g of the same object).
//! - A write through any other root, or an index assignment, kills all
//!   field-path facts.
//! - Any call that may run user code (everything except builtin collection
//!   methods, which can never write a class's int fields) kills all
//!   field-path facts. `at`, `spawn`, `serve`, `yield`, select and scope
//!   blocks do the same.
//! - Loop entry and exit drop all facts (facts.rs's conservative loop
//!   rule); guards inside the loop body still dominate writes later in the
//!   same body.
//! - Closure bodies are analyzed under a fact barrier: a guarded write
//!   inside a closure must be dominated by a check inside the same closure.
//!
//! # Obligation sites
//!
//! - `self.<field> = v` inside the guarded class's own methods: the
//!   dominance obligation.
//! - `self.<field>[i] = v` (index assignment through the guarded field):
//!   the same obligation — mutating the field's contents is an effect on
//!   the field.
//! - Any other write to a guarded field (foreign writes, writes through a
//!   non-`self` binding even inside the class): rejected outright. Guarded
//!   fields are protocol-internal; the dominance proof is only meaningful
//!   under the class's own control flow.
//! - Construction is NOT a write site: the clause governs overwrites of
//!   live state, not birth (construction obligations belong to
//!   `invariant`, which guards the same object's value shape).
//!
//! The closed write-set is the set of assignment and index-assignment
//! sites, plus — for collection-shaped fields (arrays, maps, sets, bytes)
//! — every mutating builtin method call on the field. Because a
//! collection's contents can also be mutated through an alias of the
//! field's *value* (`let d = self.data` then `d.push(...)` — issue #418),
//! collection-shaped guarded fields additionally carry a whole-program
//! aliasing ban, the same discipline `idempotency.rs` applies to dedup
//! fields: no bare-value use anywhere (no binding, passing, returning,
//! iterating, or interpolating), mutating builtins only through `self`
//! under the dominance obligation, pure reads (len/contains/indexing/...)
//! from anywhere, and construction/assignment only from freshly-created
//! values (collection literals, fresh-producing builtins like
//! `.to_bytes()`). Field types whose interior the write-set cannot close
//! at all (classes, nested collections, traits, tasks, channels) are
//! rejected at declaration. Fields of immutable value shape (ints,
//! floats, bools, bytes-the-scalar, strings) need none of this — their
//! write-set is exactly the assignment sites.
//!
//! # Concurrency side-condition
//!
//! On an `object` (entity), methods are serialized per instance, so no
//! other message can run between a dominating check and the write it
//! guards — the check-then-act window is closed by the construct (the
//! reentrancy resolution keeps same-message self-calls sound: they are
//! calls, and calls kill facts). On a value class, `guarded_by` is sound
//! for a different reason: values do not share — every binding is its own
//! copy (spawn deep-copies, wire transfers copy), so no concurrent writer
//! exists to race the check. Both cases need no extra restriction; the
//! clause is accepted on classes and objects alike.
//!
//! # Fragment
//!
//! The predicate must be decidable: `&&`/`||`/`!` over integer comparisons
//! of linear arithmetic whose leaves are int literals, `self.<f>` (own int
//! fields of the guarded class), and `<binder>.<f>` (one-level int fields
//! of the binder class). Guards on generic classes are rejected (like
//! invariants: their bodies are checked against opaque type parameters and
//! instantiations are never re-proven).

use std::collections::HashMap;

use crate::diagnostics::CompileError;
use crate::parser::ast::{
    BinOp, Block, CatchHandler, ClassDecl, ContractKind, Expr, Function, GuardClause,
    MatchPattern, Program, Stmt, TypeExpr, UnaryOp,
};
use crate::span::{Span, Spanned};

use super::env::{mangle_method, MethodResolution, TypeEnv};
use super::facts::{
    condition_facts_with, eval_condition_with, to_affine_with, Affine, FactEnv, Verdict,
};
use super::types::PlutoType;

// ─────────────────────────────────────────────────────────────────────────────
// Registration and declaration-time validation
// ─────────────────────────────────────────────────────────────────────────────

/// A validated `guarded_by` clause.
#[derive(Debug, Clone)]
pub struct GuardSpec {
    pub class_name: String,
    pub field_name: String,
    pub binder_name: String,
    pub binder_class: String,
    /// The guarded field's contents can be mutated through an alias of the
    /// field's value (arrays, maps, sets, bytes — reference-shaped
    /// collections with mutating builtin methods). Such fields carry the
    /// whole-program aliasing ban (issue #418): no bare-value use anywhere,
    /// mutating builtin methods only through `self` under the dominance
    /// obligation, and construction/assignment only from freshly-created
    /// values — otherwise `let d = self.data; d.push(...)` would mutate the
    /// guarded contents outside the closed write-set the fencing theorem
    /// depends on.
    pub aliasable: bool,
    pub predicate: Spanned<Expr>,
    /// Rendered clause for diagnostics:
    /// `guarded_by (g: WriteGrant) g.token == self.epoch`.
    pub desc: String,
    pub span: Span,
    /// Set when the clause was injected by a property instantiation
    /// (`satisfies fenced(...)`, rfc-properties.md slice 2). Every
    /// diagnostic about the obligation appends `blame()` so the failure
    /// shows BOTH the property body and the failing site.
    pub provenance: Option<crate::parser::ast::PropertyProvenance>,
}

impl GuardSpec {
    /// The property-side blame suffix ("" for hand-written clauses).
    fn blame(&self) -> String {
        crate::parser::ast::provenance_blame(&self.provenance)
    }
}

/// Immutable value shapes: a guarded collection's elements must be these —
/// an element alias cannot mutate them in place, so element reads through
/// the guarded field leak nothing the write-set must track.
fn immutable_elem(ty: &PlutoType) -> bool {
    matches!(
        ty,
        PlutoType::Int | PlutoType::Float | PlutoType::Bool | PlutoType::Byte | PlutoType::String
    )
}

/// Classify a guarded field's type for the aliasing discipline (issue
/// #418). `Ok(false)`: immutable value shape — assignment sites are the
/// whole write-set. `Ok(true)`: collection shape — mutable through aliases
/// of the field's value, so the whole-program aliasing ban applies.
/// `Err(reason)`: interior mutability the closed write-set cannot see at
/// all — rejected at declaration.
fn classify_guard_field_type(ty: &PlutoType) -> Result<bool, String> {
    match ty {
        PlutoType::Int
        | PlutoType::Float
        | PlutoType::Bool
        | PlutoType::Byte
        | PlutoType::String
        | PlutoType::Void
        | PlutoType::Enum(_)
        | PlutoType::Range
        | PlutoType::Error => Ok(false),
        PlutoType::Bytes => Ok(true),
        PlutoType::Array(e) | PlutoType::Set(e) => {
            if immutable_elem(e) {
                Ok(true)
            } else {
                Err(format!(
                    "element type {e} is not an immutable value shape (int, float, bool, \
                     byte, string) — an element alias could mutate the guarded contents \
                     in place, escaping the fence"
                ))
            }
        }
        PlutoType::Map(k, v) => {
            if immutable_elem(k) && immutable_elem(v) {
                Ok(true)
            } else {
                Err(format!(
                    "key/value types ({k}, {v}) must both be immutable value shapes \
                     (int, float, bool, byte, string) — an entry alias could mutate the \
                     guarded contents in place, escaping the fence"
                ))
            }
        }
        other => Err(format!(
            "a value of type {other} can be mutated through aliases by writes the \
             guard's closed write-set cannot see (interior mutability). Guard a field \
             of value shape (int, float, bool, byte, string, enum) or a collection of \
             those"
        )),
    }
}

fn guard_desc(field: &str, clause: &GuardClause, binder_class: &str) -> String {
    format!(
        "'{field}' guarded_by ({}: {binder_class}) {}",
        clause.binder.node,
        crate::codegen::format_invariant_expr(&clause.predicate.node)
    )
}

/// Type-check and fragment-validate every `guarded_by` clause, and register
/// the specs in `env.guarded_fields`. Runs alongside invariant registration,
/// before any body checking.
pub(crate) fn register_guards(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for class in &program.classes {
        let c = &class.node;
        let guarded: Vec<_> = c
            .fields
            .iter()
            .filter_map(|f| f.guarded_by.as_ref().map(|g| (f, g)))
            .collect();
        if guarded.is_empty() {
            continue;
        }
        if !c.type_params.is_empty() {
            return Err(CompileError::type_err(
                format!(
                    "guarded_by on generic classes is not yet supported: guard clauses are \
                     compile-time proof obligations, and generic bodies are checked against \
                     opaque type parameters. Declare the guard on a concrete class wrapping \
                     '{}' instead",
                    c.name.node
                ),
                guarded[0].1.predicate.span,
            ));
        }
        let mut specs = Vec::new();
        for (field, clause) in guarded {
            // Declaration-time validation; property-injected clauses carry
            // the property-side blame suffix on every failure (two-sided
            // blame also covers instantiation-time validation).
            let spec = validate_guard_clause(c, field, clause, env)
                .map_err(|e| super::discharge::append_blame(e, &clause.provenance))?;
            specs.push(spec);
        }
        env.guarded_fields.insert(c.name.node.clone(), specs);
    }
    Ok(())
}

/// Validate one `guarded_by` clause and build its spec.
fn validate_guard_clause(
    c: &crate::parser::ast::ClassDecl,
    field: &crate::parser::ast::Field,
    clause: &GuardClause,
    env: &mut TypeEnv,
) -> Result<GuardSpec, CompileError> {
    {
        {
            let binder_class = match &clause.binder_ty.node {
                TypeExpr::Named(n) if env.classes.contains_key(n) => n.clone(),
                TypeExpr::Named(n) => {
                    return Err(CompileError::type_err(
                        format!(
                            "guarded_by binder type '{n}' is not a declared class; the \
                             binder must name a concrete value class carrying the guard \
                             evidence (e.g. a grant or token class)"
                        ),
                        clause.binder_ty.span,
                    ));
                }
                _ => {
                    return Err(CompileError::type_err(
                        "guarded_by binder type must be a concrete value class (a named, \
                         non-generic class carrying the guard evidence)"
                            .to_string(),
                        clause.binder_ty.span,
                    ));
                }
            };
            if env.object_types.contains(&binder_class) {
                return Err(CompileError::type_err(
                    format!(
                        "guarded_by binder type '{binder_class}' is an object (entity): \
                         entity fields can change concurrently, so facts about them can \
                         never prove a guard. Use a value class as the evidence type"
                    ),
                    clause.binder_ty.span,
                ));
            }
            if clause.binder.node == "self" {
                return Err(CompileError::type_err(
                    "guarded_by binder may not be named 'self' (the predicate's 'self' is \
                     the carrying object)"
                        .to_string(),
                    clause.binder.span,
                ));
            }
            // Type-check the predicate with `self` and the binder in scope.
            env.push_scope();
            env.define_unchecked("self".to_string(), PlutoType::Class(c.name.node.clone()));
            env.define_unchecked(
                clause.binder.node.clone(),
                PlutoType::Class(binder_class.clone()),
            );
            let pred_ty = super::infer::infer_expr(
                &clause.predicate.node,
                clause.predicate.span,
                env,
                None,
            );
            env.pop_scope();
            let pred_ty = pred_ty?;
            if pred_ty != PlutoType::Bool {
                return Err(CompileError::type_err(
                    format!("guarded_by predicate must be bool, found {pred_ty}"),
                    clause.predicate.span,
                ));
            }
            let desc = guard_desc(&field.name.node, clause, &binder_class);
            // Field-shape validation (issue #418): the guard theorem is "no
            // write to the field escapes the fence", so the field's type
            // must have a closable write-set. Nullable wrappers classify as
            // their inner type (a none has no contents to mutate).
            let field_ty = env
                .classes
                .get(&c.name.node)
                .and_then(|ci| ci.fields.iter().find(|(n, _, _)| *n == field.name.node))
                .map(|(_, t, _)| t.clone())
                .unwrap_or(PlutoType::Void);
            let unwrapped = match &field_ty {
                PlutoType::Nullable(inner) => (**inner).clone(),
                other => other.clone(),
            };
            let aliasable = classify_guard_field_type(&unwrapped).map_err(|reason| {
                CompileError::type_err(
                    format!(
                        "guarded_by is not supported on field '{}' of type {field_ty}: \
                         {reason}",
                        field.name.node
                    ),
                    clause.predicate.span,
                )
            })?;
            validate_guard_fragment(
                &clause.predicate,
                &c.name.node,
                &clause.binder.node,
                &binder_class,
                &desc,
                env,
            )?;
            Ok(GuardSpec {
                class_name: c.name.node.clone(),
                field_name: field.name.node.clone(),
                binder_name: clause.binder.node.clone(),
                binder_class,
                aliasable,
                predicate: clause.predicate.clone(),
                desc,
                span: clause.predicate.span,
                provenance: clause.provenance.clone(),
            })
        }
    }
}

fn fragment_err(reason: String, desc: &str, span: Span) -> CompileError {
    CompileError::type_err(
        format!(
            "guard {desc} is outside the provable fragment: {reason}. Guard predicates are \
             compile-time proof obligations; they must be built from &&, ||, ! over integer \
             comparisons of linear arithmetic over the binder's int fields and the class's \
             own int fields (e.g. 'g.token == self.epoch')"
        ),
        span,
    )
}

/// Validate that a guard predicate is within the provable fragment.
fn validate_guard_fragment(
    expr: &Spanned<Expr>,
    class_name: &str,
    binder: &str,
    binder_class: &str,
    desc: &str,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    match &expr.node {
        Expr::BinOp { op: BinOp::And | BinOp::Or, lhs, rhs } => {
            validate_guard_fragment(lhs, class_name, binder, binder_class, desc, env)?;
            validate_guard_fragment(rhs, class_name, binder, binder_class, desc, env)
        }
        Expr::UnaryOp { op: UnaryOp::Not, operand } => {
            validate_guard_fragment(operand, class_name, binder, binder_class, desc, env)
        }
        Expr::BinOp {
            op: BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq | BinOp::Eq | BinOp::Neq,
            lhs,
            rhs,
        } => {
            validate_guard_side(lhs, class_name, binder, binder_class, desc, env)?;
            validate_guard_side(rhs, class_name, binder, binder_class, desc, env)
        }
        _ => Err(fragment_err(
            "it is not an integer comparison".to_string(),
            desc,
            expr.span,
        )),
    }
}

/// One side of a guard comparison: linear int arithmetic over `self.<f>`
/// (own int fields) and `<binder>.<f>` (one-level int fields of the binder).
fn validate_guard_side(
    expr: &Spanned<Expr>,
    class_name: &str,
    binder: &str,
    binder_class: &str,
    desc: &str,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    match &expr.node {
        Expr::IntLit(_) => Ok(()),
        Expr::FieldAccess { object, field } => {
            let (owner, root_desc) = match &object.node {
                Expr::Ident(s) if s == "self" => (class_name, "self"),
                Expr::Ident(s) if s == binder => (binder_class, "the binder"),
                _ => {
                    return Err(fragment_err(
                        format!(
                            "only direct int fields of 'self' and of the binder \
                             '{binder}' are provable"
                        ),
                        desc,
                        expr.span,
                    ));
                }
            };
            let fty = env
                .classes
                .get(owner)
                .and_then(|ci| ci.fields.iter().find(|(n, _, _)| *n == field.node))
                .map(|(_, t, _)| t.clone());
            match fty {
                Some(PlutoType::Int) => Ok(()),
                Some(other) => Err(fragment_err(
                    format!(
                        "field '{}' of {root_desc} has type {other} — only int fields \
                         are provable",
                        field.node
                    ),
                    desc,
                    expr.span,
                )),
                None => Err(CompileError::type_err(
                    format!("class '{owner}' has no field '{}'", field.node),
                    expr.span,
                )),
            }
        }
        Expr::UnaryOp { op: UnaryOp::Neg, operand } => {
            validate_guard_side(operand, class_name, binder, binder_class, desc, env)
        }
        Expr::BinOp { op: BinOp::Add | BinOp::Sub, lhs, rhs } => {
            validate_guard_side(lhs, class_name, binder, binder_class, desc, env)?;
            validate_guard_side(rhs, class_name, binder, binder_class, desc, env)
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
            validate_guard_side(lhs, class_name, binder, binder_class, desc, env)?;
            validate_guard_side(rhs, class_name, binder, binder_class, desc, env)
        }
        _ => Err(fragment_err(
            "only int literals, int fields of 'self'/the binder, and +, -, * by a \
             constant are provable"
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

// ─────────────────────────────────────────────────────────────────────────────
// The whole-program write-site pass
// ─────────────────────────────────────────────────────────────────────────────

/// Prove every write site of every guarded field. Standalone pass, run after
/// body checking (method resolutions and function signatures are complete).
pub(crate) fn check_guard_dominance(
    program: &Program,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    if env.guarded_fields.is_empty() {
        return Ok(());
    }
    for func in &program.functions {
        analyze_body(&func.node.name.node, &func.node, None, env)?;
    }
    for class in &program.classes {
        for m in &class.node.methods {
            let key = mangle_method(&class.node.name.node, &m.node.name.node);
            analyze_body(&key, &m.node, Some(&class.node), env)?;
        }
    }
    if let Some(app) = &program.app {
        for m in &app.node.methods {
            let key = mangle_method(&app.node.name.node, &m.node.name.node);
            analyze_body(&key, &m.node, None, env)?;
        }
    }
    for stage in &program.stages {
        for m in &stage.node.methods {
            let key = mangle_method(&stage.node.name.node, &m.node.name.node);
            analyze_body(&key, &m.node, None, env)?;
        }
    }
    Ok(())
}

struct Analyzer<'a> {
    env: &'a TypeEnv,
    /// Resolution key for method-call lookups (matches `env.current_fn`
    /// during body checking: function name, or `Class$method`).
    current_fn: String,
    /// `Some(class)` when analyzing a method body (`self`'s class).
    owner_class: Option<String>,
    facts: FactEnv,
    /// Scoped best-effort types of locals and parameters.
    vars: Vec<HashMap<String, PlutoType>>,
    /// Pure-value bindings: local name → affine form over live terms
    /// (`tok` → `grant.token`). Invalidated by kills on any mentioned term.
    aliases: HashMap<String, Affine>,
    error: Option<CompileError>,
}

fn analyze_body(
    current_fn: &str,
    func: &Function,
    owner: Option<&ClassDecl>,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    let mut a = Analyzer {
        env,
        current_fn: current_fn.to_string(),
        owner_class: owner.map(|c| c.name.node.clone()),
        facts: FactEnv::new(),
        vars: vec![HashMap::new()],
        aliases: HashMap::new(),
        error: None,
    };
    // Parameter types from the declared annotations.
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
    // Entry facts: requires clauses hold at entry (runtime-enforced).
    for c in &func.contracts {
        if c.node.kind != ContractKind::Requires || a.has_unsafe_call(&c.node.expr.node) {
            continue;
        }
        let cf = condition_facts_with(&c.node.expr.node, &|e| a.resolve(e));
        for f in cf.then_facts {
            a.facts.assume(f);
        }
    }
    a.walk_block(&func.body.node);
    match a.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

impl<'a> Analyzer<'a> {
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

    // ── Best-effort typing ───────────────────────────────────────────────

    fn type_of_type_expr(&self, te: &TypeExpr) -> Option<PlutoType> {
        match te {
            TypeExpr::Named(n) if n == "int" => Some(PlutoType::Int),
            TypeExpr::Named(n) if self.env.classes.contains_key(n) => {
                Some(PlutoType::Class(n.clone()))
            }
            _ => None,
        }
    }

    /// Best-effort type of an expression. `None` means unknown — the value
    /// is simply untracked (never a binder candidate, never a fact term).
    fn type_of_expr(&self, expr: &Expr) -> Option<PlutoType> {
        match expr {
            Expr::Ident(name) => self.lookup(name).cloned(),
            Expr::IntLit(_) => Some(PlutoType::Int),
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
            Expr::BinOp { op: BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod, lhs, .. } => {
                match self.type_of_expr(&lhs.node) {
                    Some(PlutoType::Int) => Some(PlutoType::Int),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Resolve an expression to a typed, trackable path. Mirrors
    /// `facts::typed_path`, but allows `self` (the receiver is protected by
    /// its own serialization while this method runs) and uses this pass's
    /// local typing. Entity/remote/domain roots other than `self` are
    /// excluded at every step.
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

    /// Leaf resolver for the fact engine: int paths in this pass's
    /// vocabulary, with alias substitution for pure `let` bindings.
    fn resolve(&self, e: &Expr) -> Option<Affine> {
        if let Expr::Ident(name) = e {
            if let Some(a) = self.aliases.get(name) {
                return Some(a.clone());
            }
        }
        match self.path_of(e) {
            Some((path, PlutoType::Int)) => Some(Affine::term(path)),
            _ => None,
        }
    }

    // ── Call safety ──────────────────────────────────────────────────────

    /// Does this expression contain a call that may run user code (and thus
    /// mutate int fields reachable through aliases)? Builtin collection
    /// methods are exempt: they never touch class fields (the load-bearing
    /// survey lives on `facts::CallSeverity::Collections` — the shared
    /// purity predicate this pass's exemptions defer to). Direct calls to
    /// free functions whose declared parameters cannot reach any class
    /// value are exempt for the same reason (`facts::free_call_severity`).
    /// Closure bodies are not scanned (creation runs nothing); they are
    /// analyzed separately under a fact barrier.
    fn has_unsafe_call(&self, expr: &Expr) -> bool {
        match expr {
            Expr::MethodCall { object, method, args, .. } => {
                let key = (self.current_fn.clone(), method.span.start);
                let builtin = matches!(
                    self.env.method_resolutions.get(&key),
                    Some(MethodResolution::Builtin)
                        | Some(MethodResolution::TaskDetach)
                        | Some(MethodResolution::TaskCancel)
                );
                if !builtin {
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
            Expr::StaticTraitCall { .. }
            | Expr::At { .. }
            | Expr::Spawn { .. } => true,
            Expr::Closure { .. } | Expr::ClosureCreate { .. } => false,
            Expr::BinOp { lhs, rhs, .. }
            | Expr::NullCoalesce { lhs, rhs } => {
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
            Expr::StructLit { fields, .. } => {
                fields.iter().any(|(_, v)| self.has_unsafe_call(&v.node))
            }
            Expr::ArrayLit { elements, .. } | Expr::SetLit { elements, .. } => {
                elements.iter().any(|e| self.has_unsafe_call(&e.node))
            }
            Expr::MapLit { entries, .. } => entries
                .iter()
                .any(|(k, v)| self.has_unsafe_call(&k.node) || self.has_unsafe_call(&v.node)),
            Expr::EnumData { fields, .. } => {
                fields.iter().any(|(_, v)| self.has_unsafe_call(&v.node))
            }
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
                        // Handler blocks contain statements; conservatively
                        // treat any statement-bearing handler as unsafe (the
                        // handler may do anything).
                        CatchHandler::Wildcard { .. } | CatchHandler::Typed { .. } => true,
                    })
            }
            Expr::Range { start, end, .. } => {
                self.has_unsafe_call(&start.node) || self.has_unsafe_call(&end.node)
            }
            Expr::If { condition, .. } => {
                // Branch statements are walked structurally (they apply
                // their own kills); only the condition is evaluated
                // unconditionally here.
                self.has_unsafe_call(&condition.node)
            }
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

    // ── Kills ────────────────────────────────────────────────────────────

    /// Drop aliases whose affine form mentions any path at or under `root`.
    fn drop_aliases_under(&mut self, root: &str) {
        self.aliases.retain(|_, a| {
            !a.terms.keys().any(|t| {
                t == root || (t.len() > root.len() && t.starts_with(root)
                    && t.as_bytes()[root.len()] == b'.')
            })
        });
    }

    fn kill_path(&mut self, root: &str) {
        self.facts.kill_path(root);
        self.aliases.remove(root);
        self.drop_aliases_under(root);
    }

    fn kill_fields(&mut self) {
        self.facts.kill_fields();
        self.aliases.retain(|_, a| !a.terms.keys().any(|t| t.contains('.')));
    }

    fn havoc(&mut self) {
        self.facts.havoc_all();
        self.aliases.clear();
    }

    /// Kill rule for a field write `root.field = v`: the written path dies,
    /// and so does every fact rooted at another variable of the same class
    /// (a potential alias of the written object). Facts about the root's
    /// *other* fields survive — a write to field f cannot change field g of
    /// the same object.
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

    // ── Expression walking (nested blocks + kill application) ───────────

    /// Process one evaluated expression: apply its call kills, analyze any
    /// statement-bearing sub-structures (if/match branches, catch handlers,
    /// closure bodies), and enforce the aliasing ban on collection-shaped
    /// guarded fields (issue #418).
    fn scan_expr(&mut self, expr: &Spanned<Expr>) {
        if self.error.is_some() {
            return;
        }
        if self.has_unsafe_call(&expr.node) {
            self.kill_fields();
        }
        self.scan_nested(&expr.node);
        self.alias_scan(expr);
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
                // valid inside, and a guarded write inside must find its
                // guard inside.
                let saved_facts = std::mem::take(&mut self.facts);
                let saved_aliases = std::mem::take(&mut self.aliases);
                self.vars.push(HashMap::new());
                for p in params {
                    if let Some(ty) = self.type_of_type_expr(&p.ty.node) {
                        self.define(&p.name.node, ty);
                    }
                }
                self.walk_block(&body.node);
                self.vars.pop();
                self.facts = saved_facts;
                self.aliases = saved_aliases;
            }
            Expr::Catch { expr, handlers } => {
                self.scan_nested(&expr.node);
                for h in handlers {
                    match h {
                        CatchHandler::Wildcard { var, body }
                        | CatchHandler::Typed { var, body, .. } => {
                            self.push_scope();
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
                self.kill_path(&name.node);
                self.undefine(&name.node);
            }
        }
    }

    // ── Statements ───────────────────────────────────────────────────────

    /// Walk a block, maintaining facts flow-sensitively. Returns true when
    /// the block always terminates (mirrors `block_always_terminates`).
    fn walk_block(&mut self, block: &Block) -> bool {
        let mut terminated = false;
        for stmt in &block.stmts {
            if self.error.is_some() {
                return terminated;
            }
            if terminated {
                // Unreachable code: still scan for foreign writes, with no
                // fact state to speak of.
                self.walk_stmt(stmt);
                continue;
            }
            terminated = self.walk_stmt(stmt);
        }
        terminated
    }

    /// Returns true when the statement always terminates the block.
    fn walk_stmt(&mut self, stmt: &Spanned<Stmt>) -> bool {
        if self.error.is_some() {
            return false;
        }
        match &stmt.node {
            Stmt::Let { name, value, .. } => {
                self.scan_expr(value);
                self.kill_path(&name.node);
                self.bind_value(&name.node, value);
                false
            }
            Stmt::Assign { target, value } => {
                self.scan_expr(value);
                self.kill_path(&target.node);
                self.bind_value(&target.node, value);
                false
            }
            Stmt::FieldAssign { object, field, value } => {
                self.scan_expr(object);
                self.scan_expr(value);
                self.field_assign(object, field, value, stmt.span);
                false
            }
            Stmt::IndexAssign { object, index, value } => {
                // The object is the write target: a guarded-field access at
                // its top is index_assign's obligation, not a bare-value
                // use — scan kills/nested structure and anything beneath
                // the guarded step, but skip the top-level alias ban.
                if self.has_unsafe_call(&object.node) {
                    self.kill_fields();
                }
                self.scan_nested(&object.node);
                if self.aliasable_guard_access(&object.node).is_some() {
                    if let Expr::FieldAccess { object: fobj, .. } = &object.node {
                        self.alias_scan(fobj);
                    }
                } else {
                    self.alias_scan(object);
                }
                self.scan_expr(index);
                self.scan_expr(value);
                self.index_assign(object, stmt.span);
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
                    self.scan_nested(&expr.node);
                } else {
                    self.scan_nested(&expr.node);
                    let cf = condition_facts_with(&expr.node, &|e| self.resolve(e));
                    for f in cf.then_facts {
                        self.facts.assume(f);
                    }
                }
                false
            }
            Stmt::LetChan { sender, receiver, capacity, .. } => {
                if let Some(c) = capacity {
                    self.scan_expr(c);
                }
                self.kill_path(&sender.node);
                self.kill_path(&receiver.node);
                self.undefine(&sender.node);
                self.undefine(&receiver.node);
                false
            }
            Stmt::Select { arms, default, after } => {
                self.havoc();
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
                self.push_scope();
                self.walk_block(&body.node);
                self.pop_scope();
                self.havoc();
                false
            }
            Stmt::Yield { value } => {
                self.scan_expr(value);
                // The consumer runs between yields and may call back into
                // reachable objects.
                self.kill_fields();
                false
            }
            Stmt::Serve { service, port } => {
                self.scan_expr(service);
                self.scan_expr(port);
                self.kill_fields();
                false
            }
        }
    }

    /// `let`/`=` binding: record the best-effort type, and a pure-value
    /// alias when the value resolves affinely with no unsafe calls.
    fn bind_value(&mut self, name: &str, value: &Spanned<Expr>) {
        self.undefine(name);
        if let Some(ty) = self.type_of_expr(&value.node) {
            self.define(name, ty);
        }
        if !self.has_unsafe_call(&value.node) {
            if let Some(aff) = to_affine_with(&value.node, &|e| self.resolve(e)) {
                self.aliases.insert(name.to_string(), aff);
            }
        }
    }

    fn handle_if(
        &mut self,
        condition: &Spanned<Expr>,
        then_block: &Block,
        else_block: Option<&Block>,
    ) -> bool {
        let cf = if self.has_unsafe_call(&condition.node) {
            super::facts::CondFacts::default()
        } else {
            condition_facts_with(&condition.node, &|e| self.resolve(e))
        };
        let then_terminates = super::block_always_terminates(then_block);
        let else_terminates = else_block.is_some_and(super::block_always_terminates);
        let mark = self.facts.kill_mark();

        self.push_scope();
        for f in &cf.then_facts {
            self.facts.assume(f.clone());
        }
        self.walk_block(then_block);
        self.pop_scope();

        if let Some(eb) = else_block {
            self.push_scope();
            for f in &cf.else_facts {
                self.facts.assume(f.clone());
            }
            self.walk_block(eb);
            self.pop_scope();
        }

        // Guard narrowing: if exactly one path terminates, the surviving
        // path's facts hold for the rest of the enclosing block — unless a
        // branch killed their paths meanwhile.
        let surviving = if then_terminates && !else_terminates {
            Some(&cf.else_facts)
        } else if else_terminates && !then_terminates {
            Some(&cf.then_facts)
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

    // ── Write sites ──────────────────────────────────────────────────────

    /// Guard specs for `class.field`, if any.
    fn guard_specs(&self, class: &str, field: &str) -> Vec<&'a GuardSpec> {
        self.env
            .guarded_fields
            .get(class)
            .map(|specs| specs.iter().filter(|s| s.field_name == field).collect())
            .unwrap_or_default()
    }

    /// Is `field` a guarded field name of ANY class? (The conservative
    /// fallback when the written object's class cannot be determined.)
    fn any_guard_named(&self, field: &str) -> Option<&'a GuardSpec> {
        self.env
            .guarded_fields
            .values()
            .flatten()
            .find(|s| s.field_name == field)
    }

    // ── Collection aliasing ban (issue #418) ─────────────────────────────

    /// If `expr` is a field access reaching an aliasable (collection-shaped)
    /// guarded field, return its spec and whether the access goes through
    /// `self`. Untypable receivers fall back to by-name matching — the
    /// closed write-set must stay closed conservatively.
    fn aliasable_guard_access(&self, expr: &Expr) -> Option<(&'a GuardSpec, bool)> {
        let Expr::FieldAccess { object, field } = expr else {
            return None;
        };
        if matches!(&object.node, Expr::Ident(r) if r == "self") {
            let owner = self.owner_class.as_deref()?;
            return self
                .guard_specs(owner, &field.node)
                .into_iter()
                .find(|s| s.aliasable)
                .map(|s| (s, true));
        }
        match self.type_of_expr(&object.node) {
            Some(PlutoType::Class(c)) => self
                .guard_specs(&c, &field.node)
                .into_iter()
                .find(|s| s.aliasable)
                .map(|s| (s, false)),
            Some(_) => None,
            None => self
                .env
                .guarded_fields
                .values()
                .flatten()
                .find(|s| s.field_name == field.node && s.aliasable)
                .map(|s| (s, false)),
        }
    }

    /// Builtin collection methods that only read their receiver — allowed
    /// on an aliasable guarded field from anywhere. Everything else is
    /// treated as mutation (conservative for methods this list has not
    /// surveyed).
    const GUARD_PURE_METHODS: &'static [&'static str] = &[
        "len",
        "contains",
        "index_of",
        "last_index_of",
        "is_empty",
        "get",
        "keys",
        "values",
        "to_array",
        "to_string",
        "first",
        "last",
        "slice",
        "byte_at",
    ];

    /// Does this expression produce a freshly-allocated collection no other
    /// binding can hold? Collection literals, and fresh-producing builtin
    /// conversions (resolved as builtins — a user method of the same name
    /// does not qualify).
    fn is_fresh_collection_value(&self, e: &Expr) -> bool {
        match e {
            Expr::ArrayLit { .. } | Expr::MapLit { .. } | Expr::SetLit { .. } => true,
            Expr::MethodCall { method, .. } => {
                let fresh_name = matches!(
                    method.node.as_str(),
                    "to_bytes" | "to_array" | "keys" | "values" | "slice" | "split"
                );
                let key = (self.current_fn.clone(), method.span.start);
                fresh_name
                    && matches!(
                        self.env.method_resolutions.get(&key),
                        Some(MethodResolution::Builtin)
                    )
            }
            _ => false,
        }
    }

    fn bare_use_error(&mut self, spec: &GuardSpec, span: Span) {
        if self.error.is_none() {
            self.error = Some(CompileError::type_err(
                format!(
                    "guarded field '{}' of class '{}' may not be used as a value \
                     (bound, passed, returned, iterated, or interpolated): it is \
                     protected by {}, and an alias to its contents could be mutated \
                     outside the fence's closed write-set (e.g. 'let d = self.{}' then \
                     'd.push(...)'). Read it through '.len()'/'.contains(...)'/indexing, \
                     and mutate it only through 'self.{}' inside '{}''s own methods{}",
                    spec.field_name,
                    spec.class_name,
                    spec.desc,
                    spec.field_name,
                    spec.field_name,
                    spec.class_name,
                    spec.blame()
                ),
                span,
            ));
        }
    }

    fn freshness_error(&mut self, spec: &GuardSpec, span: Span) {
        if self.error.is_none() {
            self.error = Some(CompileError::type_err(
                format!(
                    "guarded field '{}' of class '{}' must be initialized and assigned \
                     from a freshly-created collection (a collection literal, or a \
                     fresh-producing builtin like '.to_bytes()'/'.to_array()'): sharing \
                     a value another binding holds would alias the contents protected \
                     by {}, and the alias could mutate them outside the fence's closed \
                     write-set{}",
                    spec.field_name,
                    spec.class_name,
                    spec.desc,
                    spec.blame()
                ),
                span,
            ));
        }
    }

    /// Enforce the aliasing ban on collection-shaped guarded fields over an
    /// expression tree (modeled on idempotency.rs's `misuse_scan`): pure
    /// builtin reads from anywhere, mutating builtins only through `self`
    /// under the dominance obligation, element reads through indexing, and
    /// no bare-value use. Nested statement blocks (if/match branches,
    /// closures, catch handlers) are scanned when walked.
    fn alias_scan(&mut self, expr: &Spanned<Expr>) {
        if self.error.is_some() {
            return;
        }
        match &expr.node {
            Expr::MethodCall { object, method, args, .. } => {
                if let Some((spec, through_self)) = self.aliasable_guard_access(&object.node) {
                    let spec = spec.clone();
                    if Self::GUARD_PURE_METHODS.contains(&method.node.as_str()) {
                        // Reads are fine from anywhere.
                    } else if !through_self {
                        self.foreign_write_error(&spec, expr.span);
                        return;
                    } else {
                        // Mutating the field's contents through `self` is a
                        // write to the field: the dominance obligation.
                        self.prove_guard(&spec, expr.span);
                        if self.error.is_some() {
                            return;
                        }
                    }
                    if let Expr::FieldAccess { object: fobj, .. } = &object.node {
                        self.alias_scan(fobj);
                    }
                } else {
                    self.alias_scan(object);
                }
                for a in args {
                    self.alias_scan(a);
                }
            }
            Expr::FieldAccess { .. } => {
                if let Some((spec, _)) = self.aliasable_guard_access(&expr.node) {
                    let spec = spec.clone();
                    self.bare_use_error(&spec, expr.span);
                    return;
                }
                if let Expr::FieldAccess { object, .. } = &expr.node {
                    self.alias_scan(object);
                }
            }
            Expr::Index { object, index } => {
                // Element reads through the guarded field are fine: element
                // types are immutable value shapes by declaration-time
                // validation, so nothing mutable leaks.
                if self.aliasable_guard_access(&object.node).is_some() {
                    if let Expr::FieldAccess { object: fobj, .. } = &object.node {
                        self.alias_scan(fobj);
                    }
                } else {
                    self.alias_scan(object);
                }
                self.alias_scan(index);
            }
            Expr::StructLit { name, fields, .. } => {
                let fresh_violation: Option<GuardSpec> = self
                    .env
                    .guarded_fields
                    .get(&name.node)
                    .and_then(|specs| {
                        fields.iter().find_map(|(fname, value)| {
                            specs
                                .iter()
                                .find(|s| s.field_name == fname.node && s.aliasable)
                                .filter(|_| !self.is_fresh_collection_value(&value.node))
                                .cloned()
                        })
                    });
                if let Some(spec) = fresh_violation {
                    let bad = fields
                        .iter()
                        .find(|(fname, _)| fname.node == spec.field_name)
                        .map(|(_, v)| v.span)
                        .unwrap_or(expr.span);
                    self.freshness_error(&spec, bad);
                    return;
                }
                for (_, v) in fields {
                    self.alias_scan(v);
                }
            }
            Expr::BinOp { lhs, rhs, .. } | Expr::NullCoalesce { lhs, rhs } => {
                self.alias_scan(lhs);
                self.alias_scan(rhs);
            }
            Expr::CompareChain { operands, .. } => {
                for operand in operands {
                    self.alias_scan(operand);
                }
            }
            Expr::UnaryOp { operand, .. } => self.alias_scan(operand),
            Expr::Call { args, .. }
            | Expr::StaticTraitCall { args, .. }
            | Expr::At { args, .. } => {
                for a in args {
                    self.alias_scan(a);
                }
            }
            Expr::Spawn { call } => self.alias_scan(call),
            Expr::EnumData { fields, .. } => {
                for (_, v) in fields {
                    self.alias_scan(v);
                }
            }
            Expr::ArrayLit { elements, .. } | Expr::SetLit { elements, .. } => {
                for e in elements {
                    self.alias_scan(e);
                }
            }
            Expr::MapLit { entries, .. } => {
                for (k, v) in entries {
                    self.alias_scan(k);
                    self.alias_scan(v);
                }
            }
            Expr::StringInterp { parts } => {
                for p in parts {
                    if let crate::parser::ast::StringInterpPart::Expr(e) = p {
                        self.alias_scan(e);
                    }
                }
            }
            Expr::Propagate { expr }
            | Expr::NullPropagate { expr }
            | Expr::Cast { expr, .. } => self.alias_scan(expr),
            Expr::Catch { expr, handlers } => {
                self.alias_scan(expr);
                for h in handlers {
                    if let CatchHandler::Shorthand(e) = h {
                        self.alias_scan(e);
                    }
                    // Block handlers are statement blocks: scanned when
                    // walked.
                }
            }
            Expr::Range { start, end, .. } => {
                self.alias_scan(start);
                self.alias_scan(end);
            }
            Expr::If { condition, .. } => self.alias_scan(condition),
            Expr::Match { expr, arms } => {
                self.alias_scan(expr);
                for arm in arms {
                    self.alias_scan(&arm.value);
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

    fn field_assign(
        &mut self,
        object: &Spanned<Expr>,
        field: &Spanned<String>,
        value: &Spanned<Expr>,
        span: Span,
    ) {
        // `self.<f> = v`: the receiver is authoritative — we always know
        // whose field is written, whether or not the owner is a registered
        // class (app and stage receivers are never guarded classes).
        if matches!(&object.node, Expr::Ident(s) if s == "self") {
            let Some(owner) = self.owner_class.clone() else {
                // app / stage / served receiver: no guarded class here.
                self.kill_fields();
                return;
            };
            let specs: Vec<GuardSpec> = self
                .guard_specs(&owner, &field.node)
                .into_iter()
                .cloned()
                .collect();
            for spec in &specs {
                self.prove_guard(spec, span);
            }
            if self.error.is_none() {
                // Aliasing-in (issue #418): assigning a value another
                // binding can still reach would let that binding mutate the
                // guarded contents outside the fence afterwards.
                if let Some(spec) = specs.iter().find(|s| s.aliasable) {
                    if !self.is_fresh_collection_value(&value.node) {
                        self.freshness_error(spec, span);
                        return;
                    }
                }
            }
            if self.error.is_none() {
                self.kill_field_write("self", &owner, &field.node);
            }
            return;
        }
        // Any other target: resolve its class (typing only — entity-typed
        // roots are fine here; the entity exclusion in `path_of` is about
        // fact soundness, not write identification). A resolved guarded
        // field is a foreign write (rejected); an unresolvable target whose
        // field name matches a guarded field is rejected conservatively —
        // the closed write-set must stay closed.
        match self.type_of_expr(&object.node) {
            Some(PlutoType::Class(class)) => {
                let specs = self.guard_specs(&class, &field.node);
                if let Some(spec) = specs.first() {
                    let spec = (*spec).clone();
                    self.foreign_write_error(&spec, span);
                    return;
                }
                match self.path_of(&object.node) {
                    Some((path, _)) => self.kill_field_write(&path, &class, &field.node),
                    None => self.kill_fields(),
                }
            }
            _ => {
                if let Some(spec) = self.any_guard_named(&field.node) {
                    let spec = spec.clone();
                    self.error = Some(CompileError::type_err(
                        format!(
                            "cannot verify that this write to '{}' does not target the \
                             guarded field {} of class '{}': the assignment target is not \
                             a trackable path. Bind the object to a local variable first \
                             (let o = ...; o.{} = ...), or rename the field{}",
                            field.node, spec.desc, spec.class_name, field.node, spec.blame()
                        ),
                        span,
                    ));
                    return;
                }
                self.kill_fields();
            }
        }
    }

    fn index_assign(&mut self, object: &Spanned<Expr>, span: Span) {
        // `self.data[i] = v` mutates the guarded field's contents: same
        // obligation as a direct write. Walk the object path looking for a
        // guarded field step.
        if let Some((path, _)) = self.path_of(&object.node) {
            let segs: Vec<&str> = path.split('.').collect();
            if segs.len() == 2 && segs[0] == "self" {
                if let Some(owner) = self.owner_class.clone() {
                    let specs: Vec<GuardSpec> = self
                        .guard_specs(&owner, segs[1])
                        .into_iter()
                        .cloned()
                        .collect();
                    for spec in &specs {
                        self.prove_guard(spec, span);
                    }
                }
            } else if let Expr::FieldAccess { field, .. } = &object.node {
                if let Some(spec) = self.any_guard_named(&field.node) {
                    let spec = spec.clone();
                    self.foreign_write_error(&spec, span);
                }
            }
        } else if let Expr::FieldAccess { field, .. } = &object.node {
            if let Some(spec) = self.any_guard_named(&field.node) {
                let spec = spec.clone();
                self.foreign_write_error(&spec, span);
            }
        }
        if self.error.is_none() {
            self.kill_fields();
        }
    }

    fn foreign_write_error(&mut self, spec: &GuardSpec, span: Span) {
        self.error = Some(CompileError::type_err(
            format!(
                "guarded field '{}' of class '{}' may only be written through 'self' \
                 inside the class's own methods: it is protected by {}, and the \
                 dominance proof is only meaningful under the class's own control flow. \
                 Add a method on '{}' that takes the evidence and performs the write{}",
                spec.field_name, spec.class_name, spec.desc, spec.class_name, spec.blame()
            ),
            span,
        ));
    }

    /// The dominance obligation at a write site: some in-scope value of the
    /// binder type must make the predicate provable from the facts live
    /// here.
    fn prove_guard(&mut self, spec: &GuardSpec, span: Span) {
        // Candidates: in-scope variables of the binder class, innermost
        // binding of each name.
        let mut seen = std::collections::HashSet::new();
        let mut candidates: Vec<String> = Vec::new();
        for scope in self.vars.iter().rev() {
            for (name, ty) in scope {
                if !seen.insert(name.clone()) || name == "self" {
                    continue;
                }
                if matches!(ty, PlutoType::Class(c) if *c == spec.binder_class) {
                    candidates.push(name.clone());
                }
            }
        }
        candidates.sort();

        if candidates.is_empty() {
            self.error = Some(CompileError::type_err(
                format!(
                    "cannot prove guard {} of class '{}' at this write: no value of the \
                     binder type '{}' is in scope. The guard needs evidence — take a \
                     '{}' parameter (or bind one) and check it before the write{}",
                    spec.desc, spec.class_name, spec.binder_class, spec.binder_class, spec.blame()
                ),
                span,
            ));
            return;
        }

        for cand in &candidates {
            let resolver = |e: &Expr| -> Option<Affine> {
                if let Expr::FieldAccess { object, field } = e {
                    if let Expr::Ident(root) = &object.node {
                        if root == &spec.binder_name {
                            return Some(Affine::term(format!("{cand}.{}", field.node)));
                        }
                        if root == "self" {
                            return Some(Affine::term(format!("self.{}", field.node)));
                        }
                    }
                }
                None
            };
            if eval_condition_with(&spec.predicate.node, &resolver, &self.facts)
                == Verdict::Proven
            {
                return; // dominated: some evidence satisfies the guard here
            }
        }

        let cand_list = candidates.join("', '");
        self.error = Some(CompileError::type_err(
            format!(
                "cannot prove guard {} of class '{}' at this write: no dominating check \
                 implies the predicate for any in-scope '{}' value (tried '{cand_list}'). \
                 Every write to a guarded field must be dominated by a conditional the \
                 prover can carry to the write — e.g. a check of '{cand0}' whose failing \
                 path raises or returns, placed before the write — and the facts it \
                 establishes must survive to the write site (they are invalidated by \
                 calls that may run user code, loop boundaries, and writes to the \
                 compared fields){}",
                spec.desc,
                spec.class_name,
                spec.binder_class,
                spec.blame(),
                cand0 = candidates[0]
            ),
            span,
        ));
    }
}
