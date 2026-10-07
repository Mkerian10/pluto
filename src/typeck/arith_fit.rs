//! Fact-based overflow-check elision — issue #416 phase 2.
//!
//! Phase 1 made signed int overflow a defect: codegen checks every int
//! `+`/`-`/`*` and traps on overflow. This pass deletes the checks the
//! prover can discharge: during body checking, when the live flow facts
//! bound both operands so that the mathematical result provably fits in
//! i64, the site's span key is recorded in `env.proven_fit_spans`, and
//! `lower_binop` emits the raw op instead of the checked sequence.
//!
//! # Soundness
//!
//! Elide ONLY on a proof, never a heuristic — when in doubt, check.
//!
//! - Operand values are mathematical: under trapping semantics (and under
//!   this pass's own elisions, inductively), every int expression that
//!   completes evaluates to its mathematical value, which always lies in
//!   `[i64::MIN, i64::MAX]`. An operand's interval is therefore
//!   `affine_bounds(..) ∩ [i64::MIN, i64::MAX]`, with an unbounded side
//!   clamped to the type range; a non-affine operand falls back to the
//!   structural rules in `facts::expr_bounds` (masks, remainders,
//!   byte-valued reads — same clamp, same conservatism).
//! - The result interval is exact i128 interval arithmetic over the operand
//!   intervals (`lo_a + lo_b`, endpoint products for `*`, …). The check is
//!   elided iff that interval is contained in the i64 range.
//! - Facts are consulted at statement entry, pre-kill — the same evaluation
//!   point as `requires::record_requires_discharge` and the shrinking pass.
//!   A statement whose immediate expressions contain an impure call records
//!   nothing (the call could mutate a tracked path between fact and use);
//!   closure and spawn bodies are not walked (they run later, under other
//!   facts — their sites simply stay checked).
//! - Generic functions record nothing: their bodies are checked once under
//!   skolems, and monomorphized copies carry offset spans anyway, so a
//!   lookup from codegen would miss — the sites stay checked.
//! - Loop-header statements (`while`/`for` conditions and iterables) record
//!   nothing: they re-evaluate each iteration with post-body state. Body
//!   statements are visited in flow order with their own facts, including
//!   the loop guard's facts (assumed at body entry by `check.rs`), which is
//!   what proves the `while i < N { i = i + 1 }` counter shape.
//!
//! `wrapping_add`/`wrapping_sub`/`wrapping_mul` calls are not `BinOp` nodes,
//! so they are neither counted as candidates nor checked — deliberate
//! modular arithmetic is invisible to this pass by construction.
//!
//! Shift-amount checks (issue #441) ride the same mechanism: a `<<`/`>>`
//! whose amount is outside 0..63 is a defect, so codegen range-checks every
//! non-constant amount. When the live facts bound the amount operand inside
//! `[0, 63]`, the site's span key (same `(file_id, lhs start, rhs end)`
//! shape) is recorded in `proven_shift_spans` and the check is elided;
//! `shift_check_candidates` holds every examined site, and `pluto analyze`
//! reports the residue separately from the overflow checks. Constant amounts are decided by typeck (out-of-range constants
//! are a type error) and never checked at runtime, so they are not counted
//! as candidates. Bits shifted out of the value are NOT a defect — `<<`
//! never gets an overflow check.
//!
//! The residue is surfaced: `arith_fit_candidates` minus `proven_fit_spans`
//! is the count of checks that remain at runtime, reported by
//! `pluto analyze` next to the assumption surface (DerivedInfo).

use crate::parser::ast::{BinOp, Expr, Stmt, UnaryOp};
use crate::span::Spanned;
use crate::visit::Visitor;

use super::env::TypeEnv;
use super::facts::{
    affine_bounds, contains_impure_call, expr_bounds, immediate_exprs, len_path, to_affine,
    typed_path,
};
use super::types::PlutoType;

const I64_MIN: i128 = i64::MIN as i128;
const I64_MAX: i128 = i64::MAX as i128;

