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
//! Facts are tracked about *paths*: local `int` variables (`x`), simple
//! field paths rooted at a local (`self.balance`, `p.x.y`) whose every step
//! is a plain class field and whose final type is `int`, and *length terms*
//! (`xs.len()`) over a trackable path of collection type (array, string,
//! bytes, map, set). Entities (`object` types), remote types, and domain
//! deps are excluded — their fields can change concurrently, so a flow fact
//! about them is never sound.
//!
//! A length term is opaque (nothing relates `xs.len()` to the elements of
//! `xs`) but carries one automatic fact: `xs.len() >= 0`, baked into
//! [`FactEnv::interval_of`] so every bound query sees it. The builtin `len`
//! call on a collection-typed path runs no user code and mutates nothing,
//! so it is exempt from the call rules below ([`contains_impure_call`]);
//! `len` on anything else (class receivers, untrackable objects) stays
//! call-like.
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
//! - `assert <cond>`: the failure path aborts the process, so the
//!   condition's then-facts hold for the rest of the enclosing block —
//!   exactly the terminating-guard rule, with the same extraction path
//!   (`condition_facts`) and the same impure-call exclusion.
//!
//! No facts are extracted from a condition that contains an impure
//! call-like expression (the call could mutate state between evaluation
//! and use); builtin `len` on a collection-typed path is pure and exempt.
//!
//! Separately from flow facts, [`expr_bounds`] answers point-wise interval
//! queries for int-valued expressions *outside* the affine fragment: masks
//! (`x & c`), remainders (`x % c`), and byte-typed values widened to int
//! (`[0, 255]` by construction — a type fact that survives loops and
//! kills). Consumed by the overflow/shift-check elision pass
//! (arith_fit.rs).
//!
//! # What kills facts
//!
//! - Reassignment of a variable kills its facts and facts about paths
//!   rooted at it (`x` kills `x`, `x.f`, and `x.len()`).
//! - Any field assignment kills *all* field-path facts (aliasing: two locals
//!   can point at the same object, so a write through one invalidates facts
//!   about the other).
//! - Any statement whose immediate expressions contain a call-like node
//!   (call, method call, static trait call, `at`, `spawn`) kills facts
//!   according to the call's *severity* ([`call_severity`], the one shared
//!   exemption predicate — invariant discharge and `guarded_by` dominance
//!   consume the same classification rather than keeping their own copies):
//!
//!   - [`CallSeverity::Pure`] — builtin collection `len()` on a trackable
//!     path, and builtin *free functions* (`print`, `abs`, ...) whose
//!     argument values provably cannot reach any class instance. These run
//!     no user code and mutate nothing; they kill nothing.
//!   - [`CallSeverity::Collections`] — calls that may mutate collection
//!     contents/lengths but provably cannot write any class's int fields:
//!     builtin *methods* (receiver of primitive/collection type — see the
//!     load-bearing survey on [`call_severity`]), and direct calls to known
//!     functions/methods none of whose declared parameter types can reach a
//!     class value ([`type_reaches_class`]). These kill all *length terms*
//!     (a builtin `push`/`pop`/`clear` on an alias changes `xs.len()`) but
//!     leave field-path facts alone.
//!   - [`CallSeverity::All`] — everything else (the callee may reach and
//!     mutate any object an argument or receiver can reach, aliasing
//!     coarse by type): kills all field-path facts *and* all length terms,
//!     exactly the old conservative rule.
//!
//!   Local int variables survive every call: parameters are passed by
//!   value and `mut` params are local copies, so no call can change a
//!   caller's local.
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
//! Consumers: degenerate-condition warnings ("condition is always
//! true/false") on `if` statements (phase 1), and static invariant
//! discharge (phase 2, `discharge.rs`), which evaluates conditions through
//! the resolver-parameterized `*_with` variants — the same fragment and the
//! same conservatism, but with leaf paths resolved into a caller-chosen
//! affine vocabulary (ghost variables over a method body, field
//! substitutions at construction sites).
//!
//! # Integer semantics: defects trap, so the mathematical model is sound
//!
//! This engine models `int` as mathematical integers: `x + 1 > x` is Proven,
//! never "unless it wraps". That is justified because runtime arithmetic
//! does not wrap: signed i64 overflow on `+`, `-`, `*`, unary negation, and
//! `i64::MIN / -1` — like division/modulo by zero — is a *defect* that
//! aborts the process (issue #416; conditions raise, defects trap). Every
//! normally-completing execution therefore agrees with the mathematical
//! model, and every fact proven here holds at every point an execution
//! actually reaches — partial correctness, in the same sense that a fact
//! after an `if` guard holds only on paths that pass the guard. The
//! explicit escape hatch, the `wrapping_add`/`wrapping_sub`/`wrapping_mul`
//! builtins, stays sound automatically: like any other call, a wrapping_*
//! result is outside the affine fragment (`to_affine_with` matches calls to
//! `None`), so no interval fact is ever derived from one.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::parser::ast::{BinOp, Expr, TypeExpr, UnaryOp};
use crate::span::Spanned;
use crate::visit::{walk_expr, Visitor};

use super::env::{mangle_method, TypeEnv};
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
    /// The path's value is not a specific constant (`x != 0`). Intervals
    /// cannot express holes, so inequations get their own fact shape.
    NeConst(String, i64),
    /// Set-membership fact (idempotency proof, rfc-properties.md phase
    /// 5.5): the key path's value was NOT in the set path's set when the
    /// dominating membership check ran. `(set_path, key_path)` — e.g.
    /// `("self.seen", "k")`. Killed like any other field fact: the set
    /// path is dotted, so any call that may run user code kills it (a
    /// callee could insert the key), as does any insert into the set
    /// ([`FactEnv::apply_set_insert`]).
    SetNotContains(String, String),
    /// `set.insert(key)` executed on this control path while
    /// `SetNotContains(set, key)` held — the ARMED insert that licenses
    /// effects after it (the dedup-guard shape). Carrying this fact across
    /// calls is sound ONLY because the idempotency pass separately proves
    /// the set field is globally insert-only (monotone): no reachable code
    /// can remove the key, so "this call inserted key" stays true. The
    /// `kill_fields` exemption below encodes exactly that; the fact still
    /// dies when the KEY path is invalidated (its value identity is what
    /// the claim keys on) and at loop boundaries (havoc).
    SetInserted(String, String),
}

