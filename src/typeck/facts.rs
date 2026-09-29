//! Flow-fact engine — phase 1 of the verification RFC
//! (docs/design/rfc-verification.md).
//!
//! Pluto already does flow-sensitive narrowing for exactly one predicate:
//! nullability. This module generalizes the *domain* from nullability to
//! integer comparison facts, mirroring the structure and conservatism of the
//! nullable-narrowing machinery in `check.rs`.
//!
//! # Fact domain
//!
//! Facts are tracked about *paths*: local `int` variables (`x`) and simple
//! field paths rooted at a local (`self.balance`, `p.x.y`) whose every step
//! is a plain class field and whose final type is `int`. Entities (`object`
//! types), remote types, and domain deps are excluded — their fields can
//! change concurrently, so a flow fact about them is never sound.
//!
//! Two fact shapes are tracked:
//!
//! - **Interval bounds** over i64: `x <= 5`, `x > 0` (`Fact::Bound`).
//! - **Binary relations** between two in-scope paths: `amt <= balance`
//!   (`Fact::Rel`, ops `<`, `<=`, `==`, `!=`).
//!
//! # Where facts come from
//!
//! - `if` conditions: the then-branch assumes the condition's facts, the
//!   else-branch its negation.
//! - Guards whose failure path terminates (`if x > 10 { return }` — mirrors
//!   the existing none-guard logic): the surviving facts hold for the rest
//!   of the enclosing block, *except* for paths killed while checking the
//!   branches (a reassignment inside the surviving branch invalidates them).
//! - `&&` decomposition (both conjuncts' facts in the then-branch) and, by
//!   De Morgan, `||` decomposition in the else-branch.
//!
//! No facts are extracted from a condition that contains a call-like
//! expression (the call could mutate state between evaluation and use).
//!
//! # What kills facts
//!
//! - Reassignment of a variable kills its facts and facts about paths
//!   rooted at it (`x` kills `x` and `x.f`).
//! - Any field assignment kills *all* field-path facts (aliasing: two locals
//!   can point at the same object, so a write through one invalidates facts
//!   about the other).
//! - Any statement whose immediate expressions contain a call-like node
//!   (call, method call, static trait call, `at`, `spawn`) kills all
//!   field-path facts — the callee may mutate any reachable object. Local
//!   variables survive calls: parameters are passed by value and `mut`
//!   params are local copies, so no call can change a caller's local.
//! - Loop entry (`while` / `for`) drops **all** facts, and they stay dropped
//!   after the loop. This is deliberately conservative for phase 1 — the
//!   alternative (analyze loop bodies with facts that survive a fixpoint) is
//!   documented future work. Facts established *inside* a loop body by
//!   guards remain valid for the rest of that body (a `continue`/`break`
//!   terminator behaves like `return` for guard purposes).
//! - Closure bodies are checked under a fact barrier: outer facts are
//!   invisible inside the body (the closure runs later, when field facts may
//!   no longer hold), and facts do not escape it. Captures are by value, so
//!   the creation itself invalidates nothing — the outer state is restored
//!   after the body is checked.
//!
//! Kills are applied *through* the scope stack (a kill inside a then-branch
//! also removes the fact for the else-branch and the rest of the enclosing
//! block). This is conservative: the merged post-`if` state may not assume a
//! fact one branch invalidated, and phase 1 does not track per-branch
//! out-states.
//!
//! # The decidable fragment (`eval_condition`)
//!
//! `eval_condition` answers `Proven` / `Refuted` / `Unknown` for boolean
//! expressions built from `&&`, `||`, `!`, and integer comparisons whose
//! sides normalize to *affine* forms: sums of `coefficient * path` terms
//! plus a constant, with all coefficients integer constants (linear
//! arithmetic with constant coefficients only — no `x * y`). A comparison
//! `A cmp B` is decided by bounding `A - B` using the interval facts of
//! every path involved (and relation facts when the difference is exactly
//! `x - y`). All internal arithmetic is done in i128 with checked
//! operations; overflow widens to "unbounded", never to a wrong bound.
//! Anything outside the fragment is `Unknown`. **False positives are worse
//! than misses: when in doubt, every query answers `Unknown`.**
//!
//! Consumers in phase 1: degenerate-condition warnings ("condition is
//! always true/false") on `if` statements. Later phases (invariant
//! discharge, error-set shrinking) query the same API.

use std::collections::{BTreeMap, HashMap};

use crate::parser::ast::{BinOp, Expr, UnaryOp};
use crate::span::Spanned;
use crate::visit::{walk_expr, Visitor};

use super::env::TypeEnv;
use super::types::PlutoType;

// ─────────────────────────────────────────────────────────────────────────────
// Verdicts
// ─────────────────────────────────────────────────────────────────────────────

/// Answer to "does the current fact state decide this condition?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The condition is true on every execution reaching this point.
    Proven,
    /// The condition is false on every execution reaching this point.
    Refuted,
    /// The facts do not decide the condition (the safe default).
    Unknown,
}