/// Record overflow-check elision for every int `+`/`-`/`*` in this
/// statement's immediate expressions, against the *pre-kill* fact state.
/// Must run before `apply_stmt_kills`, right beside
/// `requires::record_requires_discharge` (same evaluation point, same kill
/// discipline).
pub(crate) fn record_arith_fit(stmt: &Stmt, env: &mut TypeEnv) {
    let Some(current_fn) = env.current_fn.clone() else {
        return;
    };
    // Generic bodies are checked once under skolem types and lowered as
    // span-offset monomorphized copies — never record (sites stay checked).
    if current_fn.contains('%') || env.generic_functions.contains_key(&current_fn) {
        return;
    }
    // Loop headers re-evaluate their condition/iterable every iteration
    // with post-body state; the facts consulted here are only valid for the
    // first evaluation. Select/scope/serve interleave with runtime
    // machinery. Skip all of them — body statements are visited in flow
    // order with their own facts.
    if matches!(
        stmt,
        Stmt::While { .. } | Stmt::For { .. } | Stmt::Select { .. } | Stmt::Scope { .. } | Stmt::Serve { .. }
    ) {
        return;
    }
    let exprs = immediate_exprs(stmt);
    if exprs.is_empty() {
        return;
    }
    // Conservative: an impure call anywhere in the statement could mutate a
    // tracked path between the fact state and an operand read.
    if exprs.iter().any(|e| contains_impure_call(e, env)) {
        return;
    }
    let mut scan = FitScan {
        env,
        candidates: Vec::new(),
        proven: Vec::new(),
        shift_candidates: Vec::new(),
        shift_proven: Vec::new(),
    };
    for e in exprs {
        scan.visit_expr(e);
    }
    let FitScan { candidates, proven, shift_candidates, shift_proven, .. } = scan;
    env.arith_fit_candidates.extend(candidates);
    env.proven_fit_spans.extend(proven);
    env.shift_check_candidates.extend(shift_candidates);
    env.proven_shift_spans.extend(shift_proven);
}

struct FitScan<'a> {
    env: &'a TypeEnv,
    candidates: Vec<(u32, usize, usize)>,
    proven: Vec<(u32, usize, usize)>,
    shift_candidates: Vec<(u32, usize, usize)>,
    shift_proven: Vec<(u32, usize, usize)>,
}

impl<'a> Visitor for FitScan<'a> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        match &expr.node {
            // Closure and spawn bodies run later, under other facts — their
            // sites stay checked, and we do not descend.
            Expr::Closure { .. } | Expr::Spawn { .. } => {}
            Expr::BinOp { op: op @ (BinOp::Add | BinOp::Sub | BinOp::Mul), lhs, rhs } => {
                // Record candidates for int sites only; non-int operands
                // (float/string/byte) are outside the affine fragment and
                // never prove fit, and codegen consults the set only from
                // the int arms — but the candidate COUNT should not include
                // float/string arithmetic, so gate on an int-shaped lhs.
                if self.is_int_operand(&lhs.node) {
                    let key = (lhs.span.file_id, lhs.span.start, rhs.span.end);
                    self.candidates.push(key);
                    if self.result_fits(op, &lhs.node, &rhs.node) {
                        self.proven.push(key);
                    }
                }
                crate::visit::walk_expr(self, expr);
            }
            Expr::BinOp { op: BinOp::Shl | BinOp::Shr, lhs, rhs } => {
                // Constant amounts are typeck-decided and never checked.
                if const_shift_amount(&rhs.node).is_none() && self.is_int_operand(&rhs.node) {
                    let key = (lhs.span.file_id, lhs.span.start, rhs.span.end);
                    self.shift_candidates.push(key);
                    if self.shift_amount_in_range(&rhs.node) {
                        self.shift_proven.push(key);
                    }
                }
                crate::visit::walk_expr(self, expr);
            }
            _ => crate::visit::walk_expr(self, expr),
        }
    }
}