impl Fact {
    /// Every path this fact talks about.
    fn paths(&self) -> impl Iterator<Item = &str> {
        match self {
            Fact::Bound(p, _) | Fact::NeConst(p, _) => std::iter::once(p.as_str()).chain(None),
            Fact::Rel(a, _, b)
            | Fact::SetNotContains(a, b)
            | Fact::SetInserted(a, b) => std::iter::once(a.as_str()).chain(Some(b.as_str())),
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
    ne_consts: Vec<(String, i64)>,
    /// Set-membership facts (`SetNotContains` / `SetInserted` only).
    memberships: Vec<Fact>,
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
    /// All length terms were killed (a call that may mutate collection
    /// contents but cannot write class int fields).
    LenTerms,
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

/// Is this path a length term (`xs.len()`, `p.items.len()`, or an
/// epoch-stamped ghost form like `s@0.len()@2`)? Length terms carry an
/// automatic `>= 0` lower bound. The `.len()` marker cannot collide with a
/// field path: identifiers never contain `(`.
pub(crate) fn is_len_term(path: &str) -> bool {
    path.contains(".len()")
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
            Fact::NeConst(p, v) => {
                if !frame.ne_consts.contains(&(p.clone(), v)) {
                    frame.ne_consts.push((p, v));
                }
            }
            Fact::SetNotContains(..) | Fact::SetInserted(..) => {
                if !frame.memberships.contains(&fact) {
                    frame.memberships.push(fact);
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
            frame.ne_consts.retain(|(p, _)| !path_under(p, root));
            frame
                .memberships
                .retain(|f| !f.paths().any(|p| path_under(p, root)));
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
            frame.ne_consts.retain(|(p, _)| !p.contains('.'));
            // SetNotContains dies with the other field facts (a callee may
            // insert the key). SetInserted survives on the set side — the
            // set is globally insert-only, so no callee can unsay the
            // insert — but dies when the KEY path is a field (the callee
            // may change what the path denotes).
            frame.memberships.retain(|f| match f {
                Fact::SetNotContains(s, k) => !s.contains('.') && !k.contains('.'),
                Fact::SetInserted(_, k) => !k.contains('.'),
                _ => true,
            });
        }
        self.kill_log.push(KillEvent::Fields);
    }

    /// Kill every fact involving a length term, in every frame. Applied for
    /// [`CallSeverity::Collections`] calls: the callee may grow or shrink a
    /// collection through an alias (builtin `push` on a shared array), but
    /// provably cannot write any class's int fields, so plain field-path
    /// facts survive.
    pub fn kill_len_terms(&mut self) {
        for frame in &mut self.frames {
            frame.intervals.retain(|k, _| !is_len_term(k));
            frame
                .relations
                .retain(|(a, _, b)| !is_len_term(a) && !is_len_term(b));
            frame.ne_consts.retain(|(p, _)| !is_len_term(p));
            // A collections-severity call may insert into any reachable
            // set through an alias: not-contains facts die. Armed inserts
            // survive (the dedup set is globally insert-only).
            frame
                .memberships
                .retain(|f| !matches!(f, Fact::SetNotContains(..)));
        }
        self.kill_log.push(KillEvent::LenTerms);
    }

    /// Drop every fact (loop entry — see module docs).
    pub fn havoc_all(&mut self) {
        for frame in &mut self.frames {
            frame.intervals.clear();
            frame.relations.clear();
            frame.ne_consts.clear();
            frame.memberships.clear();
        }
        self.kill_log.push(KillEvent::All);
    }

    /// Is `SetNotContains(set, key)` recorded?
    pub fn set_not_contains_holds(&self, set: &str, key: &str) -> bool {
        self.frames.iter().any(|frame| {
            frame
                .memberships
                .iter()
                .any(|f| matches!(f, Fact::SetNotContains(s, k) if s == set && k == key))
        })
    }

    /// Is an ARMED insert of `key` into any set recorded on this path?
    /// (The idempotency pass's effect-site query: which set field holds the
    /// key does not matter — any armed dedup insert of the claim's key
    /// licenses the effect.)
    pub fn set_inserted_for_key(&self, key: &str) -> bool {
        self.frames.iter().any(|frame| {
            frame
                .memberships
                .iter()
                .any(|f| matches!(f, Fact::SetInserted(_, k) if k == key))
        })
    }

    /// Apply the membership effect of `set.insert(arg)`: every
    /// not-contains fact about `set` dies (the inserted value may equal
    /// any tracked key), and facts under the set path (its length term)
    /// die with it. When the inserted argument IS a tracked key path with
    /// a live not-contains fact, the insert is ARMED: `SetInserted(set,
    /// key)` is assumed. Returns whether the insert armed.
    pub fn apply_set_insert(&mut self, set: &str, key_arg: Option<&str>) -> bool {
        let armed = key_arg.is_some_and(|k| self.set_not_contains_holds(set, k));
        for frame in &mut self.frames {
            frame.intervals.retain(|p, _| !path_under(p, set));
            frame
                .relations
                .retain(|(a, _, b)| !path_under(a, set) && !path_under(b, set));
            frame.ne_consts.retain(|(p, _)| !path_under(p, set));
            frame
                .memberships
                .retain(|f| !matches!(f, Fact::SetNotContains(s, _) if s == set));
        }
        self.kill_log.push(KillEvent::Path(set.to_string()));
        if armed {
            let key = key_arg.expect("armed implies key_arg").to_string();
            self.assume(Fact::SetInserted(set.to_string(), key));
        }
        armed
    }

    /// Current position in the kill log. Take a mark before checking an
    /// `if`'s branches; [`Self::killed_since`] then reports whether a fact's
    /// paths were invalidated while the branches ran.
    /// Does any live relation fact give `path` a STRICT upper neighbor
    /// (`path < q`)? Since q is itself an i64-valued path, this bounds
    /// `path <= i64::MAX - 1` even when q has no interval — used by the
    /// overflow-check elision pass (arith_fit.rs) to prove `path + 1` in
    /// range under a `while path < q` guard.
    pub(crate) fn has_strict_upper(&self, path: &str) -> bool {
        self.frames.iter().any(|f| {
            f.relations
                .iter()
                .any(|(a, op, _)| a == path && *op == RelOp::Lt)
        })
    }

    /// Dual of [`FactEnv::has_strict_upper`]: some live `q < path` bounds
    /// `path >= i64::MIN + 1`.
    pub(crate) fn has_strict_lower(&self, path: &str) -> bool {
        self.frames.iter().any(|f| {
            f.relations
                .iter()
                .any(|(_, op, b)| b == path && *op == RelOp::Lt)
        })
    }

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
                    KillEvent::LenTerms => is_len_term(p),
                    KillEvent::All => true,
                })
            })
    }

    /// The tightest interval known for `path` (TOP when nothing is known,
    /// possibly empty when facts contradict — i.e. the code is unreachable).
    /// Length terms start from their automatic `>= 0` bound instead of TOP.
    pub fn interval_of(&self, path: &str) -> Interval {
        let mut iv = if is_len_term(path) {
            Interval::at_least(0)
        } else {
            Interval::TOP
        };
        for frame in &self.frames {
            if let Some(fiv) = frame.intervals.get(path) {
                iv = iv.intersect(fiv);
            }
        }
        iv
    }

    /// Is `path != v` recorded?
    fn ne_const_holds(&self, path: &str, v: i64) -> bool {
        self.frames
            .iter()
            .any(|frame| frame.ne_consts.iter().any(|(p, c)| p == path && *c == v))
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
pub(crate) fn typed_path(expr: &Expr, env: &TypeEnv) -> Option<(String, PlutoType)> {
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

/// Is this a collection type with a pure builtin `len()`?
fn is_collection(ty: &PlutoType) -> bool {
    matches!(
        ty,
        PlutoType::Array(_)
            | PlutoType::String
            | PlutoType::Bytes
            | PlutoType::Map(_, _)
            | PlutoType::Set(_)
    )
}

/// Resolve `expr` to a length term: `xs.len()` where `xs` is a trackable
/// path of collection type (array, string, bytes, map, set). The builtin
/// `len` runs no user code, so the term is a pure read; class receivers
/// (which may define their own `len` method) never match.
pub(crate) fn len_path(expr: &Expr, env: &TypeEnv) -> Option<String> {
    let Expr::MethodCall { object, method, args, .. } = expr else {
        return None;
    };
    if method.node != "len" || !args.is_empty() {
        return None;
    }
    let (opath, oty) = typed_path(&object.node, env)?;
    is_collection(&oty).then(|| format!("{opath}.len()"))
}

/// Resolve `expr` to any trackable fact term: an int path or a length term.
/// The shared leaf vocabulary of the default resolvers below.
fn fact_term(expr: &Expr, env: &TypeEnv) -> Option<String> {
    int_path(expr, env).or_else(|| len_path(expr, env))
}

/// Does this expression mention any trackable fact term (int path or
/// length term)? Used to gate the degenerate-condition warning:
/// constant-only conditions (`if true`, `if 1 < 2`) are common as
/// scaffolding and never warned about.
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
            if fact_term(&expr.node, self.env).is_some() {
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

/// Like [`contains_call`], but exempts builtin `len()` on a trackable
/// collection-typed path: it runs no user code and mutates nothing, so it
/// neither invalidates facts nor makes a condition effectful. Everything
/// else call-like (including `len` on class receivers or untrackable
/// objects) still counts.
pub fn contains_impure_call(expr: &Spanned<Expr>, env: &TypeEnv) -> bool {
    struct CallScan<'a> {
        env: &'a TypeEnv,
        found: bool,
    }
    impl Visitor for CallScan<'_> {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if self.found {
                return;
            }
            match &expr.node {
                Expr::MethodCall { .. } if len_path(&expr.node, self.env).is_some() => {
                    // Pure length read; its object is a trackable path and
                    // cannot itself contain calls.
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
    let mut scan = CallScan { env, found: false };
    scan.visit_expr(expr);
    scan.found
}

// ─────────────────────────────────────────────────────────────────────────────
// Call severity — the one shared purity-exemption predicate
// ─────────────────────────────────────────────────────────────────────────────

/// How badly the call-like nodes of an expression can invalidate facts.
/// This is THE exemption predicate for call-severing decisions: the
/// statement kill rules here, invariant/ensures discharge's ghost-scope
/// severing (`discharge::pre_stmt`), and `guarded_by` dominance's call
/// safety all consume this classification instead of keeping their own
/// copies of the exemption logic.
///
/// Ordered: `Pure < Collections < All` — a statement's severity is the max
/// over its call-like nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CallSeverity {
    /// No call at all, or only calls that run no user code and mutate
    /// nothing: builtin collection `len()` on a trackable path, and builtin
    /// free functions whose argument values provably cannot reach a class
    /// instance.
    Pure,
    /// Calls that may mutate collection contents (and therefore lengths)
    /// but provably cannot write any int field of the target class (of
    /// *any* class when the target is `None`).
    ///
    /// LOAD-BEARING SURVEY (builtin methods cannot write class int fields):
    /// every builtin method dispatches on a receiver of primitive or
    /// collection type (`int`/`float`/`bool`/`byte`/`string`/`bytes`/
    /// arrays/maps/sets/ranges — see `infer.rs`'s builtin method tables).
    /// No builtin method takes a *class* receiver, runs user code, or
    /// follows references into class instances: mutating builtins
    /// (`push`/`pop`/`insert`/`remove`/`clear`/...) move element
    /// *references* and change lengths, never the fields of the objects
    /// those references point at. A class's int fields are therefore
    /// unreachable from any builtin — which is exactly why this level
    /// kills length terms but not field-path facts. If a builtin that
    /// writes through to class fields is ever added, this classification
    /// must be revisited.
    Collections,
    /// The callee may reach the target (receiver passed, an argument whose
    /// type can transitively reach the target's class — alias-coarse by
    /// type — or an opaque callee: closures, trait dispatch, `at`, `spawn`,
    /// static trait calls, unknown functions). Everything may be mutated.
    All,
}

/// Can a value of type `ty` transitively reach (alias) an instance of class
/// `target` — or of *any* class when `target` is `None`? Reachability
/// follows the heap: class/entity fields, enum payloads, container
/// elements. Opaque types (traits, fn values — whose captures may hold
/// anything — type params, generic instances, error values) conservatively
/// reach everything.
pub(crate) fn type_reaches_class(ty: &PlutoType, target: Option<&str>, env: &TypeEnv) -> bool {
    fn go(
        ty: &PlutoType,
        target: Option<&str>,
        env: &TypeEnv,
        visiting: &mut HashSet<String>,
    ) -> bool {
        match ty {
            PlutoType::Int
            | PlutoType::Float
            | PlutoType::Bool
            | PlutoType::Byte
            | PlutoType::Bytes
            | PlutoType::String
            | PlutoType::Void
            | PlutoType::Range => false,
            PlutoType::Array(e)
            | PlutoType::Set(e)
            | PlutoType::Nullable(e)
            | PlutoType::Task(e)
            | PlutoType::Sender(e)
            | PlutoType::Receiver(e)
            | PlutoType::Stream(e) => go(e, target, env, visiting),
            PlutoType::Map(k, v) => {
                go(k, target, env, visiting) || go(v, target, env, visiting)
            }
            PlutoType::Class(c) => {
                if target.is_none() || target == Some(c.as_str()) {
                    return true;
                }
                if !visiting.insert(format!("c:{c}")) {
                    return false; // already on the walk — cycle
                }
                match env.classes.get(c) {
                    Some(ci) => ci
                        .fields
                        .iter()
                        .any(|(_, ft, _)| go(ft, target, env, visiting)),
                    None => true, // unknown class — conservative
                }
            }
            PlutoType::Enum(name) => {
                if !visiting.insert(format!("e:{name}")) {
                    return false;
                }
                match env.enums.get(name) {
                    Some(ei) => ei.variants.iter().any(|(_, fields)| {
                        fields.iter().any(|(_, ft)| go(ft, target, env, visiting))
                    }),
                    None => true,
                }
            }
            PlutoType::Trait(_)
            | PlutoType::TypeParam(_)
            | PlutoType::Fn(..)
            | PlutoType::Error
            | PlutoType::GenericInstance(..) => true,
        }
    }
    go(ty, target, env, &mut HashSet::new())
}

/// Best-effort *syntactic* typing of a value expression, for classifying
/// builtin free-function arguments before inference has run on the
/// statement. `leaf_ty` resolves trackable paths (callers plug in
/// `typed_path` or dominance's local vocabulary). Anything unresolvable is
/// `None` (conservative).
fn syntactic_type(
    e: &Expr,
    env: &TypeEnv,
    leaf_ty: &dyn Fn(&Expr) -> Option<PlutoType>,
) -> Option<PlutoType> {
    if let Some(t) = leaf_ty(e) {
        return Some(t);
    }
    match e {
        Expr::IntLit(_) => Some(PlutoType::Int),
        Expr::FloatLit(_) => Some(PlutoType::Float),
        Expr::BoolLit(_) => Some(PlutoType::Bool),
        Expr::StringLit(_) | Expr::StringInterp { .. } => Some(PlutoType::String),
        Expr::UnaryOp { op: UnaryOp::Neg, operand } => {
            syntactic_type(&operand.node, env, leaf_ty)
        }
        Expr::UnaryOp { op: UnaryOp::Not, .. } => Some(PlutoType::Bool),
        Expr::CompareChain { .. } => Some(PlutoType::Bool),
        Expr::BinOp { op, lhs, rhs } => {
            if is_comparison(*op) || matches!(op, BinOp::And | BinOp::Or) {
                return Some(PlutoType::Bool);
            }
            // Arithmetic / concat / bitwise: both sides must agree on a
            // primitive type.
            let l = syntactic_type(&lhs.node, env, leaf_ty)?;
            let r = syntactic_type(&rhs.node, env, leaf_ty)?;
            (l == r
                && matches!(
                    l,
                    PlutoType::Int | PlutoType::Float | PlutoType::String | PlutoType::Byte
                ))
            .then_some(l)
        }
        Expr::MethodCall { object, method, args, .. }
            if method.node == "len" && args.is_empty() =>
        {
            syntactic_type(&object.node, env, leaf_ty)
                .filter(is_collection)
                .map(|_| PlutoType::Int)
        }
        Expr::Call { name, .. } => {
            // A direct call to a known function has its declared return
            // type — unless a local variable shadows the name (closure).
            if leaf_ty(&Expr::Ident(name.node.clone())).is_some() {
                return None;
            }
            env.functions.get(&name.node).map(|sig| sig.return_type.clone())
        }
        _ => None,
    }
}

/// Classify a direct free-function call (shared with `guarded_by`
/// dominance, which passes its own path vocabulary as `leaf_ty`):
///
/// - a call through a local fn-typed value (closure) is [`CallSeverity::All`]
///   — its captures may alias anything;
/// - a builtin free function (`print`, math, ...) runs no user code and
///   mutates nothing: [`CallSeverity::Pure`] when every argument's value
///   provably cannot reach the target class, [`CallSeverity::All`]
///   otherwise (the task spec's alias-coarse discipline: handing `print` a
///   possible alias of the receiver still severs);
/// - a known user function is classified by its *declared* parameter types
///   (stable without inference): no param type can reach the target ⇒
///   [`CallSeverity::Collections`] (it may still mutate collections
///   reachable from its args), else [`CallSeverity::All`];
/// - anything unknown is [`CallSeverity::All`].
pub(crate) fn free_call_severity(
    name: &str,
    args: &[Spanned<Expr>],
    env: &TypeEnv,
    target: Option<&str>,
    leaf_ty: &dyn Fn(&Expr) -> Option<PlutoType>,
) -> CallSeverity {
    if leaf_ty(&Expr::Ident(name.to_string())).is_some() {
        return CallSeverity::All;
    }
    if env.builtins.contains(name) {
        let safe = args.iter().all(|a| {
            syntactic_type(&a.node, env, leaf_ty)
                .is_some_and(|t| !type_reaches_class(&t, target, env))
        });
        return if safe { CallSeverity::Pure } else { CallSeverity::All };
    }
    match env.functions.get(name) {
        Some(sig) => {
            if sig
                .params
                .iter()
                .any(|t| type_reaches_class(t, target, env))
            {
                CallSeverity::All
            } else {
                CallSeverity::Collections
            }
        }
        None => CallSeverity::All,
    }
}

/// Classify a direct method call by its receiver's (path-resolvable) type.
fn method_call_severity(
    object: &Spanned<Expr>,
    method: &Spanned<String>,
    env: &TypeEnv,
    target: Option<&str>,
) -> CallSeverity {
    let Some((_, rty)) = typed_path(&object.node, env) else {
        return CallSeverity::All; // untrackable receiver (chained calls, ...)
    };
    match rty {
        // Builtin method carriers — see the load-bearing survey on
        // [`CallSeverity::Collections`].
        PlutoType::Array(_)
        | PlutoType::String
        | PlutoType::Bytes
        | PlutoType::Map(_, _)
        | PlutoType::Set(_)
        | PlutoType::Range
        | PlutoType::Int
        | PlutoType::Float
        | PlutoType::Bool
        | PlutoType::Byte => CallSeverity::Collections,
        // Task bookkeeping that runs no user code in this thread.
        PlutoType::Task(_) if method.node == "detach" || method.node == "cancel" => {
            CallSeverity::Collections
        }
        PlutoType::Class(c) => {
            match env.functions.get(&mangle_method(&c, &method.node)) {
                // The receiver rides in params[0] as Class(c), so "can any
                // declared parameter reach the target" covers the receiver
                // itself (and sibling self-calls classify as All).
                Some(sig) => {
                    if sig
                        .params
                        .iter()
                        .any(|t| type_reaches_class(t, target, env))
                    {
                        CallSeverity::All
                    } else {
                        CallSeverity::Collections
                    }
                }
                None => CallSeverity::All,
            }
        }
        _ => CallSeverity::All,
    }
}

/// The severity of an expression: the max over every call-like node in it
/// (closure bodies included — same conservatism as [`contains_impure_call`]).
/// `target` scopes the question: `Some(class)` asks "can these calls write
/// an int field of *this* class's instances" (invariant/ensures discharge);
/// `None` asks about any class (the statement kill rules, dominance).
pub(crate) fn call_severity(
    expr: &Spanned<Expr>,
    env: &TypeEnv,
    target: Option<&str>,
) -> CallSeverity {
    struct Scan<'a> {
        env: &'a TypeEnv,
        target: Option<&'a str>,
        sev: CallSeverity,
    }
    impl Visitor for Scan<'_> {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if self.sev == CallSeverity::All {
                return;
            }
            match &expr.node {
                Expr::MethodCall { .. } if len_path(&expr.node, self.env).is_some() => {
                    // Pure length read; its object is a trackable path and
                    // cannot itself contain calls.
                    return;
                }
                Expr::MethodCall { object, method, .. } => {
                    self.sev = self
                        .sev
                        .max(method_call_severity(object, method, self.env, self.target));
                }
                Expr::Call { name, args, .. } => {
                    let leaf = |e: &Expr| typed_path(e, self.env).map(|(_, t)| t);
                    self.sev = self.sev.max(free_call_severity(
                        &name.node,
                        args,
                        self.env,
                        self.target,
                        &leaf,
                    ));
                }
                Expr::StaticTraitCall { .. } | Expr::At { .. } | Expr::Spawn { .. } => {
                    self.sev = CallSeverity::All;
                    return;
                }
                _ => {}
            }
            walk_expr(self, expr);
        }
    }
    let mut scan = Scan { env, target, sev: CallSeverity::Pure };
    scan.visit_expr(expr);
    scan.sev
}

