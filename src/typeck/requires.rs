//! Static `requires` discharge at call sites — the first slice of
//! contracts.md phase 6 (docs/design/rfc-verification.md, "the proof
//! ladder").
//!
//! A `requires` clause is a caller obligation that today is enforced by a
//! runtime check in the callee prologue. This pass makes the obligation
//! *provable*: at a direct call site where the caller's live flow facts
//! prove every requires clause of the callee (with actual arguments and the
//! receiver substituted in), the site is recorded in
//! `env.proven_requires_sites`. Codegen then routes the call to the
//! callee's *unchecked twin* (`<name>$nochk` — the same body lowered
//! without the entry checks), eliding the runtime check for that site only.
//! Unproven sites, calls through fn-typed values and trait objects, spawn,
//! serve/RPC entries, and `at` placement keep the checked symbol — runtime
//! behavior there is exactly as before.
//!
//! # Scope (conservative by construction — mirrors shrink.rs slice 1)
//!
//! - **Direct calls only.** Named non-generic free functions, and method
//!   calls whose receiver is a trackable path of class (or object) type.
//!   Calls through fn-typed values, closures, trait objects, and `at`
//!   placement are never recorded.
//! - **Generic callees never elide.** Their call sites are rewritten to
//!   instance manglings at monomorphization and their requires would need
//!   per-instance twins; the entry check stays. Generic *callers* never
//!   record (their bodies are checked once under skolem types).
//! - **Facts must be live at the call.** Evaluation shares shrink.rs's
//!   kill discipline: within a statement, calls are visited in evaluation
//!   order and field facts are dropped once any call-like node has run;
//!   between statements the ordinary `apply_stmt_kills` rules apply. Loop
//!   headers (`while` conditions, `for` iterables) are skipped entirely —
//!   they re-evaluate every iteration with post-body state, but the facts
//!   consulted here are the pre-loop ones. Select/scope/serve statements
//!   are likewise skipped (their evaluation interleaves with runtime
//!   machinery).
//! - **Entity receivers prove parameter clauses only.** Entities mutate
//!   concurrently, so `self.field` clauses stay Unknown for them; clauses
//!   over by-value parameters still prove.
//! - **All clauses or nothing.** A site is recorded only when *every*
//!   requires clause (the method's own plus any trait-propagated ones —
//!   the same set codegen emits as runtime checks) evaluates to Proven.
//!   Codegen additionally gates twin creation on the typeck clause count
//!   matching the emitted check count, so a divergence between the two
//!   collections can only suppress elision, never skip an unproven check.
//!
//! Requires violations are hard aborts (`__pluto_requires_violation` +
//! trap), not typed errors — so a proven site changes no error set; the
//! only observable effect of discharge is that the entry check is not
//! executed for that call.

use std::collections::HashMap;

use crate::parser::ast::{ContractKind, Expr, Function, Program, Stmt};
use crate::span::Spanned;
use crate::visit::{walk_expr, Visitor};

use super::env::{mangle_method, TypeEnv};
use super::facts::{
    contains_call, eval_condition_with, immediate_exprs, len_path, to_affine, typed_path, Affine,
    FactEnv, Verdict,
};
use super::types::PlutoType;

// ─────────────────────────────────────────────────────────────────────────────
// Summaries (syntactic pre-pass)
// ─────────────────────────────────────────────────────────────────────────────

/// One `requires` clause with the parameter vocabulary it is written in
/// (trait-propagated clauses use the trait method's parameter names, which
/// may differ from the implementation's).
#[derive(Debug, Clone)]
pub struct ReqClause {
    pub expr: Expr,
    /// Declaring function's parameter names, in order, including `self` for
    /// methods.
    pub params: Vec<String>,
}

/// The full set of requires clauses a callee's entry check enforces: the
/// trait-propagated clauses (checked first — mirrors codegen's merge order)
/// followed by the function's own.
#[derive(Debug, Clone, Default)]
pub struct RequiresSummary {
    pub clauses: Vec<ReqClause>,
}

