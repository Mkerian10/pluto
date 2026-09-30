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
//! ("invariant is outside the provable fragment"). Invariants on generic
//! classes are not yet supported (their bodies are checked against skolem
//! types and their instantiations are never re-checked).
//!
//! # Obligation sites
//!
//! - **Construction**: each struct literal of an invariant-carrying class
//!   must prove the invariant from the field initializers (substituted
//!   into the invariant and evaluated against the flow facts in scope).
//! - **Foreign field writes** (`obj.field = v` anywhere outside the
//!   class's own `mut self` methods): the invariant must be proven
//!   *immediately* after the write — external code gets no
//!   temporary-violation window.
//! - **`mut self` method bodies**: writes to `self.field` perform a
//!   symbolic strong update (the field's value is tracked as an affine
//!   form over "ghost" variables — entry values of fields and locals), and
//!   the invariant must be proven at every *boundary*: method exits
//!   (return / fall-through), `raise`, `break`/`continue`, `yield`, any
//!   statement containing a call (the callee — or anyone holding an alias
//!   — may observe the object: this is the conservative slice-1 answer to
//!   reentrancy, rfc-objects.md open question 6), loop entry/body-end, and
//!   branch joins where a surviving branch changed the symbolic state.
//!   *Between* those boundaries the invariant may be temporarily broken
//!   (subtract-then-add works because the symbolic forms cancel).
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
//! establishes facts for the remainder of the block. After any call, facts
//! about fields are reduced to invariant-level knowledge (anything finer
//! may have been invalidated by the callee).
//!
//! False rejection is the failure mode to fear: every Unknown verdict is a
//! compile error under strict mode, so diagnostics name the invariant, the
//! site, the symbolic state, and the fix (add a guard / requires /
//! assert).

use std::collections::{HashMap, HashSet};

use crate::diagnostics::CompileError;
use crate::parser::ast::{
    BinOp, Block, ContractKind, Expr, Function, Program, Stmt, UnaryOp,
};
use crate::span::{Span, Spanned};
use crate::visit::{walk_expr, walk_stmt, Visitor};

use super::env::TypeEnv;
use super::facts::{
    condition_facts, condition_facts_with, contains_call, eval_condition_with, immediate_exprs,
    to_affine, to_affine_with, typed_path, Affine, FactEnv, Verdict,
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
}