impl Verdict {
    fn negate(self) -> Verdict {
        match self {
            Verdict::Proven => Verdict::Refuted,
            Verdict::Refuted => Verdict::Proven,
            Verdict::Unknown => Verdict::Unknown,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Intervals
// ─────────────────────────────────────────────────────────────────────────────

/// A closed interval over i64. `i64::MIN` / `i64::MAX` endpoints double as
/// "unbounded" sentinels (this only ever widens, never tightens: a genuine
/// value of `i64::MIN` is simply treated as unbounded below).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub lo: i64,
    pub hi: i64,
}

impl Interval {
    pub const TOP: Interval = Interval { lo: i64::MIN, hi: i64::MAX };
    /// Canonical empty interval (contradictory facts — unreachable code).
    pub const EMPTY: Interval = Interval { lo: i64::MAX, hi: i64::MIN };

    pub fn exactly(v: i64) -> Interval {
        Interval { lo: v, hi: v }
    }

    pub fn at_most(v: i64) -> Interval {
        Interval { lo: i64::MIN, hi: v }
    }

    pub fn at_least(v: i64) -> Interval {
        Interval { lo: v, hi: i64::MAX }
    }

    pub fn is_empty(&self) -> bool {
        self.lo > self.hi
    }

    pub fn intersect(&self, other: &Interval) -> Interval {
        Interval {
            lo: self.lo.max(other.lo),
            hi: self.hi.min(other.hi),
        }
    }

    /// Lower bound, `None` when unbounded below.
    fn lo_bound(&self) -> Option<i64> {
        (self.lo != i64::MIN).then_some(self.lo)
    }

    /// Upper bound, `None` when unbounded above.
    fn hi_bound(&self) -> Option<i64> {
        (self.hi != i64::MAX).then_some(self.hi)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Facts
// ─────────────────────────────────────────────────────────────────────────────

/// Relation operators between two paths. `Lt`/`Le` are stored directed
/// (`Rel(a, Lt, b)` means `a < b`); `Eq`/`Ne` are symmetric.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelOp {
    Lt,
    Le,
    Eq,
    Ne,
}

/// A single flow fact.
#[derive(Debug, Clone, PartialEq)]
pub enum Fact {
    /// The path's value lies within the interval.
    Bound(String, Interval),
    /// A binary relation between two paths.
    Rel(String, RelOp, String),
}

impl Fact {
    /// Every path this fact talks about.
    fn paths(&self) -> impl Iterator<Item = &str> {
        match self {
            Fact::Bound(p, _) => std::iter::once(p.as_str()).chain(None),
            Fact::Rel(a, _, b) => std::iter::once(a.as_str()).chain(Some(b.as_str())),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// The fact environment
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
struct Frame {
    intervals: HashMap<String, Interval>,
    relations: Vec<(String, RelOp, String)>,
}

/// Record of a kill, for the guard logic: a guard fact may only be assumed
/// after an `if` when its paths were not killed while the branches were
/// being checked.
#[derive(Debug, Clone)]
enum KillEvent {
    /// A path root (and everything under it) was killed.
    Path(String),
    /// All dotted (field) paths were killed.
    Fields,
    /// Everything was killed.
    All,
}

/// Opaque position in the kill log — see [`FactEnv::kill_mark`].
#[derive(Debug, Clone, Copy)]
pub struct KillMark(usize);

/// Scoped store of flow facts, mirroring the `ScopeTracker` discipline used
/// by nullable narrowing: frames are pushed/popped in lockstep with
/// `TypeEnv::push_scope` / `pop_scope`, facts assumed in a branch pop with
/// it. Kills, by contrast, are flow events, not scope events: they remove
/// facts from *every* frame so an invalidation inside a branch survives the
/// branch's scope pop.
#[derive(Debug, Clone)]
pub struct FactEnv {
    frames: Vec<Frame>,
    kill_log: Vec<KillEvent>,
}

impl Default for FactEnv {
    fn default() -> Self {
        Self::new()
    }
}

/// Does `key` equal `root` or lie under it (`root.field...`)?
fn path_under(key: &str, root: &str) -> bool {
    key == root || (key.len() > root.len() && key.starts_with(root) && key.as_bytes()[root.len()] == b'.')
}

impl FactEnv {
    pub fn new() -> Self {
        FactEnv {
            frames: vec![Frame::default()],
            kill_log: Vec::new(),
        }
    }

    pub fn push_frame(&mut self) {
        self.frames.push(Frame::default());
    }

    pub fn pop_frame(&mut self) {
        // Keep the base frame: unbalanced pops are a caller bug, but must not
        // leave the engine unable to answer queries.
        if self.frames.len() > 1 {
            self.frames.pop();
        } else if let Some(f) = self.frames.last_mut() {
            *f = Frame::default();
        }
    }

    /// Record a fact in the innermost frame.
    pub fn assume(&mut self, fact: Fact) {
        let frame = self.frames.last_mut().expect("FactEnv always has a base frame");
        match fact {
            Fact::Bound(path, iv) => {
                frame
                    .intervals
                    .entry(path)
                    .and_modify(|cur| *cur = cur.intersect(&iv))
                    .or_insert(iv);
            }
            Fact::Rel(a, op, b) => {
                if !frame.relations.contains(&(a.clone(), op, b.clone())) {
                    frame.relations.push((a, op, b));
                }
            }
        }
    }

    /// Kill every fact about `root` and any path under it, in every frame.
    pub fn kill_path(&mut self, root: &str) {
        for frame in &mut self.frames {
            frame.intervals.retain(|k, _| !path_under(k, root));
            frame
                .relations
                .retain(|(a, _, b)| !path_under(a, root) && !path_under(b, root));
        }
        self.kill_log.push(KillEvent::Path(root.to_string()));
    }

    /// Kill every fact involving a field path (any dotted path), in every
    /// frame. Applied whenever a call may have mutated reachable objects or
    /// a field was assigned (aliasing makes per-object precision unsound).
    pub fn kill_fields(&mut self) {
        for frame in &mut self.frames {
            frame.intervals.retain(|k, _| !k.contains('.'));
            frame
                .relations
                .retain(|(a, _, b)| !a.contains('.') && !b.contains('.'));
        }
        self.kill_log.push(KillEvent::Fields);
    }

    /// Drop every fact (loop entry — see module docs).
    pub fn havoc_all(&mut self) {
        for frame in &mut self.frames {
            frame.intervals.clear();
            frame.relations.clear();
        }
        self.kill_log.push(KillEvent::All);
    }

    /// Current position in the kill log. Take a mark before checking an
    /// `if`'s branches; [`Self::killed_since`] then reports whether a fact's
    /// paths were invalidated while the branches ran.
    pub fn kill_mark(&self) -> KillMark {
        KillMark(self.kill_log.len())
    }

    /// Was any path of `fact` killed since `mark`?
    pub fn killed_since(&self, mark: KillMark, fact: &Fact) -> bool {
        self.kill_log[mark.0.min(self.kill_log.len())..]
            .iter()
            .any(|ev| {
                fact.paths().any(|p| match ev {
                    KillEvent::Path(root) => path_under(p, root),
                    KillEvent::Fields => p.contains('.'),
                    KillEvent::All => true,
                })
            })
    }

    /// The tightest interval known for `path` (TOP when nothing is known,
    /// possibly empty when facts contradict — i.e. the code is unreachable).
    pub fn interval_of(&self, path: &str) -> Interval {
        let mut iv = Interval::TOP;
        for frame in &self.frames {
            if let Some(fiv) = frame.intervals.get(path) {
                iv = iv.intersect(fiv);
            }
        }
        iv
    }

    /// Is the directed relation `a op b` recorded? `Eq`/`Ne` are checked
    /// symmetrically.
    fn rel_holds(&self, a: &str, op: RelOp, b: &str) -> bool {
        self.frames.iter().any(|frame| {
            frame.relations.iter().any(|(ra, rop, rb)| {
                (*rop == op && ra == a && rb == b)
                    || (matches!(op, RelOp::Eq | RelOp::Ne) && *rop == op && ra == b && rb == a)
            })
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Paths
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve `expr` to a trackable path and its type: an identifier, or a
/// chain of plain class field accesses rooted at one. Excludes entities
/// (`object`), remote types, and domain deps at every step.
fn typed_path(expr: &Expr, env: &TypeEnv) -> Option<(String, PlutoType)> {
    match expr {
        Expr::Ident(name) => {
            // Mirror identifier inference: a flow-narrowed nullable reads at
            // its narrowed type.
            if let Some(narrowed) = env.narrowed_vars.lookup(name) {
                if !matches!(narrowed, PlutoType::Nullable(_)) {
                    return Some((name.clone(), narrowed.clone()));
                }
            }
            env.lookup(name).map(|ty| (name.clone(), ty.clone()))
        }
        Expr::FieldAccess { object, field } => {
            let (opath, oty) = typed_path(&object.node, env)?;
            let PlutoType::Class(cname) = oty else { return None };
            if env.object_types.contains(&cname)
                || env.remote_types.contains(&cname)
                || env.domain_types.contains(&cname)
            {
                return None;
            }
            let info = env.classes.get(&cname)?;
            let (_, fty, _) = info.fields.iter().find(|(fname, _, _)| fname == &field.node)?;
            Some((format!("{opath}.{}", field.node), fty.clone()))
        }
        _ => None,
    }
}

/// Resolve `expr` to a trackable `int`-typed path.
fn int_path(expr: &Expr, env: &TypeEnv) -> Option<String> {
    match typed_path(expr, env) {
        Some((path, PlutoType::Int)) => Some(path),
        _ => None,
    }
}

/// Does this expression mention any trackable int path? Used to gate the
/// degenerate-condition warning: constant-only conditions (`if true`,
/// `if 1 < 2`) are common as scaffolding and never warned about.
pub fn mentions_int_path(expr: &Spanned<Expr>, env: &TypeEnv) -> bool {
    struct PathScan<'a> {
        env: &'a TypeEnv,
        found: bool,
    }
    impl Visitor for PathScan<'_> {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if self.found {
                return;
            }
            if int_path(&expr.node, self.env).is_some() {
                self.found = true;
                return;
            }
            walk_expr(self, expr);
        }
    }
    let mut scan = PathScan { env, found: false };
    scan.visit_expr(expr);
    scan.found
}

/// Does this expression contain a call-like node (call, method call, static
/// trait call, `at`, `spawn`) anywhere? Such an expression may mutate
/// reachable objects, so no facts are extracted from it and field facts are
/// killed when it executes.
pub fn contains_call(expr: &Spanned<Expr>) -> bool {
    struct CallScan {
        found: bool,
    }
    impl Visitor for CallScan {
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
    let mut scan = CallScan { found: false };
    scan.visit_expr(expr);
    scan.found
}

// ─────────────────────────────────────────────────────────────────────────────
// Affine forms
// ─────────────────────────────────────────────────────────────────────────────

/// `k + Σ coeff·path`, all arithmetic in i128 (source constants are i64, so
/// checked i128 ops only overflow on absurd inputs — and then we bail to
/// `None`, i.e. Unknown).
#[derive(Debug, Clone, PartialEq)]
struct Affine {
    terms: BTreeMap<String, i128>,
    k: i128,
}

impl Affine {
    fn constant(k: i128) -> Affine {
        Affine { terms: BTreeMap::new(), k }
    }

    fn term(path: String) -> Affine {
        let mut terms = BTreeMap::new();
        terms.insert(path, 1i128);
        Affine { terms, k: 0 }
    }

    fn checked_neg(mut self) -> Option<Affine> {
        for c in self.terms.values_mut() {
            *c = c.checked_neg()?;
        }
        self.k = self.k.checked_neg()?;
        Some(self)
    }

    fn checked_add(mut self, other: &Affine) -> Option<Affine> {
        for (p, c) in &other.terms {
            let entry = self.terms.entry(p.clone()).or_insert(0);
            *entry = entry.checked_add(*c)?;
        }
        self.k = self.k.checked_add(other.k)?;
        self.terms.retain(|_, c| *c != 0);
        Some(self)
    }

    fn checked_mul_const(mut self, m: i128) -> Option<Affine> {
        for c in self.terms.values_mut() {
            *c = c.checked_mul(m)?;
        }
        self.k = self.k.checked_mul(m)?;
        self.terms.retain(|_, c| *c != 0);
        Some(self)
    }

    /// If this is exactly `x - y + k`, return `(x, y)`.
    fn as_diff(&self) -> Option<(&str, &str)> {
        if self.terms.len() != 2 {
            return None;
        }
        let mut pos = None;
        let mut neg = None;
        for (p, c) in &self.terms {
            match c {
                1 => pos = Some(p.as_str()),
                -1 => neg = Some(p.as_str()),
                _ => return None,
            }
        }
        Some((pos?, neg?))
    }
}

/// Normalize an integer-typed expression into affine form. Returns `None`
/// for anything outside the fragment.
fn to_affine(expr: &Expr, env: &TypeEnv) -> Option<Affine> {
    if let Some(path) = int_path(expr, env) {
        return Some(Affine::term(path));
    }
    match expr {
        Expr::IntLit(v) => Some(Affine::constant(*v as i128)),
        Expr::UnaryOp { op: UnaryOp::Neg, operand } => {
            to_affine(&operand.node, env)?.checked_neg()
        }
        Expr::BinOp { op: BinOp::Add, lhs, rhs } => {
            let l = to_affine(&lhs.node, env)?;
            let r = to_affine(&rhs.node, env)?;
            l.checked_add(&r)
        }
        Expr::BinOp { op: BinOp::Sub, lhs, rhs } => {
            let l = to_affine(&lhs.node, env)?;
            let r = to_affine(&rhs.node, env)?.checked_neg()?;
            l.checked_add(&r)
        }
        Expr::BinOp { op: BinOp::Mul, lhs, rhs } => {
            let l = to_affine(&lhs.node, env)?;
            let r = to_affine(&rhs.node, env)?;
            // Constant coefficients only: one side must be term-free.
            if l.terms.is_empty() {
                r.checked_mul_const(l.k)
            } else if r.terms.is_empty() {
                l.checked_mul_const(r.k)
            } else {
                None
            }
        }
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Bounding
// ─────────────────────────────────────────────────────────────────────────────

fn add_opt(a: Option<i128>, b: Option<i128>) -> Option<i128> {
    a?.checked_add(b?)
}

fn mul_opt(v: Option<i64>, c: i128) -> Option<i128> {
    (v? as i128).checked_mul(c)
}

fn max_opt(a: Option<i128>, b: Option<i128>) -> Option<i128> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (x, None) | (None, x) => x,
    }
}

fn min_opt(a: Option<i128>, b: Option<i128>) -> Option<i128> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (x, None) | (None, x) => x,
    }
}

/// Bound an affine form using the current interval facts (and relation facts
/// when the form is exactly `x - y + k`). Returns `(lo, hi)`; `None` means
/// unbounded on that side. Returns `Err(())` when some involved path has a
/// contradictory (empty) interval — the code is unreachable and every query
/// should answer Unknown rather than cascade warnings into dead branches.
fn affine_bounds(a: &Affine, facts: &FactEnv) -> Result<(Option<i128>, Option<i128>), ()> {
    let mut lo = Some(a.k);
    let mut hi = Some(a.k);
    for (path, &c) in &a.terms {
        let iv = facts.interval_of(path);
        if iv.is_empty() {
            return Err(());
        }
        let (term_lo, term_hi) = if c > 0 {
            (mul_opt(iv.lo_bound(), c), mul_opt(iv.hi_bound(), c))
        } else {
            (mul_opt(iv.hi_bound(), c), mul_opt(iv.lo_bound(), c))
        };
        lo = add_opt(lo, term_lo);
        hi = add_opt(hi, term_hi);
    }

    // Relation tightening for the pure difference form x - y + k.
    if let Some((x, y)) = a.as_diff() {
        let mut dlo: Option<i128> = None; // bounds on (x - y)
        let mut dhi: Option<i128> = None;
        if facts.rel_holds(x, RelOp::Lt, y) {
            dhi = min_opt(dhi, Some(-1));
        }
        if facts.rel_holds(x, RelOp::Le, y) {
            dhi = min_opt(dhi, Some(0));
        }
        if facts.rel_holds(y, RelOp::Lt, x) {
            dlo = max_opt(dlo, Some(1));
        }
        if facts.rel_holds(y, RelOp::Le, x) {
            dlo = max_opt(dlo, Some(0));
        }
        if facts.rel_holds(x, RelOp::Eq, y) {
            dlo = max_opt(dlo, Some(0));
            dhi = min_opt(dhi, Some(0));
        }
        lo = max_opt(lo, add_opt(dlo, Some(a.k)));
        hi = min_opt(hi, add_opt(dhi, Some(a.k)));
    }

    Ok((lo, hi))
}

// ─────────────────────────────────────────────────────────────────────────────
// Condition evaluation (the `implies` API)
// ─────────────────────────────────────────────────────────────────────────────

fn is_comparison(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq | BinOp::Eq | BinOp::Neq
    )
}

/// Decide a boolean condition against the current facts. See the module docs
/// for the fragment; anything outside it is `Unknown`.
pub fn eval_condition(cond: &Expr, env: &TypeEnv) -> Verdict {
    match cond {
        Expr::BinOp { op: BinOp::And, lhs, rhs } => {
            let l = eval_condition(&lhs.node, env);
            let r = eval_condition(&rhs.node, env);
            match (l, r) {
                (Verdict::Refuted, _) | (_, Verdict::Refuted) => Verdict::Refuted,
                (Verdict::Proven, Verdict::Proven) => Verdict::Proven,
                _ => Verdict::Unknown,
            }
        }
        Expr::BinOp { op: BinOp::Or, lhs, rhs } => {
            let l = eval_condition(&lhs.node, env);
            let r = eval_condition(&rhs.node, env);
            match (l, r) {
                (Verdict::Proven, _) | (_, Verdict::Proven) => Verdict::Proven,
                (Verdict::Refuted, Verdict::Refuted) => Verdict::Refuted,
                _ => Verdict::Unknown,
            }
        }
        Expr::UnaryOp { op: UnaryOp::Not, operand } => {
            eval_condition(&operand.node, env).negate()
        }
        Expr::BinOp { op, lhs, rhs } if is_comparison(*op) => {
            eval_comparison(*op, &lhs.node, &rhs.node, env)
        }
        _ => Verdict::Unknown,
    }
}

fn eval_comparison(op: BinOp, lhs: &Expr, rhs: &Expr, env: &TypeEnv) -> Verdict {
    let Some(l) = to_affine(lhs, env) else { return Verdict::Unknown };
    let Some(r) = to_affine(rhs, env) else { return Verdict::Unknown };
    let Some(neg_r) = r.checked_neg() else { return Verdict::Unknown };
    let Some(d) = l.checked_add(&neg_r) else { return Verdict::Unknown };

    // A - B cmp 0, with (lo, hi) bounding A - B.
    let Ok((lo, hi)) = affine_bounds(&d, &env.facts) else {
        // Contradictory facts: this code is unreachable; don't cascade.
        return Verdict::Unknown;
    };

    let lo_ge = |v: i128| lo.is_some_and(|l| l >= v);
    let hi_le = |v: i128| hi.is_some_and(|h| h <= v);

    // `x != y` facts only decide equality when the difference is exactly
    // x - y (no constant offset).
    let ne_known = d.k == 0
        && d.as_diff()
            .is_some_and(|(x, y)| env.facts.rel_holds(x, RelOp::Ne, y));

    match op {
        BinOp::Lt => {
            if hi_le(-1) {
                Verdict::Proven
            } else if lo_ge(0) {
                Verdict::Refuted
            } else {
                Verdict::Unknown
            }
        }
        BinOp::LtEq => {
            if hi_le(0) {
                Verdict::Proven
            } else if lo_ge(1) {
                Verdict::Refuted
            } else {
                Verdict::Unknown
            }
        }
        BinOp::Gt => {
            if lo_ge(1) {
                Verdict::Proven
            } else if hi_le(0) {
                Verdict::Refuted
            } else {
                Verdict::Unknown
            }
        }
        BinOp::GtEq => {
            if lo_ge(0) {
                Verdict::Proven
            } else if hi_le(-1) {
                Verdict::Refuted
            } else {
                Verdict::Unknown
            }
        }
        BinOp::Eq => {
            if lo_ge(0) && hi_le(0) {
                Verdict::Proven
            } else if lo_ge(1) || hi_le(-1) || ne_known {
                Verdict::Refuted
            } else {
                Verdict::Unknown
            }
        }
        BinOp::Neq => {
            if lo_ge(1) || hi_le(-1) || ne_known {
                Verdict::Proven
            } else if lo_ge(0) && hi_le(0) {
                Verdict::Refuted
            } else {
                Verdict::Unknown
            }
        }
        // Unreachable: guarded by is_comparison at the call site.
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::And
        | BinOp::Or | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl
        | BinOp::Shr => Verdict::Unknown,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fact extraction from conditions
// ─────────────────────────────────────────────────────────────────────────────

/// Facts a condition establishes on each branch.
#[derive(Debug, Clone, Default)]
pub struct CondFacts {
    /// Hold when the condition is true.
    pub then_facts: Vec<Fact>,
    /// Hold when the condition is false.
    pub else_facts: Vec<Fact>,
}

fn negate_cmp(op: BinOp) -> BinOp {
    match op {
        BinOp::Lt => BinOp::GtEq,
        BinOp::LtEq => BinOp::Gt,
        BinOp::Gt => BinOp::LtEq,
        BinOp::GtEq => BinOp::Lt,
        BinOp::Eq => BinOp::Neq,
        BinOp::Neq => BinOp::Eq,
        other => other,
    }
}

/// Extract branch facts from a condition. Callers must ensure the condition
/// contains no call-like expressions (see [`contains_call`]).
pub fn condition_facts(cond: &Expr, env: &TypeEnv) -> CondFacts {
    match cond {
        Expr::BinOp { op: BinOp::And, lhs, rhs } => {
            // Both conjuncts hold in the then-branch; the negation of a
            // conjunction is a disjunction, which yields no usable facts.
            let l = condition_facts(&lhs.node, env);
            let r = condition_facts(&rhs.node, env);
            CondFacts {
                then_facts: [l.then_facts, r.then_facts].concat(),
                else_facts: Vec::new(),
            }
        }
        Expr::BinOp { op: BinOp::Or, lhs, rhs } => {
            // De Morgan: both negations hold in the else-branch.
            let l = condition_facts(&lhs.node, env);
            let r = condition_facts(&rhs.node, env);
            CondFacts {
                then_facts: Vec::new(),
                else_facts: [l.else_facts, r.else_facts].concat(),
            }
        }
        Expr::UnaryOp { op: UnaryOp::Not, operand } => {
            let inner = condition_facts(&operand.node, env);
            CondFacts {
                then_facts: inner.else_facts,
                else_facts: inner.then_facts,
            }
        }
        Expr::BinOp { op, lhs, rhs } if is_comparison(*op) => {
            let Some(l) = to_affine(&lhs.node, env) else { return CondFacts::default() };
            let Some(r) = to_affine(&rhs.node, env) else { return CondFacts::default() };
            let Some(neg_r) = r.checked_neg() else { return CondFacts::default() };
            let Some(d) = l.checked_add(&neg_r) else { return CondFacts::default() };
            CondFacts {
                then_facts: facts_from_diff(*op, &d),
                else_facts: facts_from_diff(negate_cmp(*op), &d),
            }
        }
        _ => CondFacts::default(),
    }
}

/// Floor division on i128 (rounds toward negative infinity).
fn floor_div(a: i128, b: i128) -> i128 {
    let q = a / b;
    if a % b != 0 && (a < 0) != (b < 0) { q - 1 } else { q }
}

/// Ceiling division on i128 (rounds toward positive infinity).
fn ceil_div(a: i128, b: i128) -> i128 {
    let q = a / b;
    if a % b != 0 && (a < 0) == (b < 0) { q + 1 } else { q }
}

/// Clamp an i128 upper bound on a path into an interval fact.
fn upper_to_interval(u: i128) -> Interval {
    if u >= i64::MAX as i128 {
        Interval::TOP // no information
    } else if u < i64::MIN as i128 {
        Interval::EMPTY // contradiction — branch unreachable
    } else {
        Interval::at_most(u as i64)
    }
}

/// Clamp an i128 lower bound on a path into an interval fact.
fn lower_to_interval(l: i128) -> Interval {
    if l <= i64::MIN as i128 {
        Interval::TOP
    } else if l > i64::MAX as i128 {
        Interval::EMPTY
    } else {
        Interval::at_least(l as i64)
    }
}

/// Derive facts from `d cmp 0` where `d` is an affine difference.
///
/// - Single term `c·x + k`: an interval bound on `x` with exact integer
///   rounding (strict comparisons are first normalized to non-strict).
/// - Pure difference `x - y` (coefficients +1/-1, no constant): a relation
///   fact. Constant offsets between two paths are out of scope for phase 1.
fn facts_from_diff(op: BinOp, d: &Affine) -> Vec<Fact> {
    // Single-variable interval facts.
    if d.terms.len() == 1 {
        let (path, &c) = d.terms.iter().next().expect("len checked");
        debug_assert!(c != 0, "zero coefficients are pruned");
        // c·x + k cmp 0, normalized to c·x ≤ U and/or c·x ≥ L.
        let (le, ge): (Option<i128>, Option<i128>) = match op {
            BinOp::Lt => (d.k.checked_neg().and_then(|v| v.checked_sub(1)), None),
            BinOp::LtEq => (d.k.checked_neg(), None),
            BinOp::Gt => (None, d.k.checked_neg().and_then(|v| v.checked_add(1))),
            BinOp::GtEq => (None, d.k.checked_neg()),
            BinOp::Eq => (d.k.checked_neg(), d.k.checked_neg()),
            BinOp::Neq => (None, None),
            _ => (None, None),
        };
        let mut iv = Interval::TOP;
        if let Some(u) = le {
            // c·x ≤ U  ⇒  x ≤ ⌊U/c⌋ (c>0)  |  x ≥ ⌈U/c⌉ (c<0)
            iv = iv.intersect(&if c > 0 {
                upper_to_interval(floor_div(u, c))
            } else {
                lower_to_interval(ceil_div(u, c))
            });
        }
        if let Some(l) = ge {
            // c·x ≥ L  ⇒  x ≥ ⌈L/c⌉ (c>0)  |  x ≤ ⌊L/c⌋ (c<0)
            iv = iv.intersect(&if c > 0 {
                lower_to_interval(ceil_div(l, c))
            } else {
                upper_to_interval(floor_div(l, c))
            });
        }
        if iv == Interval::TOP {
            return Vec::new();
        }
        return vec![Fact::Bound(path.clone(), iv)];
    }

    // Pure-difference relation facts: x - y cmp 0.
    if d.k == 0 {
        if let Some((x, y)) = d.as_diff() {
            let rel = match op {
                BinOp::Lt => Some((x, RelOp::Lt, y)),
                BinOp::LtEq => Some((x, RelOp::Le, y)),
                BinOp::Gt => Some((y, RelOp::Lt, x)),
                BinOp::GtEq => Some((y, RelOp::Le, x)),
                BinOp::Eq => Some((x, RelOp::Eq, y)),
                BinOp::Neq => Some((x, RelOp::Ne, y)),
                _ => None,
            };
            if let Some((a, op, b)) = rel {
                return vec![Fact::Rel(a.to_string(), op, b.to_string())];
            }
        }
    }

    Vec::new()
}

// ─────────────────────────────────────────────────────────────────────────────
// Statement effects (kills)
// ─────────────────────────────────────────────────────────────────────────────

/// Apply a statement's fact kills before it is checked. See the module docs
/// for the kill rules. Nested blocks are *not* scanned here — their
/// statements apply their own kills as the checker reaches them, which is
/// what makes the analysis flow-sensitive.
pub fn apply_stmt_kills(stmt: &crate::parser::ast::Stmt, env: &mut TypeEnv) {
    use crate::parser::ast::{SelectOp, Stmt};

    let mut exprs: Vec<&Spanned<Expr>> = Vec::new();
    match stmt {
        Stmt::Let { name, value, .. } => {
            // A fresh binding can't shadow (rejected earlier), but kill
            // defensively in case a same-named fact survived a sibling scope.
            env.facts.kill_path(&name.node);
            exprs.push(value);
        }
        Stmt::Assign { target, value } => {
            env.facts.kill_path(&target.node);
            exprs.push(value);
        }
        Stmt::FieldAssign { object, value, .. } => {
            // Aliasing: another local may reference the same object, so a
            // field write invalidates every field fact.
            env.facts.kill_fields();
            exprs.push(object);
            exprs.push(value);
        }
        Stmt::While { condition, .. } => {
            // Conservative loop rule: drop everything at loop entry (and
            // therefore after the loop). See module docs.
            env.facts.havoc_all();
            exprs.push(condition);
        }
        Stmt::For { iterable, .. } => {
            env.facts.havoc_all();
            exprs.push(iterable);
        }
        Stmt::Return(value) => {
            if let Some(v) = value {
                exprs.push(v);
            }
        }
        Stmt::If { condition, .. } => exprs.push(condition),
        Stmt::IndexAssign { object, index, value } => {
            // Collections hold values; writing an element can't change any
            // tracked int path, but the index/value exprs may contain calls.
            exprs.push(object);
            exprs.push(index);
            exprs.push(value);
        }
        Stmt::Match { expr, .. } => exprs.push(expr),
        Stmt::Raise { fields, .. } => {
            for (_, e) in fields {
                exprs.push(e);
            }
        }
        Stmt::LetChan { capacity, .. } => {
            if let Some(c) = capacity {
                exprs.push(c);
            }
        }
        Stmt::Select { arms, .. } => {
            for arm in arms {
                match &arm.op {
                    SelectOp::Recv { channel, .. } => exprs.push(channel),
                    SelectOp::Send { channel, value } => {
                        exprs.push(channel);
                        exprs.push(value);
                    }
                }
            }
        }
        Stmt::Scope { seeds, .. } => {
            for s in seeds {
                exprs.push(s);
            }
        }
        Stmt::Yield { value } => exprs.push(value),
        Stmt::Assert { expr } => exprs.push(expr),
        Stmt::Serve { service, port } => {
            exprs.push(service);
            exprs.push(port);
        }
        Stmt::Break | Stmt::Continue => {}
        Stmt::Expr(e) => exprs.push(e),
    }

    if exprs.iter().any(|e| contains_call(e)) {
        env.facts.kill_fields();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests — interval / relation lattice and the affine machinery
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Interval lattice ─────────────────────────────────────────────────

    #[test]
    fn interval_intersect_narrows() {
        let a = Interval::at_most(10);
        let b = Interval::at_least(3);
        let i = a.intersect(&b);
        assert_eq!(i, Interval { lo: 3, hi: 10 });
        assert!(!i.is_empty());
    }

    #[test]
    fn interval_intersect_disjoint_is_empty() {
        let a = Interval::at_most(2);
        let b = Interval::at_least(5);
        assert!(a.intersect(&b).is_empty());
    }

    #[test]
    fn interval_top_is_identity() {
        let a = Interval { lo: -5, hi: 7 };
        assert_eq!(a.intersect(&Interval::TOP), a);
        assert_eq!(Interval::TOP.intersect(&a), a);
    }

    #[test]
    fn interval_empty_absorbs() {
        let a = Interval { lo: -5, hi: 7 };
        assert!(Interval::EMPTY.intersect(&a).is_empty());
    }

    #[test]
    fn interval_bounds_sentinels() {
        assert_eq!(Interval::TOP.lo_bound(), None);
        assert_eq!(Interval::TOP.hi_bound(), None);
        assert_eq!(Interval::exactly(4).lo_bound(), Some(4));
        assert_eq!(Interval::exactly(4).hi_bound(), Some(4));
    }

    // ── FactEnv frames, kills, lookups ───────────────────────────────────

    #[test]
    fn facts_intersect_across_frames() {
        let mut f = FactEnv::new();
        f.push_frame();
        f.assume(Fact::Bound("x".into(), Interval::at_most(10)));
        f.push_frame();
        f.assume(Fact::Bound("x".into(), Interval::at_least(3)));
        assert_eq!(f.interval_of("x"), Interval { lo: 3, hi: 10 });
        f.pop_frame();
        assert_eq!(f.interval_of("x"), Interval::at_most(10));
    }

    #[test]
    fn assume_intersects_within_frame() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("x".into(), Interval::at_most(10)));
        f.assume(Fact::Bound("x".into(), Interval::at_most(5)));
        assert_eq!(f.interval_of("x"), Interval::at_most(5));
    }

    #[test]
    fn kill_path_reaches_outer_frames() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("x".into(), Interval::at_most(10)));
        f.push_frame();
        f.kill_path("x");
        f.pop_frame();
        // The kill must survive the scope pop (flow event, not scope event).
        assert_eq!(f.interval_of("x"), Interval::TOP);
    }

    #[test]
    fn kill_path_kills_paths_under_root() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("a.b".into(), Interval::at_most(1)));
        f.assume(Fact::Bound("ab".into(), Interval::at_most(2)));
        f.kill_path("a");
        assert_eq!(f.interval_of("a.b"), Interval::TOP);
        // "ab" is not under root "a".
        assert_eq!(f.interval_of("ab"), Interval::at_most(2));
    }

    #[test]
    fn kill_fields_spares_locals() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("x".into(), Interval::at_most(1)));
        f.assume(Fact::Bound("self.balance".into(), Interval::at_least(0)));
        f.assume(Fact::Rel("x".into(), RelOp::Le, "self.balance".into()));
        f.kill_fields();
        assert_eq!(f.interval_of("x"), Interval::at_most(1));
        assert_eq!(f.interval_of("self.balance"), Interval::TOP);
        assert!(!f.rel_holds("x", RelOp::Le, "self.balance"));
    }

    #[test]
    fn havoc_drops_everything() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("x".into(), Interval::at_most(1)));
        f.assume(Fact::Rel("a".into(), RelOp::Lt, "b".into()));
        f.havoc_all();
        assert_eq!(f.interval_of("x"), Interval::TOP);
        assert!(!f.rel_holds("a", RelOp::Lt, "b"));
    }

    #[test]
    fn relations_directed_and_symmetric() {
        let mut f = FactEnv::new();
        f.assume(Fact::Rel("a".into(), RelOp::Lt, "b".into()));
        f.assume(Fact::Rel("c".into(), RelOp::Eq, "d".into()));
        assert!(f.rel_holds("a", RelOp::Lt, "b"));
        assert!(!f.rel_holds("b", RelOp::Lt, "a")); // Lt is directed
        assert!(f.rel_holds("c", RelOp::Eq, "d"));
        assert!(f.rel_holds("d", RelOp::Eq, "c")); // Eq is symmetric
    }

    #[test]
    fn kill_mark_reports_kills_since() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("x".into(), Interval::at_most(1)));
        let mark = f.kill_mark();
        let fact = Fact::Bound("x".into(), Interval::at_most(1));
        assert!(!f.killed_since(mark, &fact));
        f.kill_path("x");
        assert!(f.killed_since(mark, &fact));
        // Field-kill events only invalidate dotted paths.
        let mark2 = f.kill_mark();
        f.kill_fields();
        assert!(!f.killed_since(mark2, &fact));
        let field_fact = Fact::Bound("p.x".into(), Interval::at_most(1));
        assert!(f.killed_since(mark2, &field_fact));
    }

    #[test]
    fn pop_frame_never_removes_base() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("x".into(), Interval::at_most(1)));
        f.pop_frame();
        f.pop_frame();
        // Base frame survives (cleared), and the engine still answers.
        assert_eq!(f.interval_of("x"), Interval::TOP);
        f.assume(Fact::Bound("y".into(), Interval::at_least(0)));
        assert_eq!(f.interval_of("y"), Interval::at_least(0));
    }

    // ── Division helpers ─────────────────────────────────────────────────

    #[test]
    fn floor_and_ceil_division() {
        assert_eq!(floor_div(7, 2), 3);
        assert_eq!(floor_div(-7, 2), -4);
        assert_eq!(floor_div(7, -2), -4);
        assert_eq!(floor_div(-7, -2), 3);
        assert_eq!(ceil_div(7, 2), 4);
        assert_eq!(ceil_div(-7, 2), -3);
        assert_eq!(ceil_div(7, -2), -3);
        assert_eq!(ceil_div(-7, -2), 4);
        assert_eq!(floor_div(6, 3), 2);
        assert_eq!(ceil_div(6, 3), 2);
    }

    // ── facts_from_diff (extraction) ─────────────────────────────────────

    fn single(path: &str, c: i128, k: i128) -> Affine {
        let mut terms = BTreeMap::new();
        terms.insert(path.to_string(), c);
        Affine { terms, k }
    }

    #[test]
    fn extract_upper_bound_from_lt() {
        // x - 5 < 0  ⇒  x ≤ 4
        let facts = facts_from_diff(BinOp::Lt, &single("x", 1, -5));
        assert_eq!(facts, vec![Fact::Bound("x".into(), Interval::at_most(4))]);
    }

    #[test]
    fn extract_lower_bound_from_negated_coeff() {
        // -x + 5 < 0  ⇒  x > 5  ⇒  x ≥ 6
        let facts = facts_from_diff(BinOp::Lt, &single("x", -1, 5));
        assert_eq!(facts, vec![Fact::Bound("x".into(), Interval::at_least(6))]);
    }

    #[test]
    fn extract_rounding_with_coefficient() {
        // 2x - 7 ≤ 0  ⇒  x ≤ ⌊7/2⌋ = 3
        let facts = facts_from_diff(BinOp::LtEq, &single("x", 2, -7));
        assert_eq!(facts, vec![Fact::Bound("x".into(), Interval::at_most(3))]);
        // 2x - 7 ≥ 0  ⇒  x ≥ ⌈7/2⌉ = 4
        let facts = facts_from_diff(BinOp::GtEq, &single("x", 2, -7));
        assert_eq!(facts, vec![Fact::Bound("x".into(), Interval::at_least(4))]);
    }

    #[test]
    fn extract_point_from_eq() {
        // x - 7 == 0  ⇒  x ∈ [7,7]
        let facts = facts_from_diff(BinOp::Eq, &single("x", 1, -7));
        assert_eq!(facts, vec![Fact::Bound("x".into(), Interval::exactly(7))]);
    }

    #[test]
    fn extract_unsatisfiable_eq_is_empty() {
        // 2x - 7 == 0 has no integer solution ⇒ empty interval (dead branch)
        let facts = facts_from_diff(BinOp::Eq, &single("x", 2, -7));
        assert_eq!(facts.len(), 1);
        let Fact::Bound(_, iv) = &facts[0] else { panic!("expected bound") };
        assert!(iv.is_empty());
    }

    #[test]
    fn extract_neq_yields_no_interval() {
        let facts = facts_from_diff(BinOp::Neq, &single("x", 1, -5));
        assert!(facts.is_empty());
    }

    #[test]
    fn extract_relation_from_pure_difference() {
        // x - y ≤ 0 ⇒ Rel(x, Le, y);  x - y > 0 ⇒ Rel(y, Lt, x)
        let mut terms = BTreeMap::new();
        terms.insert("x".to_string(), 1i128);
        terms.insert("y".to_string(), -1i128);
        let d = Affine { terms, k: 0 };
        assert_eq!(
            facts_from_diff(BinOp::LtEq, &d),
            vec![Fact::Rel("x".into(), RelOp::Le, "y".into())]
        );
        assert_eq!(
            facts_from_diff(BinOp::Gt, &d),
            vec![Fact::Rel("y".into(), RelOp::Lt, "x".into())]
        );
    }

    #[test]
    fn extract_offset_difference_yields_nothing() {
        // x - y + 3 cmp 0: constant offsets between paths are out of scope.
        let mut terms = BTreeMap::new();
        terms.insert("x".to_string(), 1i128);
        terms.insert("y".to_string(), -1i128);
        let d = Affine { terms, k: 3 };
        assert!(facts_from_diff(BinOp::LtEq, &d).is_empty());
    }

    // ── affine_bounds ────────────────────────────────────────────────────

    #[test]
    fn bounds_combine_intervals_linearly() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("x".into(), Interval { lo: 1, hi: 4 }));
        // d = 2x - 3: bounds [2·1-3, 2·4-3] = [-1, 5]
        let d = single("x", 2, -3);
        assert_eq!(affine_bounds(&d, &f), Ok((Some(-1), Some(5))));
    }

    #[test]
    fn bounds_unbounded_side_stays_none() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("x".into(), Interval::at_most(4)));
        let d = single("x", 1, 0);
        assert_eq!(affine_bounds(&d, &f), Ok((None, Some(4))));
        // Negative coefficient flips which side is bounded.
        let d = single("x", -1, 0);
        assert_eq!(affine_bounds(&d, &f), Ok((Some(-4), None)));
    }

    #[test]
    fn bounds_use_relations_for_pure_difference() {
        let mut f = FactEnv::new();
        f.assume(Fact::Rel("a".into(), RelOp::Le, "b".into()));
        let mut terms = BTreeMap::new();
        terms.insert("a".to_string(), 1i128);
        terms.insert("b".to_string(), -1i128);
        let d = Affine { terms, k: 0 };
        // a - b ≤ 0 from the relation; lower side unbounded.
        assert_eq!(affine_bounds(&d, &f), Ok((None, Some(0))));
    }

    #[test]
    fn bounds_empty_interval_is_error() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("x".into(), Interval::EMPTY));
        let d = single("x", 1, 0);
        assert_eq!(affine_bounds(&d, &f), Err(()));
    }

    #[test]
    fn bounds_overflow_widens_to_unbounded() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("x".into(), Interval::exactly(i64::MAX - 1)));
        // Huge coefficient: i128 checked mul still fine; force overflow via
        // repeated max constants instead — k near i128::MAX.
        let d = Affine {
            terms: {
                let mut t = BTreeMap::new();
                t.insert("x".to_string(), i64::MAX as i128);
                t
            },
            k: i128::MAX - 1,
        };
        // (i64::MAX-1)·i64::MAX + (i128::MAX-1) overflows ⇒ unbounded, not wrong.
        let (lo, hi) = affine_bounds(&d, &f).unwrap();
        assert_eq!(lo, None);
        assert_eq!(hi, None);
    }
}