/// Build requires summaries for every non-generic free function and every
/// method of a non-generic class/object. App and stage methods are skipped
/// (their calls route through DI/RPC paths that never elide). Purely
/// syntactic; runs before body checking so summaries exist for callers
/// visited in any order.
pub(crate) fn extract_requires_summaries(program: &Program, env: &mut TypeEnv) {
    let mut out: HashMap<String, RequiresSummary> = HashMap::new();
    for func in &program.functions {
        if !func.node.type_params.is_empty() {
            continue;
        }
        let clauses = own_clauses(&func.node);
        if !clauses.is_empty() {
            out.insert(func.node.name.node.clone(), RequiresSummary { clauses });
        }
    }
    for class in &program.classes {
        let c = &class.node;
        if !c.type_params.is_empty() {
            continue;
        }
        for method in &c.methods {
            // Trait-propagated requires first (codegen checks them first),
            // then the method's own. Liskov checking forbids impls adding
            // requires to trait methods, so in practice a method has one
            // source or the other; keeping both matches codegen's merge.
            let mut clauses = trait_clauses(program, c, &method.node.name.node);
            clauses.extend(own_clauses(&method.node));
            if !clauses.is_empty() {
                out.insert(
                    mangle_method(&c.name.node, &method.node.name.node),
                    RequiresSummary { clauses },
                );
            }
        }
    }
    env.requires_summaries = out;
}

fn own_clauses(func: &Function) -> Vec<ReqClause> {
    let params: Vec<String> = func.params.iter().map(|p| p.name.node.clone()).collect();
    func.contracts
        .iter()
        .filter(|c| c.node.kind == ContractKind::Requires)
        .map(|c| ReqClause {
            expr: c.node.expr.node.clone(),
            params: params.clone(),
        })
        .collect()
}