impl<'a> FitScan<'a> {
    /// Cheap int-shape test for the candidate count. Conservative: an
    /// operand this cannot classify (e.g. a call result) makes the site
    /// uncounted AND unproven — soundness does not depend on it, since
    /// codegen consults the proven set only from the int arms.
    fn is_int_operand(&self, e: &Expr) -> bool {
        match e {
            Expr::IntLit(_) => true,
            Expr::UnaryOp { op: UnaryOp::Neg | UnaryOp::BitNot, operand } => {
                self.is_int_operand(&operand.node)
            }
            Expr::BinOp {
                op:
                    BinOp::Add
                    | BinOp::Sub
                    | BinOp::Mul
                    | BinOp::Div
                    | BinOp::Mod
                    | BinOp::Shl
                    | BinOp::Shr
                    | BinOp::BitAnd
                    | BinOp::BitOr
                    | BinOp::BitXor,
                lhs,
                ..
            } => self.is_int_operand(&lhs.node),
            // A `.to_int()` conversion is an int operand whatever its source
            // — byte/bool widenings participate (and byte sources carry
            // their [0, 255] bounds through `expr_bounds`).
            Expr::MethodCall { method, args, .. } if method.node == "to_int" && args.is_empty() => {
                true
            }
            _ => {
                matches!(typed_path(e, self.env), Some((_, PlutoType::Int)))
                    || len_path(e, self.env).is_some()
            }
        }
    }

    /// Interval of an int operand under the current facts, clamped to the
    /// i64 range (a completing int expression always evaluates to its
    /// mathematical value within i64 — trapping semantics).
    fn operand_interval(&self, e: &Expr) -> Result<(i128, i128), ()> {
        let Some(a) = to_affine(e, self.env) else {
            // Outside the affine fragment: structural interval rules —
            // masks (`x & c`), remainders (`x % c`), byte-valued reads
            // widened to int (facts.rs `expr_bounds`). Already clamped to
            // the i64 range.
            return expr_bounds(e, self.env);
        };
        let (lo, hi) = affine_bounds(&a, &self.env.facts)?;
        let mut lo = lo.unwrap_or(I64_MIN).max(I64_MIN);
        let mut hi = hi.unwrap_or(I64_MAX).min(I64_MAX);
        // Strict-relation refinement for the plain `p + k` shape: a live
        // `p < q` fact bounds p <= i64::MAX - 1 even when q has no
        // interval, because q is itself an i64 value (dually for `q < p`).
        // This is what proves the variable-bounded counter
        // `while i < n { i = i + 1 }` in range.
        if a.terms.len() == 1 {
            let (path, &c) = a.terms.iter().next().expect("one term");
            if c == 1 {
                if self.env.facts.has_strict_upper(path) {
                    hi = hi.min(I64_MAX - 1 + a.k).max(I64_MIN);
                }
                if self.env.facts.has_strict_lower(path) {
                    lo = lo.max(I64_MIN + 1 + a.k).min(I64_MAX);
                }
            }
        }
        if lo > hi {
            // Contradictory bounds: unreachable code — answer "no proof".
            return Err(());
        }
        Ok((lo, hi))
    }

    /// Do the live facts prove a shift amount lies in `[0, 63]`?
    fn shift_amount_in_range(&self, amount: &Expr) -> bool {
        match self.operand_interval(amount) {
            Ok((lo, hi)) => 0 <= lo && hi <= 63,
            Err(()) => false,
        }
    }

    fn result_fits(&self, op: &BinOp, lhs: &Expr, rhs: &Expr) -> bool {
        let (Ok((la, ha)), Ok((lb, hb))) = (self.operand_interval(lhs), self.operand_interval(rhs))
        else {
            return false;
        };
        let (lo, hi) = match op {
            BinOp::Add => (la + lb, ha + hb),
            BinOp::Sub => (la - hb, ha - lb),
            BinOp::Mul => {
                let products = [la * lb, la * hb, ha * lb, ha * hb];
                (
                    *products.iter().min().expect("non-empty"),
                    *products.iter().max().expect("non-empty"),
                )
            }
            _ => return false,
        };
        I64_MIN <= lo && hi <= I64_MAX
    }
}

/// The value of a constant shift amount: an integer literal, or unary minus
/// applied to one (possibly nested). Anything else is non-constant and is
/// range-checked at runtime. Shared with codegen, which skips the runtime
/// check for these (typeck has already rejected out-of-range constants).
pub(crate) fn const_shift_amount(e: &Expr) -> Option<i128> {
    match e {
        Expr::IntLit(n) => Some(*n as i128),
        Expr::UnaryOp { op: UnaryOp::Neg, operand } => {
            const_shift_amount(&operand.node).map(|n| -n)
        }
        _ => None,
    }
}
