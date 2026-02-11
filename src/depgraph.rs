//! Dependency graph between declarations, built post-hoc from AST cross-references
//! and TypeEnv data. Used to answer "what's affected?" when a declaration changes.

use std::collections::{HashMap, HashSet, VecDeque};

use uuid::Uuid;

use crate::parser::ast::{Block, Expr, Program, Stmt, StringInterpPart};
use crate::typeck::env::{mangle_method, MethodResolution, TypeEnv};

/// The kind of dependency edge between two declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DepEdge {
    /// Function A calls function B.
    Calls,
    /// Function A constructs class C.
    Constructs,
    /// Declaration A uses type B in its signature (param/return type).
    UsesType,
    /// Method M belongs to class C.
    MethodOf,
    /// Function A raises error E.
    RaisesError,
    /// Class C implements trait T.
    ImplementsTrait,
    /// Class C injects class D via DI.
    Injects,
    /// Conservative fallback dependency.
    DependsOn,
}

impl DepEdge {
    /// Whether this edge represents an API-level dependency (signature/contract changes
    /// propagate through these edges). Non-API edges (`MethodOf`, `DependsOn`) only
    /// propagate implementation-level changes.
    pub fn is_api_dep(&self) -> bool {
        matches!(
            self,
            DepEdge::Calls
                | DepEdge::Constructs
                | DepEdge::UsesType
                | DepEdge::RaisesError
                | DepEdge::ImplementsTrait
                | DepEdge::Injects
        )
    }
}

/// How a declaration changed — determines propagation behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// Only the implementation changed (body, not signature).
    ImplOnly,
    /// Both API (signature) and implementation changed.
    ApiAndImpl,
}