// ─────────────────────────────────────────────────────────────────────────────
// Affine forms
// ─────────────────────────────────────────────────────────────────────────────

/// `k + Σ coeff·path`, all arithmetic in i128 (source constants are i64, so
/// checked i128 ops only overflow on absurd inputs — and then we bail to
/// `None`, i.e. Unknown).
#[derive(Debug, Clone, PartialEq)]
pub struct Affine {
    pub(crate) terms: BTreeMap<String, i128>,
    pub(crate) k: i128,
}

impl Affine {
    pub(crate) fn constant(k: i128) -> Affine {
        Affine { terms: BTreeMap::new(), k }
    }

    pub(crate) fn term(path: String) -> Affine {
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

/// A leaf resolver: maps a (sub)expression directly to an affine form, or
/// `None` to let the structural rules (literals, `+`, `-`, `*` by constant)
/// try, and ultimately give up. The default resolver maps trackable int
/// paths to single-term affines; invariant discharge substitutes ghost
/// variables or construction-site field initializers instead.
pub type AffineResolver<'a> = dyn Fn(&Expr) -> Option<Affine> + 'a;

/// Normalize an integer-typed expression into affine form, resolving leaf
/// paths through `resolve`. Returns `None` for anything outside the
/// fragment.
pub fn to_affine_with(expr: &Expr, resolve: &AffineResolver) -> Option<Affine> {
    if let Some(a) = resolve(expr) {
        return Some(a);
    }
    match expr {
        Expr::IntLit(v) => Some(Affine::constant(*v as i128)),
        Expr::UnaryOp { op: UnaryOp::Neg, operand } => {
            to_affine_with(&operand.node, resolve)?.checked_neg()
        }
        Expr::BinOp { op: BinOp::Add, lhs, rhs } => {
            let l = to_affine_with(&lhs.node, resolve)?;
            let r = to_affine_with(&rhs.node, resolve)?;
            l.checked_add(&r)
        }
        Expr::BinOp { op: BinOp::Sub, lhs, rhs } => {
            let l = to_affine_with(&lhs.node, resolve)?;
            let r = to_affine_with(&rhs.node, resolve)?.checked_neg()?;
            l.checked_add(&r)
        }
        Expr::BinOp { op: BinOp::Mul, lhs, rhs } => {
            let l = to_affine_with(&lhs.node, resolve)?;
            let r = to_affine_with(&rhs.node, resolve)?;
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

/// Normalize an integer-typed expression into affine form over trackable
/// fact terms (int paths and length terms). Returns `None` for anything
/// outside the fragment.
pub(crate) fn to_affine(expr: &Expr, env: &TypeEnv) -> Option<Affine> {
    to_affine_with(expr, &|e| fact_term(e, env).map(Affine::term))
}

/// Difference `l - r` of two affine forms (`None` on overflow).
pub(crate) fn diff_affine(l: &Affine, r: &Affine) -> Option<Affine> {
    l.clone().checked_add(&r.clone().checked_neg()?)
}

/// `a + k`, with checked arithmetic (`None` on overflow).
pub(crate) fn affine_add_const(a: &Affine, k: i128) -> Option<Affine> {
    let mut out = a.clone();
    out.k = out.k.checked_add(k)?;
    Some(out)
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

/// Bound an affine form using the current interval facts, tightened by
/// relation facts wherever a pair of terms has opposite coefficients of
/// equal magnitude (`c·x - c·y` is bounded by `c·(x − y)` when a relation
/// between x and y is known — covering both the pure difference `x - y + k`
/// and mixed forms like `hi + amount - lo`). Returns `(lo, hi)`; `None`
/// means unbounded on that side. Returns `Err(())` when some involved path
/// has a contradictory (empty) interval — the code is unreachable and every
/// query should answer Unknown rather than cascade warnings into dead
/// branches.
pub(crate) fn affine_bounds(a: &Affine, facts: &FactEnv) -> Result<(Option<i128>, Option<i128>), ()> {
    // Per-term interval contributions.
    let mut contribs: Vec<(&str, i128, Option<i128>, Option<i128>)> = Vec::new();
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
        contribs.push((path.as_str(), c, term_lo, term_hi));
    }

    // Sum the interval contributions of every term except the skipped pair.
    let sum_bounds = |skip: Option<(usize, usize)>| -> (Option<i128>, Option<i128>) {
        let mut lo = Some(a.k);
        let mut hi = Some(a.k);
        for (i, (_, _, tl, th)) in contribs.iter().enumerate() {
            if let Some((x, y)) = skip {
                if i == x || i == y {
                    continue;
                }
            }
            lo = add_opt(lo, *tl);
            hi = add_opt(hi, *th);
        }
        (lo, hi)
    };

    let (mut lo, mut hi) = sum_bounds(None);

    // Relation tightening: each opposite-coefficient pair (c·x, -c·y) with a
    // known relation between x and y bounds its combined contribution
    // c·(x − y); the best bound from any single pair decomposition is kept.
    for i in 0..contribs.len() {
        for j in 0..contribs.len() {
            if i == j {
                continue;
            }
            let (x, cx, _, _) = contribs[i];
            let (y, cy, _, _) = contribs[j];
            if cx <= 0 || cx != -cy {
                continue;
            }
            // Bounds on (x − y) from relation facts.
            let mut dlo: Option<i128> = None;
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
            if dlo.is_none() && dhi.is_none() {
                continue;
            }
            let (rest_lo, rest_hi) = sum_bounds(Some((i, j)));
            let pair_lo = dlo.and_then(|d| d.checked_mul(cx));
            let pair_hi = dhi.and_then(|d| d.checked_mul(cx));
            lo = max_opt(lo, add_opt(rest_lo, pair_lo));
            hi = min_opt(hi, add_opt(rest_hi, pair_hi));
        }
    }

    Ok((lo, hi))
}

// ─────────────────────────────────────────────────────────────────────────────
// Non-affine interval evaluation (expr_bounds)
// ─────────────────────────────────────────────────────────────────────────────

const I64_MIN_128: i128 = i64::MIN as i128;
const I64_MAX_128: i128 = i64::MAX as i128;

/// Leaf-bounds hook for [`expr_bounds_with`]: interval bounds for int-valued
/// leaves outside the affine fragment. The production hook
/// ([`byte_value_bounds`]) answers `[0, 255]` for byte values widened to
/// int; tests inject their own. `None` means "no extra information".
pub(crate) type ExtraBounds<'a> = dyn Fn(&Expr) -> Option<(i128, i128)> + 'a;

/// Clamp bounds to the i64 range. Sound for any expression a program
/// actually evaluates as an `int`: arithmetic defects trap (#416), so every
/// completing int expression evaluates to its mathematical value inside
/// i64. `Err(())` when the clamped interval is empty — the facts are
/// contradictory (or the value provably cannot exist), i.e. the code is
/// unreachable, and every consumer must answer "no proof" rather than
/// derive nonsense.
fn clamp_to_i64(lo: Option<i128>, hi: Option<i128>) -> Result<(i128, i128), ()> {
    let lo = lo.unwrap_or(I64_MIN_128).max(I64_MIN_128);
    let hi = hi.unwrap_or(I64_MAX_128).min(I64_MAX_128);
    if lo > hi {
        Err(())
    } else {
        Ok((lo, hi))
    }
}

/// The constant value of an expression, if it normalizes to a term-free
/// affine form (literals, negation, folded constant arithmetic).
fn const_of(e: &Expr, resolve: &AffineResolver) -> Option<i128> {
    let a = to_affine_with(e, resolve)?;
    a.terms.is_empty().then_some(a.k)
}

/// Interval bounds of an **int-valued** expression under the current facts,
/// clamped to the i64 range. Extends the affine fragment with structural
/// rules for shapes an affine form cannot express:
///
/// - `x & c` / `c & x` for constant `c >= 0` lies in `[0, c]` regardless of
///   the other side (AND with a non-negative two's-complement mask clears
///   the sign bit and every bit above the mask's highest set bit). A
///   negative mask contributes nothing.
/// - `x % c` for constant `c > 0`: Pluto's int `%` is truncated (C-style)
///   remainder — codegen lowers it to Cranelift `srem`, so the result's
///   sign follows the dividend (`(-7) % 3 == -1`, pinned in
///   tests/integration/numeric.rs). The result therefore lies in
///   `[-(c-1), c-1]` in general and in `[0, min(x_hi, c-1)]` when `x` is
///   provably `>= 0` (for `0 <= x`, `x % c <= x`). Division by zero traps
///   (defect) and a negative or non-constant divisor is out of scope —
///   both contribute nothing.
/// - `+`/`-`/`*` recurse structurally when the affine pass rejected the
///   whole expression (a masked or byte-valued operand): operand bounds
///   are inside the i64 range, so i128 interval arithmetic cannot
///   overflow, and the node's own completed value is clamped back to the
///   i64 range (trapping semantics, #416).
/// - `extra` resolves leaves the rules above cannot (byte-typed values in
///   production).
///
/// Everything else is the full i64 range (unknown, but still an i64 value).
/// Callers must only pass int-typed expressions. **False bounds are worse
/// than wide ones: when in doubt, the answer is the full range.**
pub(crate) fn expr_bounds_with(
    expr: &Expr,
    resolve: &AffineResolver,
    extra: &ExtraBounds,
    facts: &FactEnv,
) -> Result<(i128, i128), ()> {
    // The affine fragment first — it is exact and sees interval AND
    // relation facts.
    if let Some(a) = to_affine_with(expr, resolve) {
        let (lo, hi) = affine_bounds(&a, facts)?;
        return clamp_to_i64(lo, hi);
    }
    if let Some((lo, hi)) = extra(expr) {
        return clamp_to_i64(Some(lo), Some(hi));
    }
    match expr {
        Expr::BinOp { op: BinOp::BitAnd, lhs, rhs } => {
            let mask = [&lhs.node, &rhs.node]
                .into_iter()
                .filter_map(|s| const_of(s, resolve))
                .filter(|c| *c >= 0)
                .min();
            match mask {
                Some(c) => clamp_to_i64(Some(0), Some(c)),
                None => Ok((I64_MIN_128, I64_MAX_128)),
            }
        }
        Expr::BinOp { op: BinOp::Mod, lhs, rhs } => match const_of(&rhs.node, resolve) {
            Some(c) if c > 0 => {
                let (llo, lhi) = expr_bounds_with(&lhs.node, resolve, extra, facts)?;
                if llo >= 0 {
                    clamp_to_i64(Some(0), Some((c - 1).min(lhi)))
                } else {
                    clamp_to_i64(Some(-(c - 1)), Some(c - 1))
                }
            }
            _ => Ok((I64_MIN_128, I64_MAX_128)),
        },
        Expr::BinOp { op: op @ (BinOp::Add | BinOp::Sub | BinOp::Mul), lhs, rhs } => {
            let (la, ha) = expr_bounds_with(&lhs.node, resolve, extra, facts)?;
            let (lb, hb) = expr_bounds_with(&rhs.node, resolve, extra, facts)?;
            let (lo, hi) = match op {
                BinOp::Add => (la + lb, ha + hb),
                BinOp::Sub => (la - hb, ha - lb),
                BinOp::Mul => {
                    let p = [la * lb, la * hb, ha * lb, ha * hb];
                    (
                        *p.iter().min().expect("non-empty"),
                        *p.iter().max().expect("non-empty"),
                    )
                }
                _ => unreachable!("matched Add | Sub | Mul above"),
            };
            clamp_to_i64(Some(lo), Some(hi))
        }
        Expr::UnaryOp { op: UnaryOp::Neg, operand } => {
            let (lo, hi) = expr_bounds_with(&operand.node, resolve, extra, facts)?;
            clamp_to_i64(Some(-hi), Some(-lo))
        }
        _ => Ok((I64_MIN_128, I64_MAX_128)),
    }
}

/// [`expr_bounds_with`] over the default vocabulary: trackable fact terms
/// as affine leaves, byte-typed values as `[0, 255]`. Consumed by the
/// overflow/shift-check elision pass (arith_fit.rs) for operands outside
/// the affine fragment.
pub(crate) fn expr_bounds(expr: &Expr, env: &TypeEnv) -> Result<(i128, i128), ()> {
    expr_bounds_with(
        expr,
        &|e| fact_term(e, env).map(Affine::term),
        &|e| byte_value_bounds(e, env),
        &env.facts,
    )
}

/// `[0, 255]` bounds for byte values and their int widenings: trackable
/// byte-typed paths (params, locals, fields), element reads from `bytes`
/// and `[byte]`, casts *to* byte, and `as int` casts of any of those. A
/// byte's value is 0..255 by construction — a type fact, not a flow fact,
/// so it holds wherever the value is read (inside loops, after calls).
pub(crate) fn byte_value_bounds(e: &Expr, env: &TypeEnv) -> Option<(i128, i128)> {
    if let Expr::Cast { expr, target_type } = e {
        if matches!(&target_type.node, TypeExpr::Named(n) if n == "int")
            && is_byte_valued(&expr.node, env)
        {
            return Some((0, 255));
        }
        return None;
    }
    is_byte_valued(e, env).then_some((0, 255))
}

/// Is this expression byte-typed by construction? Trackable byte paths,
/// element reads from `bytes` / `[byte]`, and casts to byte. Conservative:
/// anything unrecognized (method calls, map reads, ...) is not.
fn is_byte_valued(e: &Expr, env: &TypeEnv) -> bool {
    match e {
        Expr::Index { object, .. } => match typed_path(&object.node, env) {
            Some((_, PlutoType::Bytes)) => true,
            Some((_, PlutoType::Array(el))) => *el == PlutoType::Byte,
            _ => false,
        },
        Expr::Cast { target_type, .. } => {
            matches!(&target_type.node, TypeExpr::Named(n) if n == "byte")
        }
        _ => matches!(typed_path(e, env), Some((_, PlutoType::Byte))),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Condition evaluation (the `implies` API)
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn is_comparison(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq | BinOp::Eq | BinOp::Neq
    )
}

/// Decide a boolean condition against the current facts. See the module docs
/// for the fragment; anything outside it is `Unknown`.
pub fn eval_condition(cond: &Expr, env: &TypeEnv) -> Verdict {
    eval_condition_with(cond, &|e| fact_term(e, env).map(Affine::term), &env.facts)
}

/// Decide a boolean condition with leaf paths resolved through `resolve`
/// and bounds drawn from `facts`. Same fragment and conservatism as
/// [`eval_condition`].
pub fn eval_condition_with(cond: &Expr, resolve: &AffineResolver, facts: &FactEnv) -> Verdict {
    match cond {
        Expr::BinOp { op: BinOp::And, lhs, rhs } => {
            let l = eval_condition_with(&lhs.node, resolve, facts);
            let r = eval_condition_with(&rhs.node, resolve, facts);
            match (l, r) {
                (Verdict::Refuted, _) | (_, Verdict::Refuted) => Verdict::Refuted,
                (Verdict::Proven, Verdict::Proven) => Verdict::Proven,
                _ => Verdict::Unknown,
            }
        }
        Expr::BinOp { op: BinOp::Or, lhs, rhs } => {
            let l = eval_condition_with(&lhs.node, resolve, facts);
            let r = eval_condition_with(&rhs.node, resolve, facts);
            match (l, r) {
                (Verdict::Proven, _) | (_, Verdict::Proven) => Verdict::Proven,
                (Verdict::Refuted, Verdict::Refuted) => Verdict::Refuted,
                _ => Verdict::Unknown,
            }
        }
        Expr::UnaryOp { op: UnaryOp::Not, operand } => {
            eval_condition_with(&operand.node, resolve, facts).negate()
        }
        Expr::BinOp { op, lhs, rhs } if is_comparison(*op) => {
            eval_comparison(*op, &lhs.node, &rhs.node, resolve, facts)
        }
        // A chained comparison is the conjunction of its adjacent pairs
        // (same combination rule as `&&` above).
        Expr::CompareChain { operands, ops } => {
            let mut verdict = Verdict::Proven;
            for (i, op) in ops.iter().enumerate() {
                let v = eval_comparison(*op, &operands[i].node, &operands[i + 1].node, resolve, facts);
                verdict = match (verdict, v) {
                    (Verdict::Refuted, _) | (_, Verdict::Refuted) => return Verdict::Refuted,
                    (Verdict::Proven, Verdict::Proven) => Verdict::Proven,
                    _ => Verdict::Unknown,
                };
            }
            verdict
        }
        _ => Verdict::Unknown,
    }
}

fn eval_comparison(
    op: BinOp,
    lhs: &Expr,
    rhs: &Expr,
    resolve: &AffineResolver,
    facts: &FactEnv,
) -> Verdict {
    let Some(l) = to_affine_with(lhs, resolve) else { return Verdict::Unknown };
    let Some(r) = to_affine_with(rhs, resolve) else { return Verdict::Unknown };
    let Some(neg_r) = r.checked_neg() else { return Verdict::Unknown };
    let Some(d) = l.checked_add(&neg_r) else { return Verdict::Unknown };

    // A - B cmp 0, with (lo, hi) bounding A - B.
    let Ok((lo, hi)) = affine_bounds(&d, facts) else {
        // Contradictory facts: this code is unreachable; don't cascade.
        return Verdict::Unknown;
    };

    let lo_ge = |v: i128| lo.is_some_and(|l| l >= v);
    let hi_le = |v: i128| hi.is_some_and(|h| h <= v);

    // `x != y` facts only decide equality when the difference is exactly
    // x - y (no constant offset).
    let ne_rel_known = d.k == 0
        && d.as_diff()
            .is_some_and(|(x, y)| facts.rel_holds(x, RelOp::Ne, y));

    // Single-term equations `c·x + k == 0` are impossible when no integer
    // solution exists (indivisible, or out of i64 range) or when an
    // inequation fact excludes the solution (`x != v`).
    let eq_impossible = if d.terms.len() == 1 {
        let (p, &c) = d.terms.iter().next().expect("len checked");
        if d.k % c != 0 {
            true
        } else {
            match i64::try_from(-d.k / c) {
                Ok(sol) => facts.ne_const_holds(p, sol),
                Err(_) => true,
            }
        }
    } else {
        false
    };
    let ne_known = ne_rel_known || eq_impossible;

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
/// contains no impure call-like expressions (see [`contains_impure_call`]).
pub fn condition_facts(cond: &Expr, env: &TypeEnv) -> CondFacts {
    condition_facts_with(cond, &|e| fact_term(e, env).map(Affine::term))
}

/// Extract branch facts from a condition with leaf paths resolved through
/// `resolve`. Same contract as [`condition_facts`].
pub fn condition_facts_with(cond: &Expr, resolve: &AffineResolver) -> CondFacts {
    match cond {
        Expr::BinOp { op: BinOp::And, lhs, rhs } => {
            // Both conjuncts hold in the then-branch; the negation of a
            // conjunction is a disjunction, which yields no usable facts.
            let l = condition_facts_with(&lhs.node, resolve);
            let r = condition_facts_with(&rhs.node, resolve);
            CondFacts {
                then_facts: [l.then_facts, r.then_facts].concat(),
                else_facts: Vec::new(),
            }
        }
        Expr::BinOp { op: BinOp::Or, lhs, rhs } => {
            // De Morgan: both negations hold in the else-branch.
            let l = condition_facts_with(&lhs.node, resolve);
            let r = condition_facts_with(&rhs.node, resolve);
            CondFacts {
                then_facts: Vec::new(),
                else_facts: [l.else_facts, r.else_facts].concat(),
            }
        }
        Expr::UnaryOp { op: UnaryOp::Not, operand } => {
            let inner = condition_facts_with(&operand.node, resolve);
            CondFacts {
                then_facts: inner.else_facts,
                else_facts: inner.then_facts,
            }
        }
        Expr::BinOp { op, lhs, rhs } if is_comparison(*op) => {
            comparison_facts(*op, &lhs.node, &rhs.node, resolve)
        }
        // A chained comparison is the conjunction of its adjacent pairs:
        // every pair holds in the then-branch; the negation is a
        // disjunction, which yields no usable facts — except for a lone
        // pair, which can't happen (the parser keeps single comparisons
        // as BinOp).
        Expr::CompareChain { operands, ops } => {
            let mut then_facts = Vec::new();
            for (i, op) in ops.iter().enumerate() {
                then_facts.extend(
                    comparison_facts(*op, &operands[i].node, &operands[i + 1].node, resolve)
                        .then_facts,
                );
            }
            CondFacts { then_facts, else_facts: Vec::new() }
        }
        _ => CondFacts::default(),
    }
}

/// Facts from a single comparison `lhs <op> rhs` in the affine fragment.
fn comparison_facts(op: BinOp, lhs: &Expr, rhs: &Expr, resolve: &AffineResolver) -> CondFacts {
    let Some(l) = to_affine_with(lhs, resolve) else { return CondFacts::default() };
    let Some(r) = to_affine_with(rhs, resolve) else { return CondFacts::default() };
    let Some(neg_r) = r.checked_neg() else { return CondFacts::default() };
    let Some(d) = l.checked_add(&neg_r) else { return CondFacts::default() };
    CondFacts {
        then_facts: facts_from_diff(op, &d),
        else_facts: facts_from_diff(negate_cmp(op), &d),
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
pub(crate) fn facts_from_diff(op: BinOp, d: &Affine) -> Vec<Fact> {
    // Single-variable interval facts.
    if d.terms.len() == 1 {
        let (path, &c) = d.terms.iter().next().expect("len checked");
        debug_assert!(c != 0, "zero coefficients are pruned");
        // c·x + k != 0 ⇒ x != -k/c when that quotient is an exact i64
        // (otherwise the inequation is vacuously true — no fact).
        if op == BinOp::Neq {
            if d.k % c == 0 {
                if let Ok(sol) = i64::try_from(-d.k / c) {
                    return vec![Fact::NeConst(path.clone(), sol)];
                }
            }
            return Vec::new();
        }
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

/// The expressions a statement evaluates *directly* (nested blocks are not
/// scanned — their statements are visited by the checker in flow order).
/// Shared by the kill rules below and by invariant discharge's call
/// detection.
pub fn immediate_exprs(stmt: &crate::parser::ast::Stmt) -> Vec<&Spanned<Expr>> {
    use crate::parser::ast::{SelectOp, Stmt};

    let mut exprs: Vec<&Spanned<Expr>> = Vec::new();
    match stmt {
        Stmt::Let { value, .. } => exprs.push(value),
        Stmt::Assign { value, .. } => exprs.push(value),
        Stmt::FieldAssign { object, value, .. } => {
            exprs.push(object);
            exprs.push(value);
        }
        Stmt::While { condition, .. } => exprs.push(condition),
        Stmt::For { iterable, .. } => exprs.push(iterable),
        Stmt::Return(value) => {
            if let Some(v) = value {
                exprs.push(v);
            }
        }
        Stmt::If { condition, .. } => exprs.push(condition),
        Stmt::IndexAssign { object, index, value } => {
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
        Stmt::Select { arms, after, .. } => {
            for arm in arms {
                match &arm.op {
                    SelectOp::Recv { channel, .. } => exprs.push(channel),
                    SelectOp::Send { channel, value } => {
                        exprs.push(channel);
                        exprs.push(value);
                    }
                }
            }
            if let Some(a) = after {
                exprs.push(&a.duration);
            }
        }
        Stmt::Scope { seeds, .. } => {
            for s in seeds {
                exprs.push(s);
            }
        }
        Stmt::Yield { value } => exprs.push(value),
        Stmt::Assert { expr } => exprs.push(expr),
        // Evaluates nothing directly; its block's statements are visited by
        // the checker in flow order.
        Stmt::ExpectRaises { .. } => {}
        Stmt::Serve { service, port } => {
            exprs.push(service);
            exprs.push(port);
        }
        Stmt::Break | Stmt::Continue => {}
        Stmt::Expr(e) => exprs.push(e),
    }
    exprs
}

/// Apply a statement's fact kills before it is checked. See the module docs
/// for the kill rules. Nested blocks are *not* scanned here — their
/// statements apply their own kills as the checker reaches them, which is
/// what makes the analysis flow-sensitive.
pub fn apply_stmt_kills(stmt: &crate::parser::ast::Stmt, env: &mut TypeEnv) {
    use crate::parser::ast::Stmt;

    match stmt {
        Stmt::Let { name, .. } => {
            // A fresh binding can't shadow (rejected earlier), but kill
            // defensively in case a same-named fact survived a sibling scope.
            env.facts.kill_path(&name.node);
        }
        Stmt::Assign { target, .. } => {
            env.facts.kill_path(&target.node);
        }
        Stmt::FieldAssign { .. } => {
            // Aliasing: another local may reference the same object, so a
            // field write invalidates every field fact.
            env.facts.kill_fields();
        }
        Stmt::While { .. } | Stmt::For { .. } => {
            // Conservative loop rule: drop everything at loop entry (and
            // therefore after the loop). See module docs.
            env.facts.havoc_all();
        }
        Stmt::Return(_)
        | Stmt::If { .. }
        | Stmt::IndexAssign { .. }
        | Stmt::Match { .. }
        | Stmt::Raise { .. }
        | Stmt::LetChan { .. }
        | Stmt::Select { .. }
        | Stmt::Scope { .. }
        | Stmt::Yield { .. }
        | Stmt::Assert { .. }
        | Stmt::ExpectRaises { .. }
        | Stmt::Serve { .. }
        | Stmt::Break
        | Stmt::Continue
        | Stmt::Expr(_) => {}
    }

    // Call kills, scaled by the shared severity classification (see the
    // module docs): Pure kills nothing, Collections kills length terms
    // (builtin mutators change lengths through aliases), All kills every
    // field-path fact (length terms are dotted paths, so they die too).
    let sev = immediate_exprs(stmt)
        .iter()
        .map(|e| call_severity(e, env, None))
        .max()
        .unwrap_or(CallSeverity::Pure);
    match sev {
        CallSeverity::Pure => {}
        CallSeverity::Collections => env.facts.kill_len_terms(),
        CallSeverity::All => env.facts.kill_fields(),
    }
}

/// Facts a `let`/`=` binding establishes about its target. Deliberately
/// narrow — three shapes transfer:
///
/// - a direct `xs.len()` value (the bound variable inherits the automatic
///   `>= 0` and an equality to the length term, which dies with the usual
///   kills while the `>= 0` bound — true of the captured value forever —
///   survives them);
/// - a struct literal of a (non-entity) class: construction is fully
///   transparent — the construction proof obligation already evaluated the
///   initializers, so the caller's fact env learns the exact int-field
///   values (`acc.balance == 100` after `let acc = Account { balance: 100 }`).
///   The usual kill rules take over from there. Entities are excluded
///   (entity fields never carry flow facts);
/// - a byte widening (`b as int`, `data[i] as int` — see
///   [`byte_value_bounds`]): the binding starts bounded to `[0, 255]`.
///
/// General value-to-binding fact transfer is out of scope. Callers assume
/// these after the binding is defined; the statement's own kill (of the
/// target's stale facts) has already run.
pub(crate) fn binding_facts(name: &str, value: &Expr, env: &TypeEnv) -> Vec<Fact> {
    if let Some(lp) = len_path(value, env) {
        return vec![
            Fact::Bound(name.to_string(), Interval::at_least(0)),
            Fact::Rel(name.to_string(), RelOp::Eq, lp),
        ];
    }
    if let Expr::StructLit { name: cls, fields, .. } = value {
        return construction_facts(name, &cls.node, fields, env);
    }
    // Byte widening (`let x = b as int`, `let x = data[i] as int`): the
    // captured value is a byte's, so the binding starts in [0, 255]. Like
    // the `>= 0` length bound above, this is a fact about the captured
    // value itself — it holds until the binding is reassigned.
    if matches!(value, Expr::Cast { .. }) && byte_value_bounds(value, env).is_some() {
        return vec![Fact::Bound(name.to_string(), Interval { lo: 0, hi: 255 })];
    }
    Vec::new()
}

/// Exact post-construction facts for a struct-literal binding: for every
/// int field whose initializer normalizes to an affine form,
/// `root.field == <initializer affine>`. When the literal contains an
/// impure call, only call-stable affines (constants and caller locals — no
/// dotted terms) are kept: field initializers evaluate in order, so a
/// sibling initializer's call may have mutated the object a dotted term
/// reads through.
fn construction_facts(
    root: &str,
    cls: &str,
    lit_fields: &[(Spanned<String>, Spanned<Expr>)],
    env: &TypeEnv,
) -> Vec<Fact> {
    // Entities mutate concurrently — their fields never carry flow facts.
    if env.object_types.contains(cls)
        || env.remote_types.contains(cls)
        || env.domain_types.contains(cls)
    {
        return Vec::new();
    }
    let Some(info) = env.classes.get(cls) else {
        return Vec::new();
    };
    let had_call = lit_fields
        .iter()
        .any(|(_, v)| contains_impure_call(v, env));
    let mut out = Vec::new();
    for (fname, fexpr) in lit_fields {
        let is_int_field = info
            .fields
            .iter()
            .any(|(n, t, _)| n == &fname.node && *t == PlutoType::Int);
        if !is_int_field {
            continue;
        }
        let Some(aff) = to_affine(&fexpr.node, env) else {
            continue;
        };
        // Call-stability and self-reference guards: dotted terms are
        // dropped when any initializer called out, and terms rooted at the
        // binding itself (an `x = C { f: x.f }` rebinding) are never valid
        // post-binding.
        if aff.terms.keys().any(|t| path_under(t, root))
            || (had_call && aff.terms.keys().any(|t| t.contains('.')))
        {
            continue;
        }
        let path = Affine::term(format!("{root}.{}", fname.node));
        if let Some(d) = diff_affine(&path, &aff) {
            out.extend(facts_from_diff(BinOp::Eq, &d));
        }
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests — interval / relation lattice and the affine machinery
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Membership facts (idempotency pass, phase 5.5) ───────────────────

    #[test]
    fn set_insert_arms_only_under_not_contains() {
        let mut f = FactEnv::new();
        // Unarmed: no not-contains fact is live.
        assert!(!f.apply_set_insert("self.seen", Some("k")));
        assert!(!f.set_inserted_for_key("k"));
        // Armed: the check's fact licenses the insert.
        f.assume(Fact::SetNotContains("self.seen".into(), "k".into()));
        assert!(f.set_not_contains_holds("self.seen", "k"));
        assert!(f.apply_set_insert("self.seen", Some("k")));
        assert!(f.set_inserted_for_key("k"));
        // The insert consumed the not-contains fact (contains is now true).
        assert!(!f.set_not_contains_holds("self.seen", "k"));
    }

    #[test]
    fn any_insert_kills_not_contains_on_the_set() {
        let mut f = FactEnv::new();
        f.assume(Fact::SetNotContains("self.seen".into(), "k".into()));
        // Inserting a DIFFERENT value may still insert k's value.
        assert!(!f.apply_set_insert("self.seen", None));
        assert!(!f.set_not_contains_holds("self.seen", "k"));
    }

    #[test]
    fn inserted_survives_field_kills_for_local_keys_only() {
        let mut f = FactEnv::new();
        f.assume(Fact::SetInserted("self.seen".into(), "k".into()));
        f.assume(Fact::SetInserted("self.seen".into(), "req.id".into()));
        // A call that may run user code: monotone armed inserts survive on
        // the set side, but a dotted KEY path may no longer denote the
        // inserted value.
        f.kill_fields();
        assert!(f.set_inserted_for_key("k"));
        assert!(!f.set_inserted_for_key("req.id"));
        // Not-contains facts never survive a call.
        f.assume(Fact::SetNotContains("self.seen".into(), "k".into()));
        f.kill_fields();
        assert!(!f.set_not_contains_holds("self.seen", "k"));
        // Collections-severity calls kill not-contains too.
        f.assume(Fact::SetNotContains("self.seen".into(), "k".into()));
        f.kill_len_terms();
        assert!(!f.set_not_contains_holds("self.seen", "k"));
        // Reassigning the key root kills the armed insert.
        f.kill_path("k");
        assert!(!f.set_inserted_for_key("k"));
    }

    #[test]
    fn havoc_clears_membership_facts() {
        let mut f = FactEnv::new();
        f.assume(Fact::SetInserted("self.seen".into(), "k".into()));
        f.havoc_all();
        assert!(!f.set_inserted_for_key("k"));
    }

    #[test]
    fn membership_facts_pop_with_their_frame() {
        let mut f = FactEnv::new();
        f.push_frame();
        f.assume(Fact::SetInserted("self.seen".into(), "k".into()));
        assert!(f.set_inserted_for_key("k"));
        f.pop_frame();
        assert!(!f.set_inserted_for_key("k"));
    }

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
    fn kill_len_terms_spares_field_paths() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("x".into(), Interval::at_most(1)));
        f.assume(Fact::Bound("self.balance".into(), Interval::at_least(0)));
        f.assume(Fact::Bound("xs.len()".into(), Interval::at_least(3)));
        f.assume(Fact::Rel("n".into(), RelOp::Eq, "xs.len()".into()));
        let mark = f.kill_mark();
        f.kill_len_terms();
        // Locals and plain field paths survive.
        assert_eq!(f.interval_of("x"), Interval::at_most(1));
        assert_eq!(f.interval_of("self.balance"), Interval::at_least(0));
        // Length terms are back to the automatic >= 0 and relations
        // involving them die.
        assert_eq!(f.interval_of("xs.len()"), Interval::at_least(0));
        assert!(!f.rel_holds("n", RelOp::Eq, "xs.len()"));
        // killed_since sees len-term facts as invalidated, others not.
        assert!(f.killed_since(mark, &Fact::Bound("xs.len()".into(), Interval::at_least(3))));
        assert!(!f.killed_since(mark, &Fact::Bound("self.balance".into(), Interval::at_least(0))));
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
    fn extract_neq_yields_ne_const() {
        // x - 5 != 0 ⇒ x != 5
        let facts = facts_from_diff(BinOp::Neq, &single("x", 1, -5));
        assert_eq!(facts, vec![Fact::NeConst("x".into(), 5)]);
        // 2x - 7 != 0 is vacuously true over the integers — no fact.
        let facts = facts_from_diff(BinOp::Neq, &single("x", 2, -7));
        assert!(facts.is_empty());
    }

    #[test]
    fn ne_const_decides_equality() {
        let mut f = FactEnv::new();
        f.assume(Fact::NeConst("x".into(), 0));
        assert!(f.ne_const_holds("x", 0));
        assert!(!f.ne_const_holds("x", 1));
        f.kill_path("x");
        assert!(!f.ne_const_holds("x", 0));
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
    fn bounds_pair_decomposition_with_extra_term() {
        // d = hi + amount - lo with lo < hi and amount ≥ 0: the (hi, -lo)
        // pair contributes ≥ 1 via the relation, amount contributes ≥ 0.
        let mut f = FactEnv::new();
        f.assume(Fact::Rel("lo".into(), RelOp::Lt, "hi".into()));
        f.assume(Fact::Bound("amount".into(), Interval::at_least(0)));
        let mut terms = BTreeMap::new();
        terms.insert("hi".to_string(), 1i128);
        terms.insert("amount".to_string(), 1i128);
        terms.insert("lo".to_string(), -1i128);
        let d = Affine { terms, k: 0 };
        let (lo, hi) = affine_bounds(&d, &f).unwrap();
        assert_eq!(lo, Some(1));
        assert_eq!(hi, None);
    }

    #[test]
    fn bounds_pair_decomposition_scaled_coefficients() {
        // d = 2x - 2y with x ≤ y ⇒ 2(x−y) ≤ 0.
        let mut f = FactEnv::new();
        f.assume(Fact::Rel("x".into(), RelOp::Le, "y".into()));
        let mut terms = BTreeMap::new();
        terms.insert("x".to_string(), 2i128);
        terms.insert("y".to_string(), -2i128);
        let d = Affine { terms, k: 0 };
        let (lo, hi) = affine_bounds(&d, &f).unwrap();
        assert_eq!(lo, None);
        assert_eq!(hi, Some(0));
    }

    #[test]
    fn bounds_pair_decomposition_keeps_constant() {
        // d = x - y + 5 with x == y ⇒ exactly 5 (the original pure-diff case).
        let mut f = FactEnv::new();
        f.assume(Fact::Rel("x".into(), RelOp::Eq, "y".into()));
        let mut terms = BTreeMap::new();
        terms.insert("x".to_string(), 1i128);
        terms.insert("y".to_string(), -1i128);
        let d = Affine { terms, k: 5 };
        let (lo, hi) = affine_bounds(&d, &f).unwrap();
        assert_eq!(lo, Some(5));
        assert_eq!(hi, Some(5));
    }

    #[test]
    fn bounds_empty_interval_is_error() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("x".into(), Interval::EMPTY));
        let d = single("x", 1, 0);
        assert_eq!(affine_bounds(&d, &f), Err(()));
    }

    // ── Length terms ─────────────────────────────────────────────────────

    #[test]
    fn len_term_detection() {
        assert!(is_len_term("xs.len()"));
        assert!(is_len_term("p.items.len()"));
        assert!(is_len_term("s@0.len()@2")); // epoch-stamped ghost form
        assert!(!is_len_term("xs"));
        assert!(!is_len_term("p.length"));
        assert!(!is_len_term("len"));
    }

    #[test]
    fn len_term_auto_nonnegative() {
        let f = FactEnv::new();
        // With no recorded facts, a length term is still known >= 0.
        assert_eq!(f.interval_of("xs.len()"), Interval::at_least(0));
        assert_eq!(f.interval_of("xs"), Interval::TOP);
    }

    #[test]
    fn len_term_bound_intersects_auto_nonneg() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("xs.len()".into(), Interval::at_most(5)));
        assert_eq!(f.interval_of("xs.len()"), Interval { lo: 0, hi: 5 });
        // A contradictory upper bound empties against the automatic >= 0.
        f.assume(Fact::Bound("xs.len()".into(), Interval::at_most(-1)));
        assert!(f.interval_of("xs.len()").is_empty());
    }

    #[test]
    fn len_nonneg_decides_comparisons() {
        // xs.len() >= 0 is Proven and xs.len() < 0 is Refuted with no facts.
        let f = FactEnv::new();
        let d = single("xs.len()", 1, 0);
        assert_eq!(affine_bounds(&d, &f), Ok((Some(0), None)));
    }

    #[test]
    fn len_term_affine_participation() {
        // d = xs.len() - 1 with xs.len() <= 10: bounds [-1, 9].
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("xs.len()".into(), Interval::at_most(10)));
        let d = single("xs.len()", 1, -1);
        assert_eq!(affine_bounds(&d, &f), Ok((Some(-1), Some(9))));
    }

    #[test]
    fn len_narrows_through_relations() {
        // i < xs.len() recorded as a relation: i - xs.len() + 1 <= 0, i.e.
        // `i <= xs.len() - 1` is provable from the pair decomposition.
        let mut f = FactEnv::new();
        f.assume(Fact::Rel("i".into(), RelOp::Lt, "xs.len()".into()));
        let mut terms = BTreeMap::new();
        terms.insert("i".to_string(), 1i128);
        terms.insert("xs.len()".to_string(), -1i128);
        let d = Affine { terms, k: 1 };
        let (_, hi) = affine_bounds(&d, &f).unwrap();
        assert_eq!(hi, Some(0));
    }

    #[test]
    fn reassigning_base_kills_len_term() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("xs.len()".into(), Interval::at_most(5)));
        f.kill_path("xs");
        // Back to only the automatic bound.
        assert_eq!(f.interval_of("xs.len()"), Interval::at_least(0));
    }

    #[test]
    fn call_kill_drops_len_terms() {
        // Length terms are dotted paths: the conservative field kill (any
        // impure call — including mut method calls and mut-arg passes on
        // the base) drops them too.
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("xs.len()".into(), Interval::at_most(5)));
        f.assume(Fact::Rel("i".into(), RelOp::Lt, "xs.len()".into()));
        f.kill_fields();
        assert_eq!(f.interval_of("xs.len()"), Interval::at_least(0));
        assert!(!f.rel_holds("i", RelOp::Lt, "xs.len()"));
        // Facts on plain locals survive.
        f.assume(Fact::Bound("i".into(), Interval::at_most(3)));
        f.kill_fields();
        assert_eq!(f.interval_of("i"), Interval::at_most(3));
    }

    #[test]
    fn kill_mark_sees_len_kills() {
        let mut f = FactEnv::new();
        let fact = Fact::Bound("xs.len()".into(), Interval::at_most(5));
        f.assume(fact.clone());
        let mark = f.kill_mark();
        f.kill_fields();
        assert!(f.killed_since(mark, &fact));
        let mark2 = f.kill_mark();
        f.kill_path("xs");
        assert!(f.killed_since(mark2, &fact));
        let mark3 = f.kill_mark();
        f.kill_path("ys");
        assert!(!f.killed_since(mark3, &fact));
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

    // ── expr_bounds_with: masks, remainders, structural recursion ────────

    const FULL: (i128, i128) = (i64::MIN as i128, i64::MAX as i128);

    fn ident_resolve(e: &Expr) -> Option<Affine> {
        match e {
            Expr::Ident(n) => Some(Affine::term(n.clone())),
            _ => None,
        }
    }

    fn no_extra(_: &Expr) -> Option<(i128, i128)> {
        None
    }

    fn sp(e: Expr) -> Box<Spanned<Expr>> {
        Box::new(Spanned::dummy(e))
    }

    fn bin(op: BinOp, l: Expr, r: Expr) -> Expr {
        Expr::BinOp { op, lhs: sp(l), rhs: sp(r) }
    }

    fn ident(n: &str) -> Expr {
        Expr::Ident(n.to_string())
    }

    fn bounds(e: &Expr, f: &FactEnv) -> Result<(i128, i128), ()> {
        expr_bounds_with(e, &ident_resolve, &no_extra, f)
    }

    #[test]
    fn mask_const_bounds_either_side() {
        let f = FactEnv::new();
        // x & 255 ∈ [0, 255] regardless of x.
        let e = bin(BinOp::BitAnd, ident("x"), Expr::IntLit(255));
        assert_eq!(bounds(&e, &f), Ok((0, 255)));
        // Constant on the left too.
        let e = bin(BinOp::BitAnd, Expr::IntLit(255), ident("x"));
        assert_eq!(bounds(&e, &f), Ok((0, 255)));
        // x & 0 is exactly 0.
        let e = bin(BinOp::BitAnd, ident("x"), Expr::IntLit(0));
        assert_eq!(bounds(&e, &f), Ok((0, 0)));
        // Two constant masks: the smaller wins (5 & 3 ≤ 3).
        let e = bin(BinOp::BitAnd, Expr::IntLit(5), Expr::IntLit(3));
        assert_eq!(bounds(&e, &f), Ok((0, 3)));
    }

    #[test]
    fn mask_negative_or_nonconst_contributes_nothing() {
        let f = FactEnv::new();
        // x & -1 == x: a negative mask preserves the sign bit — NO fact.
        let neg_one = Expr::UnaryOp { op: UnaryOp::Neg, operand: sp(Expr::IntLit(1)) };
        let e = bin(BinOp::BitAnd, ident("x"), neg_one);
        assert_eq!(bounds(&e, &f), Ok(FULL));
        // x & y with no constant side — NO fact.
        let e = bin(BinOp::BitAnd, ident("x"), ident("y"));
        assert_eq!(bounds(&e, &f), Ok(FULL));
        // Other bitwise ops never get mask bounds (x | c can be huge).
        let e = bin(BinOp::BitOr, ident("x"), Expr::IntLit(255));
        assert_eq!(bounds(&e, &f), Ok(FULL));
        let e = bin(BinOp::BitXor, ident("x"), Expr::IntLit(255));
        assert_eq!(bounds(&e, &f), Ok(FULL));
    }

    #[test]
    fn mod_const_bounds_truncated_semantics() {
        // Pluto `%` is truncated (srem): the sign follows the DIVIDEND —
        // `(-7) % 3 == -1` — so without a dividend sign the bound must be
        // symmetric. Pinned against codegen in tests/integration/numeric.rs.
        let f = FactEnv::new();
        let e = bin(BinOp::Mod, ident("x"), Expr::IntLit(10));
        assert_eq!(bounds(&e, &f), Ok((-9, 9)));
    }

    #[test]
    fn mod_const_nonneg_dividend_tightens() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("x".into(), Interval::at_least(0)));
        let e = bin(BinOp::Mod, ident("x"), Expr::IntLit(10));
        assert_eq!(bounds(&e, &f), Ok((0, 9)));
        // A dividend already tighter than the divisor caps the result:
        // 0 <= x <= 5 ⇒ x % 10 == x ∈ [0, 5].
        f.assume(Fact::Bound("x".into(), Interval::at_most(5)));
        assert_eq!(bounds(&e, &f), Ok((0, 5)));
    }

    #[test]
    fn mod_zero_or_negative_divisor_contributes_nothing() {
        let f = FactEnv::new();
        // x % 0 traps at runtime — the bound engine must claim NOTHING.
        let e = bin(BinOp::Mod, ident("x"), Expr::IntLit(0));
        assert_eq!(bounds(&e, &f), Ok(FULL));
        // Negative divisors are out of scope — NO fact.
        let neg3 = Expr::UnaryOp { op: UnaryOp::Neg, operand: sp(Expr::IntLit(3)) };
        let e = bin(BinOp::Mod, ident("x"), neg3);
        assert_eq!(bounds(&e, &f), Ok(FULL));
        // Non-constant divisor — NO fact.
        let e = bin(BinOp::Mod, ident("x"), ident("y"));
        assert_eq!(bounds(&e, &f), Ok(FULL));
    }

    #[test]
    fn structural_recursion_composes_masks() {
        // (x & 255) * 256 + (y & 255) ∈ [0, 65535] — the two-byte pack.
        let f = FactEnv::new();
        let lo_byte = |v: &str| bin(BinOp::BitAnd, ident(v), Expr::IntLit(255));
        let e = bin(
            BinOp::Add,
            bin(BinOp::Mul, lo_byte("x"), Expr::IntLit(256)),
            lo_byte("y"),
        );
        assert_eq!(bounds(&e, &f), Ok((0, 65535)));
        // Negation flips the interval.
        let e = Expr::UnaryOp { op: UnaryOp::Neg, operand: sp(lo_byte("x")) };
        assert_eq!(bounds(&e, &f), Ok((-255, 0)));
        // Subtraction of two masked values.
        let e = bin(BinOp::Sub, lo_byte("x"), lo_byte("y"));
        assert_eq!(bounds(&e, &f), Ok((-255, 255)));
    }

    #[test]
    fn extra_hook_resolves_leaves() {
        // Model a byte-valued leaf: `b` is not affine-resolvable, the extra
        // hook answers [0, 255] — the production shape for `data[i] as int`.
        let f = FactEnv::new();
        let resolve = |e: &Expr| match e {
            Expr::Ident(n) if n == "b" => None,
            other => ident_resolve(other),
        };
        let extra = |e: &Expr| match e {
            Expr::Ident(n) if n == "b" => Some((0i128, 255i128)),
            _ => None,
        };
        let e = bin(
            BinOp::Add,
            bin(BinOp::Mul, ident("b"), Expr::IntLit(256)),
            ident("b"),
        );
        assert_eq!(expr_bounds_with(&e, &resolve, &extra, &f), Ok((0, 65535)));
    }

    #[test]
    fn unknown_expr_is_full_i64_range() {
        let f = FactEnv::new();
        // An unresolvable leaf: full range, never a panic, never Err.
        let e = Expr::BoolLit(true);
        assert_eq!(bounds(&e, &f), Ok(FULL));
        // Affine path with no facts: full range too.
        assert_eq!(bounds(&ident("x"), &f), Ok(FULL));
    }

    #[test]
    fn contradictory_facts_are_err() {
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("x".into(), Interval::EMPTY));
        assert_eq!(bounds(&ident("x"), &f), Err(()));
        // The contradiction propagates through structural recursion (the
        // dividend of a mod, an operand of an add).
        let e = bin(BinOp::Mod, ident("x"), Expr::IntLit(10));
        assert_eq!(bounds(&e, &f), Err(()));
    }

    #[test]
    fn affine_facts_still_apply_through_expr_bounds() {
        // The affine fragment keeps its precision: x in [2, 5] ⇒ 2x - 1 in
        // [3, 9], through the same entry point.
        let mut f = FactEnv::new();
        f.assume(Fact::Bound("x".into(), Interval { lo: 2, hi: 5 }));
        let e = bin(
            BinOp::Sub,
            bin(BinOp::Mul, Expr::IntLit(2), ident("x")),
            Expr::IntLit(1),
        );
        assert_eq!(bounds(&e, &f), Ok((3, 9)));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Property tests — soundness of the implies API against brute-force
// evaluation, affine consistency, and saturation at i64 bounds
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod prop_tests {
    use super::*;
    use crate::span::Spanned;
    use proptest::prelude::*;

    /// The variable vocabulary for model-checked tests.
    const VARS: [&str; 3] = ["x", "y", "z"];
    /// Brute-force domain per variable: small enough to enumerate the full
    /// cube (9^3 = 729 assignments), large enough to exercise strict/
    /// non-strict boundaries and integer rounding.
    const DOM_LO: i64 = -4;
    const DOM_HI: i64 = 4;

    /// The leaf resolver used throughout: identifiers become single-term
    /// affines, mirroring how trackable int paths resolve in production.
    fn ident_resolver(e: &Expr) -> Option<Affine> {
        match e {
            Expr::Ident(name) => Some(Affine::term(name.clone())),
            _ => None,
        }
    }

    // ── Model facts ──────────────────────────────────────────────────────

    #[derive(Debug, Clone)]
    enum GFact {
        Bound(usize, i64, i64),
        Rel(usize, RelOp, usize),
        Ne(usize, i64),
    }

    fn arb_relop() -> impl Strategy<Value = RelOp> {
        prop_oneof![
            Just(RelOp::Lt),
            Just(RelOp::Le),
            Just(RelOp::Eq),
            Just(RelOp::Ne),
        ]
    }

    fn arb_gfact() -> impl Strategy<Value = GFact> {
        prop_oneof![
            (0..3usize, DOM_LO..=DOM_HI, DOM_LO..=DOM_HI)
                .prop_map(|(v, a, b)| GFact::Bound(v, a.min(b), a.max(b))),
            (0..3usize, arb_relop(), 0..3usize).prop_map(|(a, op, b)| GFact::Rel(a, op, b)),
            (0..3usize, DOM_LO..=DOM_HI).prop_map(|(v, c)| GFact::Ne(v, c)),
        ]
    }

    fn gfact_to_fact(f: &GFact) -> Fact {
        match f {
            GFact::Bound(v, lo, hi) => {
                Fact::Bound(VARS[*v].to_string(), Interval { lo: *lo, hi: *hi })
            }
            GFact::Rel(a, op, b) => Fact::Rel(VARS[*a].to_string(), *op, VARS[*b].to_string()),
            GFact::Ne(v, c) => Fact::NeConst(VARS[*v].to_string(), *c),
        }
    }

    fn gfact_holds(f: &GFact, asg: &[i64; 3]) -> bool {
        match f {
            GFact::Bound(v, lo, hi) => asg[*v] >= *lo && asg[*v] <= *hi,
            GFact::Rel(a, op, b) => match op {
                RelOp::Lt => asg[*a] < asg[*b],
                RelOp::Le => asg[*a] <= asg[*b],
                RelOp::Eq => asg[*a] == asg[*b],
                RelOp::Ne => asg[*a] != asg[*b],
            },
            GFact::Ne(v, c) => asg[*v] != *c,
        }
    }

    /// Does an engine `Fact` hold under a model assignment? (Used to check
    /// the facts *extracted* from a condition against the model.)
    fn fact_holds_model(f: &Fact, asg: &[i64; 3]) -> bool {
        let val = |p: &str| {
            VARS.iter()
                .position(|v| *v == p)
                .map(|i| asg[i])
                .expect("extracted fact mentions an unknown path")
        };
        match f {
            Fact::Bound(p, iv) => {
                let v = val(p);
                v >= iv.lo && v <= iv.hi
            }
            Fact::Rel(a, op, b) => {
                let (x, y) = (val(a), val(b));
                match op {
                    RelOp::Lt => x < y,
                    RelOp::Le => x <= y,
                    RelOp::Eq => x == y,
                    RelOp::Ne => x != y,
                }
            }
            Fact::NeConst(p, c) => val(p) != *c,
            // Membership facts come only from the idempotency pass, never
            // from the int-condition extraction this model checks.
            Fact::SetNotContains(..) | Fact::SetInserted(..) => {
                unreachable!("condition extraction never yields membership facts")
            }
        }
    }

    // ── Model expressions (the affine fragment) ──────────────────────────

    #[derive(Debug, Clone)]
    enum GExpr {
        Var(usize),
        Const(i64),
        Add(Box<GExpr>, Box<GExpr>),
        Sub(Box<GExpr>, Box<GExpr>),
        Mul(i64, Box<GExpr>),
        Neg(Box<GExpr>),
    }

    fn arb_gexpr() -> impl Strategy<Value = GExpr> {
        let leaf = prop_oneof![
            (0..3usize).prop_map(GExpr::Var),
            (-6..=6i64).prop_map(GExpr::Const),
        ];
        leaf.prop_recursive(3, 16, 2, |inner| {
            prop_oneof![
                (inner.clone(), inner.clone())
                    .prop_map(|(a, b)| GExpr::Add(Box::new(a), Box::new(b))),
                (inner.clone(), inner.clone())
                    .prop_map(|(a, b)| GExpr::Sub(Box::new(a), Box::new(b))),
                (-4..=4i64, inner.clone()).prop_map(|(c, e)| GExpr::Mul(c, Box::new(e))),
                inner.prop_map(|e| GExpr::Neg(Box::new(e))),
            ]
        })
    }

    fn gexpr_to_ast(e: &GExpr) -> Spanned<Expr> {
        let node = match e {
            GExpr::Var(v) => Expr::Ident(VARS[*v].to_string()),
            GExpr::Const(c) => Expr::IntLit(*c),
            GExpr::Add(a, b) => Expr::BinOp {
                op: BinOp::Add,
                lhs: Box::new(gexpr_to_ast(a)),
                rhs: Box::new(gexpr_to_ast(b)),
            },
            GExpr::Sub(a, b) => Expr::BinOp {
                op: BinOp::Sub,
                lhs: Box::new(gexpr_to_ast(a)),
                rhs: Box::new(gexpr_to_ast(b)),
            },
            GExpr::Mul(c, a) => Expr::BinOp {
                op: BinOp::Mul,
                lhs: Box::new(Spanned::dummy(Expr::IntLit(*c))),
                rhs: Box::new(gexpr_to_ast(a)),
            },
            GExpr::Neg(a) => Expr::UnaryOp {
                op: UnaryOp::Neg,
                operand: Box::new(gexpr_to_ast(a)),
            },
        };
        Spanned::dummy(node)
    }

    fn gexpr_eval(e: &GExpr, asg: &[i64; 3]) -> i128 {
        match e {
            GExpr::Var(v) => asg[*v] as i128,
            GExpr::Const(c) => *c as i128,
            GExpr::Add(a, b) => gexpr_eval(a, asg) + gexpr_eval(b, asg),
            GExpr::Sub(a, b) => gexpr_eval(a, asg) - gexpr_eval(b, asg),
            GExpr::Mul(c, a) => (*c as i128) * gexpr_eval(a, asg),
            GExpr::Neg(a) => -gexpr_eval(a, asg),
        }
    }

    // ── Model conditions ─────────────────────────────────────────────────

    #[derive(Debug, Clone)]
    enum GCond {
        Cmp(GExpr, BinOp, GExpr),
        And(Box<GCond>, Box<GCond>),
        Or(Box<GCond>, Box<GCond>),
        Not(Box<GCond>),
    }

    fn arb_cmp_op() -> impl Strategy<Value = BinOp> {
        prop_oneof![
            Just(BinOp::Lt),
            Just(BinOp::LtEq),
            Just(BinOp::Gt),
            Just(BinOp::GtEq),
            Just(BinOp::Eq),
            Just(BinOp::Neq),
        ]
    }

    fn arb_gcond() -> impl Strategy<Value = GCond> {
        let leaf = (arb_gexpr(), arb_cmp_op(), arb_gexpr())
            .prop_map(|(l, op, r)| GCond::Cmp(l, op, r));
        leaf.prop_recursive(2, 8, 2, |inner| {
            prop_oneof![
                (inner.clone(), inner.clone())
                    .prop_map(|(a, b)| GCond::And(Box::new(a), Box::new(b))),
                (inner.clone(), inner.clone())
                    .prop_map(|(a, b)| GCond::Or(Box::new(a), Box::new(b))),
                inner.prop_map(|c| GCond::Not(Box::new(c))),
            ]
        })
    }

    fn gcond_to_ast(c: &GCond) -> Spanned<Expr> {
        let node = match c {
            GCond::Cmp(l, op, r) => Expr::BinOp {
                op: *op,
                lhs: Box::new(gexpr_to_ast(l)),
                rhs: Box::new(gexpr_to_ast(r)),
            },
            GCond::And(a, b) => Expr::BinOp {
                op: BinOp::And,
                lhs: Box::new(gcond_to_ast(a)),
                rhs: Box::new(gcond_to_ast(b)),
            },
            GCond::Or(a, b) => Expr::BinOp {
                op: BinOp::Or,
                lhs: Box::new(gcond_to_ast(a)),
                rhs: Box::new(gcond_to_ast(b)),
            },
            GCond::Not(a) => Expr::UnaryOp {
                op: UnaryOp::Not,
                operand: Box::new(gcond_to_ast(a)),
            },
        };
        Spanned::dummy(node)
    }

    fn gcond_eval(c: &GCond, asg: &[i64; 3]) -> bool {
        match c {
            GCond::Cmp(l, op, r) => {
                let (lv, rv) = (gexpr_eval(l, asg), gexpr_eval(r, asg));
                match op {
                    BinOp::Lt => lv < rv,
                    BinOp::LtEq => lv <= rv,
                    BinOp::Gt => lv > rv,
                    BinOp::GtEq => lv >= rv,
                    BinOp::Eq => lv == rv,
                    BinOp::Neq => lv != rv,
                    _ => unreachable!("only comparisons are generated"),
                }
            }
            GCond::And(a, b) => gcond_eval(a, asg) && gcond_eval(b, asg),
            GCond::Or(a, b) => gcond_eval(a, asg) || gcond_eval(b, asg),
            GCond::Not(a) => !gcond_eval(a, asg),
        }
    }

    fn all_assignments() -> impl Iterator<Item = [i64; 3]> {
        (DOM_LO..=DOM_HI).flat_map(move |x| {
            (DOM_LO..=DOM_HI).flat_map(move |y| (DOM_LO..=DOM_HI).map(move |z| [x, y, z]))
        })
    }

    fn affine_eval(a: &Affine, asg: &[i64; 3]) -> i128 {
        a.k + a
            .terms
            .iter()
            .map(|(p, c)| {
                let i = VARS
                    .iter()
                    .position(|v| *v == p)
                    .expect("affine term over an unknown path");
                c * (asg[i] as i128)
            })
            .sum::<i128>()
    }

    proptest! {
        /// SOUNDNESS: `eval_condition_with` vs brute force. Any assignment
        /// in the (sub)domain satisfying every assumed fact also satisfies
        /// the full-i64 semantics of those facts, so:
        ///   Proven  ⇒ the condition holds on every satisfying assignment;
        ///   Refuted ⇒ the condition fails on every satisfying assignment.
        /// Unknown is always acceptable. A violation here is a soundness
        /// bug in the fact engine (discharge/shrinking/dominance all trust
        /// these verdicts).
        #[test]
        fn implies_sound_vs_bruteforce(
            gfacts in prop::collection::vec(arb_gfact(), 0..5),
            gcond in arb_gcond(),
        ) {
            let mut env = FactEnv::new();
            for f in &gfacts {
                env.assume(gfact_to_fact(f));
            }
            let cond = gcond_to_ast(&gcond);
            let verdict = eval_condition_with(&cond.node, &ident_resolver, &env);
            if verdict == Verdict::Unknown {
                return Ok(());
            }
            for asg in all_assignments() {
                if !gfacts.iter().all(|f| gfact_holds(f, &asg)) {
                    continue;
                }
                let actual = gcond_eval(&gcond, &asg);
                match verdict {
                    Verdict::Proven => prop_assert!(
                        actual,
                        "SOUNDNESS BUG: Proven, but condition is false at {:?}\nfacts: {:?}\ncond: {:?}",
                        asg, gfacts, gcond
                    ),
                    Verdict::Refuted => prop_assert!(
                        !actual,
                        "SOUNDNESS BUG: Refuted, but condition is true at {:?}\nfacts: {:?}\ncond: {:?}",
                        asg, gfacts, gcond
                    ),
                    Verdict::Unknown => unreachable!(),
                }
            }
        }

        /// SOUNDNESS: facts extracted from a condition must be implied by
        /// it. For every assignment where the condition is true, every
        /// then-fact must hold; where false, every else-fact must hold.
        #[test]
        fn condition_facts_sound(gcond in arb_gcond()) {
            let cond = gcond_to_ast(&gcond);
            let cf = condition_facts_with(&cond.node, &ident_resolver);
            for asg in all_assignments() {
                let truth = gcond_eval(&gcond, &asg);
                let owed = if truth { &cf.then_facts } else { &cf.else_facts };
                for f in owed {
                    prop_assert!(
                        fact_holds_model(f, &asg),
                        "SOUNDNESS BUG: extracted fact {:?} does not hold at {:?} \
                         (condition {:?} is {})",
                        f, asg, gcond, truth
                    );
                }
            }
        }

        /// Affine normalization agrees with direct evaluation: when an
        /// expression is inside the fragment, its affine form evaluates to
        /// the same value as the expression itself at every assignment.
        #[test]
        fn affine_eval_consistency(gexpr in arb_gexpr()) {
            let ast = gexpr_to_ast(&gexpr);
            let Some(aff) = to_affine_with(&ast.node, &ident_resolver) else {
                // Outside the fragment (checked-arithmetic bail) — fine.
                return Ok(());
            };
            for asg in all_assignments() {
                prop_assert_eq!(
                    affine_eval(&aff, &asg),
                    gexpr_eval(&gexpr, &asg),
                    "affine form {:?} disagrees with {:?} at {:?}",
                    aff, gexpr, asg
                );
            }
        }

        /// Substitution consistency: resolving each variable to an offset
        /// alias (`x ↦ x' + d`) commutes with evaluation — the substituted
        /// affine at `x' = v` equals the plain affine at `x = v + d`.
        #[test]
        fn affine_substitution_consistency(
            gexpr in arb_gexpr(),
            deltas in [-5..=5i64, -5..=5i64, -5..=5i64],
        ) {
            let ast = gexpr_to_ast(&gexpr);
            let subst = move |e: &Expr| -> Option<Affine> {
                match e {
                    Expr::Ident(name) => {
                        let i = VARS.iter().position(|v| v == name)?;
                        let mut a = Affine::term(name.clone());
                        a.k = deltas[i] as i128;
                        Some(a)
                    }
                    _ => None,
                }
            };
            let (Some(plain), Some(substituted)) = (
                to_affine_with(&ast.node, &ident_resolver),
                to_affine_with(&ast.node, &subst),
            ) else {
                return Ok(());
            };
            for asg in all_assignments() {
                let shifted = [
                    asg[0] + deltas[0],
                    asg[1] + deltas[1],
                    asg[2] + deltas[2],
                ];
                prop_assert_eq!(
                    affine_eval(&substituted, &asg),
                    affine_eval(&plain, &shifted),
                    "substitution does not commute for {:?} with deltas {:?} at {:?}",
                    gexpr, deltas, asg
                );
            }
        }
    }

    // ── expr_bounds soundness (masks / remainders) ───────────────────────

    /// Expression grammar for the structural bound rules: the affine
    /// fragment plus `& const` and `% const` nodes. Evaluation mirrors the
    /// runtime: two's-complement AND, truncated (srem) remainder, trap on
    /// `% 0` (modeled as `None` — no completed value, nothing owed).
    #[derive(Debug, Clone)]
    enum BExpr {
        Var(usize),
        Const(i64),
        Add(Box<BExpr>, Box<BExpr>),
        Sub(Box<BExpr>, Box<BExpr>),
        MulC(i64, Box<BExpr>),
        Neg(Box<BExpr>),
        /// `e & c` (or `c & e` when the flag is set).
        AndC(bool, i64, Box<BExpr>),
        /// `e % c`.
        ModC(Box<BExpr>, i64),
    }

    fn arb_bexpr() -> impl Strategy<Value = BExpr> {
        let leaf = prop_oneof![
            (0..3usize).prop_map(BExpr::Var),
            (-6..=6i64).prop_map(BExpr::Const),
        ];
        leaf.prop_recursive(3, 16, 2, |inner| {
            prop_oneof![
                (inner.clone(), inner.clone())
                    .prop_map(|(a, b)| BExpr::Add(Box::new(a), Box::new(b))),
                (inner.clone(), inner.clone())
                    .prop_map(|(a, b)| BExpr::Sub(Box::new(a), Box::new(b))),
                (-4..=4i64, inner.clone()).prop_map(|(c, e)| BExpr::MulC(c, Box::new(e))),
                inner.clone().prop_map(|e| BExpr::Neg(Box::new(e))),
                (any::<bool>(), -6..=6i64, inner.clone())
                    .prop_map(|(flip, c, e)| BExpr::AndC(flip, c, Box::new(e))),
                (inner, -6..=6i64).prop_map(|(e, c)| BExpr::ModC(Box::new(e), c)),
            ]
        })
    }

    fn bexpr_to_ast(e: &BExpr) -> Spanned<Expr> {
        let node = match e {
            BExpr::Var(v) => Expr::Ident(VARS[*v].to_string()),
            BExpr::Const(c) => Expr::IntLit(*c),
            BExpr::Add(a, b) => Expr::BinOp {
                op: BinOp::Add,
                lhs: Box::new(bexpr_to_ast(a)),
                rhs: Box::new(bexpr_to_ast(b)),
            },
            BExpr::Sub(a, b) => Expr::BinOp {
                op: BinOp::Sub,
                lhs: Box::new(bexpr_to_ast(a)),
                rhs: Box::new(bexpr_to_ast(b)),
            },
            BExpr::MulC(c, a) => Expr::BinOp {
                op: BinOp::Mul,
                lhs: Box::new(Spanned::dummy(Expr::IntLit(*c))),
                rhs: Box::new(bexpr_to_ast(a)),
            },
            BExpr::Neg(a) => Expr::UnaryOp {
                op: UnaryOp::Neg,
                operand: Box::new(bexpr_to_ast(a)),
            },
            BExpr::AndC(flip, c, a) => {
                let (lhs, rhs) = if *flip {
                    (Spanned::dummy(Expr::IntLit(*c)), bexpr_to_ast(a))
                } else {
                    (bexpr_to_ast(a), Spanned::dummy(Expr::IntLit(*c)))
                };
                Expr::BinOp { op: BinOp::BitAnd, lhs: Box::new(lhs), rhs: Box::new(rhs) }
            }
            BExpr::ModC(a, c) => Expr::BinOp {
                op: BinOp::Mod,
                lhs: Box::new(bexpr_to_ast(a)),
                rhs: Box::new(Spanned::dummy(Expr::IntLit(*c))),
            },
        };
        Spanned::dummy(node)
    }

    /// Runtime-faithful evaluation; `None` models a trap (`% 0`). Values in
    /// this model stay far inside i64, so i128 `&` and `%` agree exactly
    /// with the lowered i64 `band` / `srem` (Rust's `%` is truncated, like
    /// srem).
    fn bexpr_eval(e: &BExpr, asg: &[i64; 3]) -> Option<i128> {
        match e {
            BExpr::Var(v) => Some(asg[*v] as i128),
            BExpr::Const(c) => Some(*c as i128),
            BExpr::Add(a, b) => Some(bexpr_eval(a, asg)? + bexpr_eval(b, asg)?),
            BExpr::Sub(a, b) => Some(bexpr_eval(a, asg)? - bexpr_eval(b, asg)?),
            BExpr::MulC(c, a) => Some((*c as i128) * bexpr_eval(a, asg)?),
            BExpr::Neg(a) => Some(-bexpr_eval(a, asg)?),
            BExpr::AndC(_, c, a) => Some(bexpr_eval(a, asg)? & (*c as i128)),
            BExpr::ModC(a, c) => {
                let v = bexpr_eval(a, asg)?;
                if *c == 0 {
                    None // defect: modulo by zero — no completed value
                } else {
                    Some(v % (*c as i128))
                }
            }
        }
    }

    proptest! {
        /// SOUNDNESS: `expr_bounds_with` vs brute force. Every assignment
        /// satisfying the assumed facts whose evaluation completes must
        /// land inside the computed bounds; `Err(())` (contradiction) is
        /// only acceptable when NO assignment satisfies the facts.
        #[test]
        fn expr_bounds_sound_vs_bruteforce(
            gfacts in prop::collection::vec(arb_gfact(), 0..4),
            bexpr in arb_bexpr(),
        ) {
            let mut env = FactEnv::new();
            for f in &gfacts {
                env.assume(gfact_to_fact(f));
            }
            let ast = bexpr_to_ast(&bexpr);
            let no_extra = |_: &Expr| None;
            match expr_bounds_with(&ast.node, &ident_resolver, &no_extra, &env) {
                Ok((lo, hi)) => {
                    for asg in all_assignments() {
                        if !gfacts.iter().all(|f| gfact_holds(f, &asg)) {
                            continue;
                        }
                        let Some(v) = bexpr_eval(&bexpr, &asg) else { continue };
                        prop_assert!(
                            lo <= v && v <= hi,
                            "SOUNDNESS BUG: value {} outside [{}, {}] at {:?}\nfacts: {:?}\nexpr: {:?}",
                            v, lo, hi, asg, gfacts, bexpr
                        );
                    }
                }
                Err(()) => {
                    for asg in all_assignments() {
                        prop_assert!(
                            !gfacts.iter().all(|f| gfact_holds(f, &asg)),
                            "SOUNDNESS BUG: Err(contradiction) but {:?} satisfies {:?}",
                            asg, gfacts
                        );
                    }
                }
            }
        }
    }

    // ── Saturation at i64 bounds ─────────────────────────────────────────

    fn arb_extreme_const() -> impl Strategy<Value = i64> {
        prop_oneof![
            Just(i64::MIN),
            Just(i64::MIN + 1),
            Just(i64::MAX),
            Just(i64::MAX - 1),
            Just(-1i64),
            Just(0i64),
            Just(1i64),
            any::<i64>(),
        ]
    }

    fn arb_extreme_interval() -> impl Strategy<Value = Interval> {
        (arb_extreme_const(), arb_extreme_const())
            .prop_map(|(a, b)| Interval { lo: a.min(b), hi: a.max(b) })
    }

    proptest! {
        /// Interval arithmetic and condition evaluation never panic at the
        /// i64 boundaries: extreme constants, extreme interval endpoints,
        /// and coefficients that force checked-arithmetic bailouts must all
        /// degrade to Unknown / no-facts, not overflow.
        #[test]
        fn extremes_never_panic(
            iv_x in arb_extreme_interval(),
            iv_y in arb_extreme_interval(),
            c1 in arb_extreme_const(),
            c2 in arb_extreme_const(),
            m in arb_extreme_const(),
            op in arb_cmp_op(),
            ne_c in arb_extreme_const(),
        ) {
            let mut env = FactEnv::new();
            env.assume(Fact::Bound("x".to_string(), iv_x));
            env.assume(Fact::Bound("y".to_string(), iv_y));
            env.assume(Fact::NeConst("x".to_string(), ne_c));
            env.assume(Fact::Rel("x".to_string(), RelOp::Le, "y".to_string()));

            // (m * x + c1) op (y + c2) — exercises affine normalization,
            // bounding, relation tightening, and integer rounding in
            // facts_from_diff, all at the saturation edges.
            let lhs = Expr::BinOp {
                op: BinOp::Add,
                lhs: Box::new(Spanned::dummy(Expr::BinOp {
                    op: BinOp::Mul,
                    lhs: Box::new(Spanned::dummy(Expr::IntLit(m))),
                    rhs: Box::new(Spanned::dummy(Expr::Ident("x".to_string()))),
                })),
                rhs: Box::new(Spanned::dummy(Expr::IntLit(c1))),
            };
            let rhs = Expr::BinOp {
                op: BinOp::Add,
                lhs: Box::new(Spanned::dummy(Expr::Ident("y".to_string()))),
                rhs: Box::new(Spanned::dummy(Expr::IntLit(c2))),
            };
            let cond = Expr::BinOp {
                op,
                lhs: Box::new(Spanned::dummy(lhs)),
                rhs: Box::new(Spanned::dummy(rhs)),
            };

            // Must not panic; any verdict / fact set is acceptable here.
            let _ = eval_condition_with(&cond, &ident_resolver, &env);
            let _ = condition_facts_with(&cond, &ident_resolver);

            // Interval ops themselves saturate without panicking.
            let both = iv_x.intersect(&iv_y);
            let _ = both.is_empty();
            let _ = env.interval_of("x");
        }
    }
}