/// Requires clauses a class method inherits from the traits the class
/// implements (same AST source codegen's trait-contract propagation reads).
fn trait_clauses(
    program: &Program,
    class: &crate::parser::ast::ClassDecl,
    method_name: &str,
) -> Vec<ReqClause> {
    let mut out = Vec::new();
    for trait_ref in &class.impl_traits {
        for trait_decl in &program.traits {
            if trait_decl.node.name.node != trait_ref.name.node {
                continue;
            }
            for tm in &trait_decl.node.methods {
                if tm.name.node != method_name {
                    continue;
                }
                let params: Vec<String> = tm.params.iter().map(|p| p.name.node.clone()).collect();
                for c in &tm.contracts {
                    if c.node.kind == ContractKind::Requires {
                        out.push(ReqClause {
                            expr: c.node.expr.node.clone(),
                            params: params.clone(),
                        });
                    }
                }
            }
        }
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Call-site evaluation (during body checking)
// ─────────────────────────────────────────────────────────────────────────────

/// Evaluate requires discharge for every direct call in this statement's
/// immediate expressions, against the *pre-kill* fact state. Must run before
/// `apply_stmt_kills`, right beside `shrink::record_call_site_shrinking`
/// (same evaluation point, same kill discipline).
pub(crate) fn record_requires_discharge(stmt: &Stmt, env: &mut TypeEnv) {
    if env.requires_summaries.is_empty() {
        return;
    }
    let Some(current_fn) = env.current_fn.clone() else {
        return;
    };
    // Generic callers never record: their bodies are checked once under
    // skolem types, while the proof would have to hold per instantiation.
    if current_fn.contains('%') || env.generic_functions.contains_key(&current_fn) {
        return;
    }
    // Loop headers re-evaluate their condition/iterable every iteration with
    // post-body state; the facts consulted here are only valid for the first
    // evaluation. Select/scope/serve interleave with runtime machinery.
    // Skip all of them — body statements are visited in flow order with
    // their own (havocked) facts.
    if matches!(
        stmt,
        Stmt::While { .. } | Stmt::For { .. } | Stmt::Select { .. } | Stmt::Scope { .. } | Stmt::Serve { .. }
    ) {
        return;
    }
    let mut scan = ReqScan {
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
    for (key, callee) in records {
        env.proven_requires_sites.insert(key, callee);
    }
}

struct ReqScan<'a> {
    env: &'a TypeEnv,
    current_fn: &'a str,
    /// A call-like node already completed within this statement: field facts
    /// are no longer trustworthy for subsequent calls.
    calls_before: bool,
    /// Lazily built copy of the fact state with field facts dropped.
    killed_facts: Option<FactEnv>,
    records: Vec<((String, usize), String)>,
}

impl ReqScan<'_> {
    fn try_prove(
        &mut self,
        callee: &str,
        span_start: usize,
        args: &[Spanned<Expr>],
        receiver: Option<(&str, bool)>,
    ) {
        let Some(summary) = self.env.requires_summaries.get(callee) else {
            return;
        };
        if summary.clauses.is_empty() {
            return;
        }
        let clauses = summary.clauses.clone();
        // Positional views of the actuals (clauses map their own parameter
        // names onto these positions — trait clauses may use different
        // names than the implementation).
        let arg_affs: Vec<Option<Affine>> =
            args.iter().map(|a| to_affine(&a.node, self.env)).collect();
        // Trackable-path views for substituting the callee's `p.field` /
        // `p.len()` clause leaves: the callee's `p` aliases the actual, so
        // at call time the callee's entry `p.field` equals the caller's
        // `a.field`. Field substitution needs a non-entity class-typed
        // actual; `len()` substitution a collection-typed actual.
        let mut arg_field_base: Vec<Option<String>> = vec![None; args.len()];
        let mut arg_len_base: Vec<Option<String>> = vec![None; args.len()];
        for (i, a) in args.iter().enumerate() {
            if let Some((apath, aty)) = typed_path(&a.node, self.env) {
                match aty {
                    PlutoType::Class(c)
                        if !self.env.object_types.contains(&c)
                            && !self.env.remote_types.contains(&c)
                            && !self.env.domain_types.contains(&c) =>
                    {
                        arg_field_base[i] = Some(apath);
                    }
                    PlutoType::Array(_)
                    | PlutoType::String
                    | PlutoType::Bytes
                    | PlutoType::Map(_, _)
                    | PlutoType::Set(_) => {
                        arg_len_base[i] = Some(apath);
                    }
                    _ => {}
                }
            }
        }
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
        for clause in &clauses {
            let formals: Vec<&String> =
                clause.params.iter().filter(|p| p.as_str() != "self").collect();
            if formals.len() != args.len() {
                return;
            }
            let index_of = |name: &str| formals.iter().position(|p| p.as_str() == name);
            let resolve = |e: &Expr| -> Option<Affine> {
                match e {
                    Expr::Ident(name) => index_of(name).and_then(|i| arg_affs[i].clone()),
                    Expr::FieldAccess { object, field } => match &object.node {
                        Expr::Ident(s) if s == "self" => match receiver {
                            Some((rpath, true)) => {
                                Some(Affine::term(format!("{rpath}.{}", field.node)))
                            }
                            _ => None,
                        },
                        // One-level field path on a parameter: `p.field`
                        // reads the actual's field in the caller's
                        // vocabulary.
                        Expr::Ident(p) => index_of(p)
                            .and_then(|i| arg_field_base[i].as_ref())
                            .map(|b| Affine::term(format!("{b}.{}", field.node))),
                        _ => None,
                    },
                    // `p.len()` on a collection parameter: the actual's
                    // length term (carries the automatic >= 0 bound).
                    Expr::MethodCall { object, method, args: margs, .. }
                        if method.node == "len" && margs.is_empty() =>
                    {
                        match &object.node {
                            Expr::Ident(p) if p != "self" => index_of(p)
                                .and_then(|i| arg_len_base[i].as_ref())
                                .map(|b| Affine::term(format!("{b}.len()"))),
                            _ => None,
                        }
                    }
                    _ => None,
                }
            };
            if eval_condition_with(&clause.expr, &resolve, facts) != Verdict::Proven {
                return;
            }
        }
        self.records.push((
            (self.current_fn.to_string(), span_start),
            callee.to_string(),
        ));
    }
}

impl Visitor for ReqScan<'_> {
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
                // Handler bodies run conditionally, after the call.
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
                // dynamic targets never elide.
                if self.env.lookup(&name.node).is_none() {
                    self.try_prove(&name.node, name.span.start, args, None);
                }
                self.calls_before = true;
            }
            Expr::MethodCall { object, method, args, .. } => {
                // Builtin `len()` on a collection-typed path is a pure read:
                // it neither proves nor invalidates field facts.
                if len_path(&expr.node, self.env).is_some() {
                    return;
                }
                if let Some((rpath, PlutoType::Class(cname))) = typed_path(&object.node, self.env)
                {
                    // Entities are shared and mutate concurrently: their
                    // field facts are never tracked, so `self.field`
                    // clauses stay Unknown. Parameter clauses still prove.
                    let fields_ok = !self.env.object_types.contains(&cname)
                        && !self.env.remote_types.contains(&cname)
                        && !self.env.domain_types.contains(&cname);
                    let callee = mangle_method(&cname, &method.node);
                    self.try_prove(
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
// Unit tests — proof decisions recorded in proven_requires_sites
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::env::TypeEnv;
    use crate::diagnostics::CompileError;

    fn check(src: &str) -> Result<TypeEnv, CompileError> {
        let tokens = crate::lexer::lex(src).unwrap();
        let mut parser = crate::parser::Parser::new(&tokens, src);
        let mut program = parser.parse_program().unwrap();
        crate::modules::resolve_qualified_access_single_file(&mut program)?;
        crate::typeck::type_check(&program).map(|(env, _warnings)| env)
    }

    fn proven_callees(env: &TypeEnv) -> Vec<&str> {
        env.proven_requires_sites
            .values()
            .map(|s| s.as_str())
            .collect()
    }

    #[test]
    fn guarded_free_fn_site_proves() {
        let env = check(
            "fn positive(x: int) int\n    requires x > 0\n{\n    return x * 2\n}\n\n\
             fn main() {\n    let a = 5\n    if a > 0 {\n        print(positive(a))\n    }\n}",
        )
        .unwrap();
        assert_eq!(proven_callees(&env), vec!["positive"]);
    }

    #[test]
    fn literal_argument_proves() {
        let env = check(
            "fn positive(x: int) int\n    requires x > 0\n{\n    return x * 2\n}\n\n\
             fn main() {\n    print(positive(3))\n}",
        )
        .unwrap();
        assert_eq!(proven_callees(&env), vec!["positive"]);
    }

    #[test]
    fn unguarded_site_not_recorded() {
        let env = check(
            "fn positive(x: int) int\n    requires x > 0\n{\n    return x * 2\n}\n\n\
             fn main(a: int) {\n    print(positive(a))\n}",
        )
        .unwrap();
        assert!(env.proven_requires_sites.is_empty());
    }

    #[test]
    fn partial_conjunct_proof_not_recorded() {
        // The guard proves `x > 0` but not `x < 100`: all-or-nothing.
        let env = check(
            "fn bounded(x: int) int\n    requires x > 0\n    requires x < 100\n{\n    return x\n}\n\n\
             fn main(a: int) {\n    if a > 0 {\n        print(bounded(a))\n    }\n}",
        )
        .unwrap();
        assert!(env.proven_requires_sites.is_empty());
    }

    #[test]
    fn multi_conjunct_full_proof_recorded() {
        let env = check(
            "fn bounded(x: int) int\n    requires x > 0\n    requires x < 100\n{\n    return x\n}\n\n\
             fn main(a: int) {\n    if a > 0 {\n        if a < 50 {\n            print(bounded(a))\n        }\n    }\n}",
        )
        .unwrap();
        assert_eq!(proven_callees(&env), vec!["bounded"]);
    }

    const ACCOUNT: &str = "class Account {\n    balance: int\n\n    \
        fn withdraw(mut self, amt: int) int\n        requires self.balance >= amt\n    {\n        \
        self.balance = self.balance - amt\n        return self.balance\n    }\n}\n\n";

    #[test]
    fn method_receiver_field_clause_proves() {
        let env = check(&format!(
            "{ACCOUNT}fn main() {{\n    let mut a = Account {{ balance: 100 }}\n    \
             if a.balance >= 10 {{\n        print(a.withdraw(10))\n    }}\n}}"
        ))
        .unwrap();
        assert_eq!(proven_callees(&env), vec!["Account$withdraw"]);
    }

    #[test]
    fn interleaved_call_kills_receiver_field_facts() {
        // noise() takes an Account, so it may mutate `a` through an alias:
        // field facts die, the runtime check is retained. (A reach-free
        // interleaved call — no class-reaching params — would no longer
        // kill: the purity-aware severing rule, facts::call_severity.)
        let env = check(&format!(
            "{ACCOUNT}fn noise(acct: Account) int {{\n    return acct.balance\n}}\n\n\
             fn main() {{\n    let mut a = Account {{ balance: 100 }}\n    \
             let mut b = Account {{ balance: 50 }}\n    \
             if a.balance >= 10 {{\n        noise(b)\n        print(a.withdraw(10))\n    }}\n}}"
        ))
        .unwrap();
        assert!(env.proven_requires_sites.is_empty());
    }

    #[test]
    fn param_field_clause_proves_from_actual_path() {
        let env = check(&format!(
            "{ACCOUNT}fn take(mut acc: Account, amt: int) \n    requires acc.balance >= amt\n{{\n    \
             acc.balance = acc.balance - amt\n}}\n\n\
             fn main() {{\n    let mut a = Account {{ balance: 100 }}\n    \
             if a.balance >= 5 {{\n        take(a, 5)\n    }}\n}}"
        ))
        .unwrap();
        assert_eq!(proven_callees(&env), vec!["take"]);
    }

    #[test]
    fn len_clause_proves_from_actual_length() {
        let env = check(
            "fn first(xs: [int]) int\n    requires xs.len() > 0\n{\n    return xs[0]\n}\n\n\
             fn main() {\n    let a = [1, 2, 3]\n    if a.len() > 0 {\n        print(first(a))\n    }\n}",
        )
        .unwrap();
        assert_eq!(proven_callees(&env), vec!["first"]);
    }

    #[test]
    fn generic_callee_never_recorded() {
        let env = check(
            "fn pick<T>(v: T, n: int) T\n    requires n > 0\n{\n    return v\n}\n\n\
             fn main() {\n    print(pick(7, 3))\n}",
        )
        .unwrap();
        assert!(env.proven_requires_sites.is_empty());
    }

    #[test]
    fn generic_caller_never_records() {
        let env = check(
            "fn positive(x: int) int\n    requires x > 0\n{\n    return x * 2\n}\n\n\
             fn go<T>(v: T) T {\n    print(positive(3))\n    return v\n}\n\n\
             fn main() {\n    print(go(1))\n}",
        )
        .unwrap();
        assert!(env.proven_requires_sites.is_empty());
    }

    #[test]
    fn loop_header_call_never_recorded() {
        // The while condition re-evaluates every iteration; pre-loop facts
        // only cover the first. Conservatively skipped.
        let env = check(
            "fn positive(x: int) bool\n    requires x > 0\n{\n    return x > 1\n}\n\n\
             fn main() {\n    let a = 5\n    if a > 0 {\n        while positive(a) {\n            break\n        }\n    }\n}",
        )
        .unwrap();
        assert!(env.proven_requires_sites.is_empty());
    }

    #[test]
    fn entity_param_clause_proves_self_field_clause_does_not() {
        // Entities mutate concurrently: `self.field` clauses stay Unknown,
        // parameter clauses still prove.
        let env = check(
            "object Counter {\n    value: int\n\n    \
             fn bump(mut self, by: int) int\n        requires by > 0\n    {\n        \
             self.value = self.value + by\n        return self.value\n    }\n\n    \
             fn drain(mut self, amt: int) int\n        requires self.value >= amt\n    {\n        \
             self.value = self.value - amt\n        return self.value\n    }\n}\n\n\
             fn main() {\n    let mut c = Counter { value: 10 }\n    print(c.bump(2))\n    \
             print(c.drain(1))\n}",
        )
        .unwrap();
        let callees = proven_callees(&env);
        assert!(callees.contains(&"Counter$bump"));
        assert!(!callees.contains(&"Counter$drain"));
    }

    #[test]
    fn trait_requires_covered_by_summary() {
        // The trait clause is part of the callee's entry check; the site
        // only proves when the caller's facts prove it.
        let src_guarded = "trait Sink {\n    fn put(mut self, n: int)\n        requires n > 0\n}\n\n\
             class Box impl Sink {\n    total: int\n\n    \
             fn put(mut self, n: int) {\n        self.total = self.total + n\n    }\n}\n\n\
             fn main() {\n    let mut b = Box { total: 0 }\n    b.put(5)\n}";
        let env = check(src_guarded).unwrap();
        assert_eq!(proven_callees(&env), vec!["Box$put"]);

        let src_unguarded = "trait Sink {\n    fn put(mut self, n: int)\n        requires n > 0\n}\n\n\
             class Box impl Sink {\n    total: int\n\n    \
             fn put(mut self, n: int) {\n        self.total = self.total + n\n    }\n}\n\n\
             fn main(k: int) {\n    let mut b = Box { total: 0 }\n    b.put(k)\n}";
        let env = check(src_unguarded).unwrap();
        assert!(env.proven_requires_sites.is_empty());
    }

    #[test]
    fn call_through_fn_value_never_recorded() {
        let env = check(
            "fn positive(x: int) int\n    requires x > 0\n{\n    return x * 2\n}\n\n\
             fn main() {\n    let f = (x: int) => x + 1\n    print(f(3))\n}",
        )
        .unwrap();
        assert!(env.proven_requires_sites.is_empty());
    }
}
