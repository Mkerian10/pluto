use std::collections::{HashMap, HashSet};

use crate::parser::ast::*;
use crate::typeck::env::{mangle_method, TypeEnv};
use crate::span::Spanned;
use crate::visit::{walk_expr, walk_stmt, Visitor};

/// Infer which DI singletons need rwlock synchronization.
///
/// A singleton needs synchronization when it is accessed from both:
///   (a) a spawned task (transitively through spawn target functions), AND
///   (b) the main thread (transitively through App$main)
///
/// Also: if two different spawn targets both access the same singleton,
/// that singleton needs sync (concurrent tasks accessing shared state).
pub fn infer_synchronization(program: &Program, env: &mut TypeEnv) {
    // No app/stage or no DI singletons → nothing to synchronize
    if env.app.is_none() && env.stages.is_empty() || env.di_order.is_empty() {
        return;
    }
    // No spawn targets → no concurrency → nothing to synchronize
    if env.spawn_target_fns.is_empty() {
        return;
    }

    let di_singletons: HashSet<String> = env.di_order.iter().cloned().collect();

    // Step 1: Collect direct singleton accesses and call-graph edges per function/method
    let mut singleton_accesses: HashMap<String, HashSet<String>> = HashMap::new();
    let mut call_edges: HashMap<String, HashSet<String>> = HashMap::new();

    // Top-level functions
    for func in &program.functions {
        if !func.node.type_params.is_empty() { continue; }
        let name = func.node.name.node.clone();
        let (accesses, edges) = collect_block_accesses(&func.node.body.node, &name, env, &di_singletons);
        singleton_accesses.entry(name.clone()).or_default().extend(accesses);
        call_edges.entry(name).or_default().extend(edges);
    }

    // Class methods
    for class in &program.classes {
        if !class.node.type_params.is_empty() { continue; }
        let class_name = &class.node.name.node;
        for method in &class.node.methods {
            let mangled = mangle_method(class_name, &method.node.name.node);
            let (accesses, edges) = collect_block_accesses(&method.node.body.node, &mangled, env, &di_singletons);
            singleton_accesses.entry(mangled.clone()).or_default().extend(accesses);
            call_edges.entry(mangled).or_default().extend(edges);
        }
    }

    // Default trait methods inherited by classes
    for class in &program.classes {
        if !class.node.type_params.is_empty() { continue; }
        let class_name = &class.node.name.node;
        let class_method_names: Vec<String> =
            class.node.methods.iter().map(|m| m.node.name.node.clone()).collect();
        for trait_name in &class.node.impl_traits {
            for trait_decl in &program.traits {
                if trait_decl.node.name.node == trait_name.node {
                    for tm in &trait_decl.node.methods {
                        if let Some(body) = &tm.body && !class_method_names.contains(&tm.name.node) {
                            let mangled = mangle_method(class_name, &tm.name.node);
                            let (accesses, edges) = collect_block_accesses(&body.node, &mangled, env, &di_singletons);
                            singleton_accesses.entry(mangled.clone()).or_default().extend(accesses);
                            call_edges.entry(mangled).or_default().extend(edges);
                        }
                    }
                }
            }
        }
    }

    // App methods
    if let Some(app_spanned) = &program.app {
        let app_name = &app_spanned.node.name.node;
        for method in &app_spanned.node.methods {
            let mangled = mangle_method(app_name, &method.node.name.node);
            let (accesses, edges) = collect_block_accesses(&method.node.body.node, &mangled, env, &di_singletons);
            singleton_accesses.entry(mangled.clone()).or_default().extend(accesses);
            call_edges.entry(mangled).or_default().extend(edges);
        }
    }

    // Stage methods
    for stage_spanned in &program.stages {
        let stage_name = &stage_spanned.node.name.node;
        for method in &stage_spanned.node.methods {
            let mangled = mangle_method(stage_name, &method.node.name.node);
            let (accesses, edges) = collect_block_accesses(&method.node.body.node, &mangled, env, &di_singletons);
            singleton_accesses.entry(mangled.clone()).or_default().extend(accesses);
            call_edges.entry(mangled).or_default().extend(edges);
        }
    }

    // Step 2: Fixed-point propagation — if fn A calls fn B and B accesses singleton S, then A also accesses S
    loop {
        let mut changed = false;
        for (fn_name, edges) in &call_edges.clone() {
            let mut current = singleton_accesses.get(fn_name).cloned().unwrap_or_default();
            for callee in edges {
                if let Some(callee_accesses) = singleton_accesses.get(callee) {
                    for s in callee_accesses {
                        if current.insert(s.clone()) {
                            changed = true;
                        }
                    }
                }
            }
            singleton_accesses.insert(fn_name.clone(), current);
        }
        if !changed {
            break;
        }
    }

    // Step 3: Compute spawn-side and main-side accesses
    let mut spawn_side: HashSet<String> = HashSet::new();
    let mut per_spawn_accesses: Vec<HashSet<String>> = Vec::new();
    for target_fn in env.spawn_target_fns.values() {
        let accesses = singleton_accesses.get(target_fn).cloned().unwrap_or_default();
        spawn_side.extend(accesses.iter().cloned());
        per_spawn_accesses.push(accesses);
    }

    // Determine the main-side entry point (app or stage)
    let main_side = if let Some(app) = &env.app {
        let app_main = mangle_method(&app.0, "main");
        singleton_accesses.get(&app_main).cloned().unwrap_or_default()
    } else if let Some(stage) = env.stages.first() {
        let stage_main = mangle_method(&stage.0, "main");
        singleton_accesses.get(&stage_main).cloned().unwrap_or_default()
    } else {
        return;
    };

    // A singleton needs sync if:
    // (a) accessed from both spawn-side and main-side, OR
    // (b) accessed from two different spawn targets
    let mut synchronized: HashSet<String> = HashSet::new();

    // (a) spawn ∩ main
    for s in &spawn_side {
        if main_side.contains(s) {
            synchronized.insert(s.clone());
        }
    }

    // (b) accessed from multiple different spawn targets
    for singleton in &di_singletons {
        let mut count = 0;
        for per_spawn in &per_spawn_accesses {
            if per_spawn.contains(singleton) {
                count += 1;
            }
        }
        if count >= 2 {
            synchronized.insert(singleton.clone());
        }
    }

    env.synchronized_singletons = synchronized;
}