/// Type-check and fragment-validate every class invariant, and register the
/// provable specs in `env.class_invariants`. Runs before any body checking
/// so obligations can be enforced at every site.
pub(crate) fn register_invariants(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for class in &program.classes {
        let c = &class.node;
        if c.invariants.is_empty() {
            continue;
        }
        if !c.type_params.is_empty() {
            return Err(CompileError::type_err(
                format!(
                    "invariants on generic classes are not yet supported: invariants are \
                     compile-time proof obligations, and generic bodies are checked against \
                     opaque type parameters. Declare the invariant on a concrete class \
                     wrapping '{}' instead",
                    c.name.node
                ),
                c.invariants[0].span,
            ));
        }
        // Type-check the invariant expressions with `self` in scope.
        env.push_scope();
        env.define_unchecked("self".to_string(), PlutoType::Class(c.name.node.clone()));
        for inv in &c.invariants {
            let inv_type =
                super::infer::infer_expr(&inv.node.expr.node, inv.node.expr.span, env, None)?;
            if inv_type != PlutoType::Bool {
                env.pop_scope();
                return Err(CompileError::type_err(
                    format!("invariant expression must be bool, found {inv_type}"),
                    inv.node.expr.span,
                ));
            }
        }
        env.pop_scope();

        // Provable-fragment validation.
        let mut specs = Vec::new();
        for inv in &c.invariants {
            let desc = crate::codegen::format_invariant_expr(&inv.node.expr.node);
            validate_provable(&inv.node.expr, &c.name.node, &desc, env)?;
            specs.push(InvariantSpec {
                expr: inv.node.expr.node.clone(),
                desc,
                span: inv.node.expr.span,
            });
        }
        env.class_invariants.insert(c.name.node.clone(), specs);
    }
    Ok(())
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
fn validate_provable(
    expr: &Spanned<Expr>,
    class_name: &str,
    desc: &str,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    match &expr.node {
        Expr::BinOp { op: BinOp::And | BinOp::Or, lhs, rhs } => {
            validate_provable(lhs, class_name, desc, env)?;
            validate_provable(rhs, class_name, desc, env)
        }
        Expr::UnaryOp { op: UnaryOp::Not, operand } => {
            validate_provable(operand, class_name, desc, env)
        }
        Expr::BinOp {
            op: BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq | BinOp::Eq | BinOp::Neq,
            lhs,
            rhs,
        } => {
            validate_affine_side(lhs, class_name, desc, env)?;
            validate_affine_side(rhs, class_name, desc, env)
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
/// over direct int fields of `self`.
fn validate_affine_side(
    expr: &Spanned<Expr>,
    class_name: &str,
    desc: &str,
    env: &TypeEnv,
) -> Result<(), CompileError> {
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
            let fty = env
                .classes
                .get(class_name)
                .and_then(|ci| ci.fields.iter().find(|(n, _, _)| *n == field.node))
                .map(|(_, t, _)| t.clone());
            match fty {
                Some(PlutoType::Int) => Ok(()),
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
        Expr::MethodCall { method, .. } => Err(fragment_err(
            format!(
                "'.{}()' — collection and method facts are not provable",
                method.node
            ),
            desc,
            expr.span,
        )),
        Expr::UnaryOp { op: UnaryOp::Neg, operand } => {
            validate_affine_side(operand, class_name, desc, env)
        }
        Expr::BinOp { op: BinOp::Add | BinOp::Sub, lhs, rhs } => {
            validate_affine_side(lhs, class_name, desc, env)?;
            validate_affine_side(rhs, class_name, desc, env)
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
            validate_affine_side(lhs, class_name, desc, env)?;
            validate_affine_side(rhs, class_name, desc, env)
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
    /// Facts over the ghost vocabulary.
    pub ghost_facts: FactEnv,
    /// Current symbolic value of each int field of the class.
    sym: HashMap<String, Affine>,
    /// All int fields of the class (the sym key set).
    fields: Vec<String>,
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
}

impl InvariantScope {
    fn dirty(&self) -> bool {
        !self.touched.is_empty()
    }

    fn local_ghost(&self, name: &str) -> String {
        let v = self.local_ver.get(name).copied().unwrap_or(0);
        format!("{name}@{v}")
    }

    /// Resolve a leaf expression into the ghost vocabulary: `self.f` reads
    /// its current symbolic value, an int local reads its current version
    /// ghost. Anything else (foreign paths, calls) is unresolvable.
    pub(crate) fn resolve(&self, env: &TypeEnv, e: &Expr) -> Option<Affine> {
        match e {
            Expr::FieldAccess { object, field }
                if matches!(&object.node, Expr::Ident(s) if s == "self") =>
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
            _ => None,
        }
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

/// The fields an expression mentions as `self.<field>`.
fn mentioned_fields(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
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
    for spec in &scope.invariants {
        let fields = spec_fields(spec);
        if fields.is_disjoint(&scope.touched) {
            // Untouched since the last boundary — holds by assumption.
            continue;
        }
        let s: &InvariantScope = scope;
        let verdict =
            eval_condition_with(&spec.expr, &|e| s.resolve(env, e), &s.ghost_facts);
        match verdict {
            Verdict::Proven => {}
            Verdict::Refuted => {
                return Err(CompileError::type_err(
                    format!(
                        "invariant '{}' of class '{}' is violated at {site} in method '{}': {}",
                        spec.desc,
                        scope.class_name,
                        scope.method_name,
                        symbolic_state(scope, &fields),
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
                         'assert' (e.g. 'if amt <= self.balance {{ ... }}')",
                        spec.desc,
                        scope.class_name,
                        scope.method_name,
                        symbolic_state(scope, &fields),
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
            format!("self.{f} = {rendered}")
        })
        .collect();
    parts.sort();
    if parts.is_empty() {
        "the known facts do not decide it".to_string()
    } else {
        format!("at this point {}", parts.join(", "))
    }
}

/// Re-anchor every field on fresh ghosts carrying exactly the
/// invariant-level facts. Used at boundaries after the invariant has been
/// (re-)established, and after calls (which may invalidate anything finer).
fn re_anchor(scope: &mut InvariantScope, env: &TypeEnv) {
    scope.next_ghost += 1;
    let n = scope.next_ghost;
    for f in &scope.fields {
        scope
            .sym
            .insert(f.clone(), Affine::term(format!("self.{f}@{n}")));
    }
    scope.touched.clear();
    let mut to_assume = Vec::new();
    {
        let s: &InvariantScope = scope;
        for spec in &s.invariants {
            to_assume
                .extend(condition_facts_with(&spec.expr, &|e| s.resolve(env, e)).then_facts);
        }
    }
    for f in to_assume {
        scope.ghost_facts.assume(f);
    }
}

fn checkpoint_scope(env: &mut TypeEnv, span: Span, site: &str) -> Result<(), CompileError> {
    let Some(mut scope) = env.invariant_scope.take() else {
        return Ok(());
    };
    let r = checkpoint(&mut scope, env, span, site);
    env.invariant_scope = Some(scope);
    r
}

fn re_anchor_scope(env: &mut TypeEnv) {
    let Some(mut scope) = env.invariant_scope.take() else {
        return;
    };
    re_anchor(&mut scope, env);
    env.invariant_scope = Some(scope);
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
        if c.node.kind != ContractKind::Requires || contains_call(&c.node.expr) {
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

    // Ghost proof scope for mut-self methods of invariant-carrying classes.
    let Some(cn) = class_name else { return Ok(()) };
    if !func.params.iter().any(|p| p.name.node == "self" && p.is_mut) {
        return Ok(());
    }
    let Some(specs) = env.class_invariants.get(cn) else {
        return Ok(());
    };
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
    let mut scope = InvariantScope {
        class_name: cn.to_string(),
        method_name: func.name.node.clone(),
        invariants: specs.clone(),
        ghost_facts: FactEnv::new(),
        sym: HashMap::new(),
        fields,
        touched: HashSet::new(),
        local_ver: HashMap::new(),
        next_ghost: 0,
    };
    for f in &scope.fields {
        scope
            .sym
            .insert(f.clone(), Affine::term(format!("self.{f}@0")));
    }
    let mut ghost_assume = Vec::new();
    {
        let s: &InvariantScope = &scope;
        for spec in &s.invariants {
            ghost_assume
                .extend(condition_facts_with(&spec.expr, &|e| s.resolve(env, e)).then_facts);
        }
        for c in &func.contracts {
            if c.node.kind != ContractKind::Requires || contains_call(&c.node.expr) {
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

/// Prove the invariant at the method's fall-through exit and drop the proof
/// scope. Called after the body is checked.
pub(crate) fn function_exit(env: &mut TypeEnv, span: Span) -> Result<(), CompileError> {
    let r = checkpoint_scope(env, span, "the end of this method");
    env.invariant_scope = None;
    r
}

// ─────────────────────────────────────────────────────────────────────────────
// Statement hooks
// ─────────────────────────────────────────────────────────────────────────────

/// Obligations that need the *pre-statement* fact state. Runs before
/// `apply_stmt_kills`.
pub(crate) fn pre_stmt(stmt: &Stmt, span: Span, env: &mut TypeEnv) -> Result<(), CompileError> {
    if env.class_invariants.is_empty() || is_exempt_fn(env) {
        return Ok(());
    }

    // Call boundary: a statement performing any call may let the callee (or
    // anyone holding an alias) observe the object — the invariant must hold
    // here, and afterwards only invariant-level facts survive.
    if env.invariant_scope.is_some()
        && immediate_exprs(stmt).iter().any(|e| contains_call(e))
    {
        checkpoint_scope(env, span, "this call (the callee may observe the object)")?;
        re_anchor_scope(env);
    }

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
        Stmt::Return(_) => checkpoint_scope(env, span, "this return")?,
        Stmt::Raise { .. } => checkpoint_scope(env, span, "this raise")?,
        Stmt::Break => checkpoint_scope(env, span, "this break")?,
        Stmt::Continue => checkpoint_scope(env, span, "this continue")?,
        Stmt::Yield { .. } => {
            checkpoint_scope(env, span, "this yield")?;
            re_anchor_scope(env);
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
        | Stmt::Serve { .. }
        | Stmt::Expr(_) => {}
    }
    Ok(())
}

/// Fact updates that follow a statement. Runs after the statement is fully
/// checked (so new bindings exist and branch merges are done).
pub(crate) fn post_stmt(stmt: &Stmt, env: &mut TypeEnv) -> Result<(), CompileError> {
    if env.class_invariants.is_empty() || is_exempt_fn(env) {
        return Ok(());
    }
    let had_call = immediate_exprs(stmt).iter().any(|e| contains_call(e));
    match stmt {
        Stmt::Let { name, .. } => {
            // A fresh class-typed binding satisfies its invariants (its
            // construction or producing call was proven/validated).
            if had_call {
                reassume_invariants_main(env);
            }
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
            if !contains_call(expr) {
                let cf = condition_facts(&expr.node, env);
                for f in cf.then_facts {
                    env.facts.assume(f);
                }
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
            re_anchor_scope(env);
            reassume_invariants_main(env);
        }
        Stmt::Assign { .. }
        | Stmt::Return(_)
        | Stmt::If { .. }
        | Stmt::While { .. }
        | Stmt::For { .. }
        | Stmt::Match { .. }
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
    Ok(())
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
    let Some(specs) = env.class_invariants.get(&cls) else {
        return Ok(());
    };
    let specs = specs.clone();

    // Strong update inside the class's own mut-self method.
    if opath.as_deref() == Some("self") {
        if let Some(scope) = env.invariant_scope.as_ref() {
            if scope.class_name == cls {
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
    // object's path (whose facts are the entry/boundary assumptions).
    let val_aff = to_affine(&value.node, env);
    for spec in &specs {
        let resolve = |e: &Expr| match e {
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
        };
        match eval_condition_with(&spec.expr, &resolve, &env.facts) {
            Verdict::Proven => {}
            Verdict::Refuted => {
                return Err(CompileError::type_err(
                    format!(
                        "this write to '{opath}.{}' violates invariant '{}' of class '{cls}'",
                        field.node, spec.desc
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
                         'mut self' method of '{cls}'",
                        spec.desc, field.node, field.node
                    ),
                    span,
                ));
            }
        }
    }
    Ok(())
}

fn strong_update(
    scope: &mut InvariantScope,
    env: &TypeEnv,
    field: &str,
    value: &Expr,
    specs: &[InvariantSpec],
) {
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

// ─────────────────────────────────────────────────────────────────────────────
// Branch and loop hooks
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn branch_snapshot(env: &TypeEnv) -> Option<SymSnapshot> {
    env.invariant_scope.as_ref().map(|s| SymSnapshot {
        sym: s.sym.clone(),
        touched: s.touched.clone(),
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
/// fall-through path — and the state re-anchors on invariant-level facts.
/// Otherwise the pre-branch state is restored unchanged.
pub(crate) fn branch_join(
    env: &mut TypeEnv,
    snap: Option<SymSnapshot>,
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
        branch_restore(env, &Some(sn));
        checkpoint_scope(env, span, "this branch join")?;
    }
    re_anchor_scope(env);
    reassume_invariants_main(env);
    Ok(())
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
        re_anchor_scope(env);
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

/// After the loop: back to invariant-level knowledge.
pub(crate) fn loop_exit(env: &mut TypeEnv, affects: bool) {
    if affects {
        re_anchor_scope(env);
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
    if is_exempt_fn(env) {
        return Ok(());
    }
    let specs = specs.clone();
    let mut inits: HashMap<String, Option<Affine>> = HashMap::new();
    for (n, v) in lit_fields {
        inits.insert(n.node.clone(), to_affine(&v.node, env));
    }
    for spec in &specs {
        let resolve = |e: &Expr| match e {
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
                        "construction of '{class_name}' violates its invariant '{}'",
                        spec.desc
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
                         expressions the prover can bound",
                        spec.desc
                    ),
                    span,
                ));
            }
        }
    }
    Ok(())
}