/// A dependency graph between declarations, keyed by UUID.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DependencyGraph {
    /// Forward edges: declaration UUID → [(target UUID, edge kind)].
    pub deps: HashMap<Uuid, Vec<(Uuid, DepEdge)>>,
    /// Reverse edges: target UUID → [(dependent UUID, edge kind)].
    pub rdeps: HashMap<Uuid, Vec<(Uuid, DepEdge)>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a directed dependency edge from `from` to `to`. Deduplicates.
    pub fn add_edge(&mut self, from: Uuid, to: Uuid, kind: DepEdge) {
        let entry = (to, kind);
        let fwd = self.deps.entry(from).or_default();
        if !fwd.contains(&entry) {
            fwd.push(entry);
        }
        let rev_entry = (from, kind);
        let rev = self.rdeps.entry(to).or_default();
        if !rev.contains(&rev_entry) {
            rev.push(rev_entry);
        }
    }

    /// Get forward dependencies of a declaration.
    pub fn deps_of(&self, id: Uuid) -> &[(Uuid, DepEdge)] {
        self.deps.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get reverse dependencies (dependents) of a declaration.
    pub fn rdeps_of(&self, id: Uuid) -> &[(Uuid, DepEdge)] {
        self.rdeps.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Compute the set of declarations affected by a set of changes.
    ///
    /// BFS from the changed set through reverse edges. If a declaration changed
    /// `ImplOnly`, only propagate through non-API edges (`MethodOf`, `DependsOn`).
    /// `ApiAndImpl` changes propagate through all edges.
    pub fn affected(&self, changed: &HashMap<Uuid, ChangeKind>) -> HashSet<Uuid> {
        let mut affected = HashSet::new();
        let mut queue: VecDeque<(Uuid, ChangeKind)> = VecDeque::new();

        // Seed with all changed declarations
        for (&id, &kind) in changed {
            affected.insert(id);
            queue.push_back((id, kind));
        }

        while let Some((id, change_kind)) = queue.pop_front() {
            // Walk reverse edges: who depends on `id`?
            for &(dependent, edge_kind) in self.rdeps_of(id) {
                // ImplOnly changes only propagate through non-API edges
                if change_kind == ChangeKind::ImplOnly && edge_kind.is_api_dep() {
                    continue;
                }
                if affected.insert(dependent) {
                    // Dependents that are affected via API edges propagate as ApiAndImpl
                    // (their behavior may have changed due to the API change in their dep).
                    // Dependents affected via non-API edges propagate as ImplOnly.
                    let propagated_kind = if edge_kind.is_api_dep() {
                        ChangeKind::ApiAndImpl
                    } else {
                        ChangeKind::ImplOnly
                    };
                    queue.push_back((dependent, propagated_kind));
                }
            }
        }

        affected
    }
}

/// Index for looking up declaration UUIDs by name. Mirrors the pattern from `xref::DeclIndex`.
struct NameIndex {
    fn_index: HashMap<String, Uuid>,
    class_index: HashMap<String, Uuid>,
    error_index: HashMap<String, Uuid>,
    trait_index: HashMap<String, Uuid>,
}

impl NameIndex {
    fn build(program: &Program) -> Self {
        let mut fn_index = HashMap::new();
        let mut class_index = HashMap::new();
        let mut error_index = HashMap::new();
        let mut trait_index = HashMap::new();

        for f in &program.functions {
            fn_index.insert(f.node.name.node.clone(), f.node.id);
        }

        for c in &program.classes {
            class_index.insert(c.node.name.node.clone(), c.node.id);
            for m in &c.node.methods {
                let mangled = mangle_method(&c.node.name.node, &m.node.name.node);
                fn_index.insert(mangled, m.node.id);
            }
        }

        for e in &program.errors {
            error_index.insert(e.node.name.node.clone(), e.node.id);
        }

        for t in &program.traits {
            trait_index.insert(t.node.name.node.clone(), t.node.id);
        }

        if let Some(app) = &program.app {
            for m in &app.node.methods {
                let mangled = mangle_method(&app.node.name.node, &m.node.name.node);
                fn_index.insert(mangled, m.node.id);
            }
        }

        for stage in &program.stages {
            for m in &stage.node.methods {
                let mangled = mangle_method(&stage.node.name.node, &m.node.name.node);
                fn_index.insert(mangled, m.node.id);
            }
        }

        Self { fn_index, class_index, error_index, trait_index }
    }
}

/// Build the full dependency graph from a type-checked program.
///
/// This walks the post-type-check, post-xref AST and TypeEnv to extract all
/// dependency edges between declarations.
pub fn build_dep_graph(program: &Program, env: &TypeEnv) -> DependencyGraph {
    let mut graph = DependencyGraph::new();
    let index = NameIndex::build(program);

    // 1. Walk function bodies for call/construct/enum edges
    for f in &program.functions {
        walk_block_for_edges(&f.node.body.node, f.node.id, &mut graph);
    }

    // 2. Walk class methods + invariants
    for c in &program.classes {
        for m in &c.node.methods {
            walk_block_for_edges(&m.node.body.node, m.node.id, &mut graph);

            // 5. MethodOf: method belongs to class
            graph.add_edge(m.node.id, c.node.id, DepEdge::MethodOf);
        }
        for inv in &c.node.invariants {
            walk_expr_for_edges(&inv.node.expr.node, c.node.id, &mut graph);
        }
    }

    // Walk app methods
    if let Some(app) = &program.app {
        for m in &app.node.methods {
            walk_block_for_edges(&m.node.body.node, m.node.id, &mut graph);
        }
    }

    // Walk stage methods
    for stage in &program.stages {
        for m in &stage.node.methods {
            walk_block_for_edges(&m.node.body.node, m.node.id, &mut graph);
        }
    }

    // 3. Method call edges via env.method_resolutions
    for (&(ref fn_name, _span_start), resolution) in &env.method_resolutions {
        if let MethodResolution::Class { mangled_name } = resolution {
            if let (Some(&caller_id), Some(&target_id)) =
                (index.fn_index.get(fn_name), index.fn_index.get(mangled_name))
            {
                graph.add_edge(caller_id, target_id, DepEdge::Calls);
            }
        }
    }

    // 6. RaisesError edges via env.fn_errors
    for (fn_name, error_names) in &env.fn_errors {
        if let Some(&fn_id) = index.fn_index.get(fn_name) {
            for err_name in error_names {
                if let Some(&err_id) = index.error_index.get(err_name) {
                    graph.add_edge(fn_id, err_id, DepEdge::RaisesError);
                }
            }
        }
    }

    // 7. ImplementsTrait edges via env.classes
    for (class_name, class_info) in &env.classes {
        if let Some(&class_id) = index.class_index.get(class_name) {
            for trait_name in &class_info.impl_traits {
                if let Some(&trait_id) = index.trait_index.get(trait_name) {
                    graph.add_edge(class_id, trait_id, DepEdge::ImplementsTrait);
                }
            }
        }
    }

    // 8. Injects edges via env.classes (injected fields)
    for (class_name, class_info) in &env.classes {
        if let Some(&class_id) = index.class_index.get(class_name) {
            for (_, field_type, is_injected) in &class_info.fields {
                if *is_injected {
                    // The field type name is the injected class
                    let injected_name = format!("{}", field_type);
                    if let Some(&injected_id) = index.class_index.get(&injected_name) {
                        graph.add_edge(class_id, injected_id, DepEdge::Injects);
                    }
                }
            }
        }
    }

    // 9. Spawn edges via env.spawn_target_fns
    for (&(_span_start, _span_end), target_fn_name) in &env.spawn_target_fns {
        if let Some(&target_id) = index.fn_index.get(target_fn_name) {
            // We need to find the containing function. The spawn_target_fns key is a span,
            // but we can look up current_fn context through method_resolutions or iterate.
            // For now, record just the target — the caller side is already captured
            // by walk_block_for_edges when it encounters Expr::Spawn containing a Call.
            // The Expr::Spawn → Expr::Call will have target_id set by xref, so
            // walk_expr_for_edges already records the Calls edge. This is a safety net
            // for cases where the spawn target is resolved differently.
            let _ = target_id;
        }
    }

    graph
}

/// Walk a block and record call/construct edges from the containing function.
fn walk_block_for_edges(block: &Block, container_id: Uuid, graph: &mut DependencyGraph) {
    for stmt in &block.stmts {
        walk_stmt_for_edges(&stmt.node, container_id, graph);
    }
}

fn walk_stmt_for_edges(stmt: &Stmt, container_id: Uuid, graph: &mut DependencyGraph) {
    match stmt {
        Stmt::Let { value, .. } => walk_expr_for_edges(&value.node, container_id, graph),
        Stmt::Return(Some(expr)) => walk_expr_for_edges(&expr.node, container_id, graph),
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => walk_expr_for_edges(&value.node, container_id, graph),
        Stmt::FieldAssign { object, value, .. } => {
            walk_expr_for_edges(&object.node, container_id, graph);
            walk_expr_for_edges(&value.node, container_id, graph);
        }
        Stmt::If { condition, then_block, else_block } => {
            walk_expr_for_edges(&condition.node, container_id, graph);
            walk_block_for_edges(&then_block.node, container_id, graph);
            if let Some(eb) = else_block {
                walk_block_for_edges(&eb.node, container_id, graph);
            }
        }
        Stmt::While { condition, body } => {
            walk_expr_for_edges(&condition.node, container_id, graph);
            walk_block_for_edges(&body.node, container_id, graph);
        }
        Stmt::For { iterable, body, .. } => {
            walk_expr_for_edges(&iterable.node, container_id, graph);
            walk_block_for_edges(&body.node, container_id, graph);
        }
        Stmt::IndexAssign { object, index, value } => {
            walk_expr_for_edges(&object.node, container_id, graph);
            walk_expr_for_edges(&index.node, container_id, graph);
            walk_expr_for_edges(&value.node, container_id, graph);
        }
        Stmt::Match { expr, arms } => {
            walk_expr_for_edges(&expr.node, container_id, graph);
            for arm in arms {
                walk_block_for_edges(&arm.body.node, container_id, graph);
            }
        }
        Stmt::Raise { fields, error_id, .. } => {
            if let Some(eid) = error_id {
                graph.add_edge(container_id, *eid, DepEdge::RaisesError);
            }
            for (_, expr) in fields {
                walk_expr_for_edges(&expr.node, container_id, graph);
            }
        }
        Stmt::LetChan { capacity, .. } => {
            if let Some(cap) = capacity {
                walk_expr_for_edges(&cap.node, container_id, graph);
            }
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &arm.op {
                    crate::parser::ast::SelectOp::Recv { channel, .. } => {
                        walk_expr_for_edges(&channel.node, container_id, graph);
                    }
                    crate::parser::ast::SelectOp::Send { channel, value } => {
                        walk_expr_for_edges(&channel.node, container_id, graph);
                        walk_expr_for_edges(&value.node, container_id, graph);
                    }
                }
                walk_block_for_edges(&arm.body.node, container_id, graph);
            }
            if let Some(def) = default {
                walk_block_for_edges(&def.node, container_id, graph);
            }
        }
        Stmt::Scope { seeds, body, .. } => {
            for seed in seeds {
                walk_expr_for_edges(&seed.node, container_id, graph);
            }
            walk_block_for_edges(&body.node, container_id, graph);
        }
        Stmt::Break | Stmt::Continue => {}
        Stmt::Yield { value } => walk_expr_for_edges(&value.node, container_id, graph),
        Stmt::Expr(expr) => walk_expr_for_edges(&expr.node, container_id, graph),
    }
}