/// Collect direct singleton accesses and call-graph edges from a block.
struct AccessCollector<'a> {
    accesses: &'a mut HashSet<String>,
    edges: &'a mut HashSet<String>,
    current_fn: &'a str,
    env: &'a TypeEnv,
    di_singletons: &'a HashSet<String>,
}

impl Visitor for AccessCollector<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        // Handle expressions that need access/edge collection
        match &expr.node {
            Expr::Call { name, .. } => {
                self.edges.insert(name.node.clone());
            }
            Expr::MethodCall { method, .. } => {
                // Check if the method call is on a DI singleton — use method_resolutions from typeck
                let key = (self.current_fn.to_string(), method.span.start);
                if let Some(crate::typeck::env::MethodResolution::Class { mangled_name }) = self.env.method_resolutions.get(&key) {
                    // Extract class name from mangled name (format: "ClassName$method")
                    if let Some(class_name) = mangled_name.split('$').next() {
                        if self.di_singletons.contains(class_name) {
                            self.accesses.insert(class_name.to_string());
                        }
                    }
                    self.edges.insert(mangled_name.clone());
                }
            }
            Expr::Spawn { call } => {
                // Spawn is opaque to concurrency analysis for the spawned body.
                // Only collect effects from spawn arg expressions.
                if let Expr::Closure { body, .. } = &call.node {
                    for stmt in &body.node.stmts {
                        if let Stmt::Return(Some(ret_expr)) = &stmt.node {
                            let args = match &ret_expr.node {
                                Expr::Call { args, .. } => Some(args),
                                Expr::MethodCall { args, .. } => Some(args),
                                _ => None,
                            };
                            if let Some(args) = args {
                                for arg in args {
                                    self.visit_expr(arg);
                                }
                            }
                        }
                    }
                }
                return;
            }
            Expr::StringInterp { parts } => {
                for part in parts {
                    if let StringInterpPart::Expr(e) = part {
                        self.visit_expr(e);
                    }
                }
                return;
            }
            Expr::QualifiedAccess { segments } => {
                panic!(
                    "QualifiedAccess should be resolved by module flattening before concurrency. Segments: {:?}",
                    segments.iter().map(|s| &s.node).collect::<Vec<_>>()
                )
            }
            _ => {}
        }
        // Recurse into sub-expressions
        walk_expr(self, expr);
    }

    fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
        // No special statement handling needed, just recurse
        walk_stmt(self, stmt);
    }
}

fn collect_block_accesses(
    block: &Block,
    current_fn: &str,
    env: &TypeEnv,
    di_singletons: &HashSet<String>,
) -> (HashSet<String>, HashSet<String>) {
    let mut accesses = HashSet::new();
    let mut edges = HashSet::new();
    let mut collector = AccessCollector {
        accesses: &mut accesses,
        edges: &mut edges,
        current_fn,
        env,
        di_singletons,
    };
    for stmt in &block.stmts {
        collector.visit_stmt(stmt);
    }
    (accesses, edges)
}