fn walk_expr_for_edges(expr: &Expr, container_id: Uuid, graph: &mut DependencyGraph) {
    match expr {
        // 1. Function call edges
        Expr::Call { args, target_id, .. } => {
            if let Some(tid) = target_id {
                graph.add_edge(container_id, *tid, DepEdge::Calls);
            }
            for arg in args {
                walk_expr_for_edges(&arg.node, container_id, graph);
            }
        }
        // 3. Struct construction edges
        Expr::StructLit { fields, target_id, .. } => {
            if let Some(tid) = target_id {
                graph.add_edge(container_id, *tid, DepEdge::Constructs);
            }
            for (_, expr) in fields {
                walk_expr_for_edges(&expr.node, container_id, graph);
            }
        }
        // 4. Enum construction edges
        Expr::EnumUnit { enum_id, .. } => {
            if let Some(eid) = enum_id {
                graph.add_edge(container_id, *eid, DepEdge::Constructs);
            }
        }
        Expr::EnumData { enum_id, fields, .. } => {
            if let Some(eid) = enum_id {
                graph.add_edge(container_id, *eid, DepEdge::Constructs);
            }
            for (_, expr) in fields {
                walk_expr_for_edges(&expr.node, container_id, graph);
            }
        }
        // Closure targets
        Expr::ClosureCreate { target_id, .. } => {
            if let Some(tid) = target_id {
                graph.add_edge(container_id, *tid, DepEdge::Calls);
            }
        }
        // Spawn → the inner call's target_id is captured by the Call arm
        Expr::Spawn { call } => {
            walk_expr_for_edges(&call.node, container_id, graph);
        }
        // Recurse into sub-expressions
        Expr::BinOp { lhs, rhs, .. } => {
            walk_expr_for_edges(&lhs.node, container_id, graph);
            walk_expr_for_edges(&rhs.node, container_id, graph);
        }
        Expr::UnaryOp { operand, .. } => {
            walk_expr_for_edges(&operand.node, container_id, graph);
        }
        Expr::FieldAccess { object, .. } => {
            walk_expr_for_edges(&object.node, container_id, graph);
        }
        Expr::MethodCall { object, args, .. } => {
            walk_expr_for_edges(&object.node, container_id, graph);
            for arg in args {
                walk_expr_for_edges(&arg.node, container_id, graph);
            }
        }
        Expr::ArrayLit { elements } => {
            for el in elements {
                walk_expr_for_edges(&el.node, container_id, graph);
            }
        }
        Expr::Index { object, index } => {
            walk_expr_for_edges(&object.node, container_id, graph);
            walk_expr_for_edges(&index.node, container_id, graph);
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    walk_expr_for_edges(&e.node, container_id, graph);
                }
            }
        }
        Expr::Closure { body, .. } => {
            walk_block_for_edges(&body.node, container_id, graph);
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                walk_expr_for_edges(&k.node, container_id, graph);
                walk_expr_for_edges(&v.node, container_id, graph);
            }
        }
        Expr::SetLit { elements, .. } => {
            for el in elements {
                walk_expr_for_edges(&el.node, container_id, graph);
            }
        }
        Expr::Propagate { expr } => {
            walk_expr_for_edges(&expr.node, container_id, graph);
        }
        Expr::Catch { expr, .. } => {
            walk_expr_for_edges(&expr.node, container_id, graph);
        }
        Expr::Cast { expr, .. } => {
            walk_expr_for_edges(&expr.node, container_id, graph);
        }
        Expr::Range { start, end, .. } => {
            walk_expr_for_edges(&start.node, container_id, graph);
            walk_expr_for_edges(&end.node, container_id, graph);
        }
        Expr::NullPropagate { expr } => {
            walk_expr_for_edges(&expr.node, container_id, graph);
        }
        // Leaf expressions — no edges
        Expr::IntLit(_)
        | Expr::FloatLit(_)
        | Expr::BoolLit(_)
        | Expr::StringLit(_)
        | Expr::Ident(_)
        | Expr::NoneLit => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn test_add_edge_deduplicates() {
        let mut g = DependencyGraph::new();
        let a = uuid(1);
        let b = uuid(2);
        g.add_edge(a, b, DepEdge::Calls);
        g.add_edge(a, b, DepEdge::Calls);
        assert_eq!(g.deps_of(a).len(), 1);
        assert_eq!(g.rdeps_of(b).len(), 1);
    }

    #[test]
    fn test_add_edge_different_kinds_not_deduplicated() {
        let mut g = DependencyGraph::new();
        let a = uuid(1);
        let b = uuid(2);
        g.add_edge(a, b, DepEdge::Calls);
        g.add_edge(a, b, DepEdge::Constructs);
        assert_eq!(g.deps_of(a).len(), 2);
    }

    #[test]
    fn test_deps_of_unknown() {
        let g = DependencyGraph::new();
        assert!(g.deps_of(uuid(999)).is_empty());
        assert!(g.rdeps_of(uuid(999)).is_empty());
    }

    #[test]
    fn test_affected_impl_only() {
        // A --Calls--> B --MethodOf--> C
        // Change A as ImplOnly → should NOT propagate through Calls (API edge) to rdeps of A
        // But A itself is affected.
        //
        // Actually: affected starts from changed set and walks REVERSE edges.
        // If B is changed ImplOnly, we look at rdeps of B. A depends on B via Calls.
        // ImplOnly + API edge = don't propagate. So only B is affected.
        let mut g = DependencyGraph::new();
        let a = uuid(1);
        let b = uuid(2);
        let c = uuid(3);
        g.add_edge(a, b, DepEdge::Calls);     // A calls B
        g.add_edge(b, c, DepEdge::MethodOf);   // B is method of C

        let mut changed = HashMap::new();
        changed.insert(b, ChangeKind::ImplOnly);
        let affected = g.affected(&changed);

        // B changed ImplOnly. rdeps of B: A via Calls (API edge, blocked).
        // So only B is affected.
        assert!(affected.contains(&b));
        assert!(!affected.contains(&a));
        assert!(!affected.contains(&c));
    }

    #[test]
    fn test_affected_api_change() {
        // A --Calls--> B
        // Change B as ApiAndImpl → A is affected (via reverse Calls edge)
        let mut g = DependencyGraph::new();
        let a = uuid(1);
        let b = uuid(2);
        g.add_edge(a, b, DepEdge::Calls);

        let mut changed = HashMap::new();
        changed.insert(b, ChangeKind::ApiAndImpl);
        let affected = g.affected(&changed);

        assert!(affected.contains(&a));
        assert!(affected.contains(&b));
    }

    #[test]
    fn test_affected_transitive() {
        // A --Calls--> B --Calls--> C
        // Change C as ApiAndImpl → B and A are both affected
        let mut g = DependencyGraph::new();
        let a = uuid(1);
        let b = uuid(2);
        let c = uuid(3);
        g.add_edge(a, b, DepEdge::Calls);
        g.add_edge(b, c, DepEdge::Calls);

        let mut changed = HashMap::new();
        changed.insert(c, ChangeKind::ApiAndImpl);
        let affected = g.affected(&changed);

        assert!(affected.contains(&a));
        assert!(affected.contains(&b));
        assert!(affected.contains(&c));
    }

    #[test]
    fn test_affected_impl_only_through_method_of() {
        // A --MethodOf--> C
        // Change C as ImplOnly → A is affected (MethodOf is non-API)
        let mut g = DependencyGraph::new();
        let a = uuid(1);
        let c = uuid(2);
        g.add_edge(a, c, DepEdge::MethodOf);

        let mut changed = HashMap::new();
        changed.insert(c, ChangeKind::ImplOnly);
        let affected = g.affected(&changed);

        assert!(affected.contains(&a));
        assert!(affected.contains(&c));
    }
}
