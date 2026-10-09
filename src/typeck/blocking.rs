//! Blocking-effect inference (#369 execution-model, §6).
//!
//! Computes which functions/methods can block the OS thread, transitively.
//! A node "blocks" if it reaches a *blocking leaf*: an operation that parks
//! the calling thread with no scheduler-yield/readiness form. On a green
//! scheduler (the #369 two-tier proposal) such a call stalls the whole
//! scheduler, so this is the analysis a green-task boundary check (reject a
//! green task that can block, naming the path) and a safe CPU-only spawn pool
//! both rest on.
//!
//! Structurally this mirrors `crate::concurrency::infer_synchronization`: an
//! all-calls call graph (every call edge, since blocking flows through a call
//! regardless of `catch`, unlike error-ability) plus a union fixed point. It
//! is computed today but NOT yet enforced — no green-task construct exists, so
//! nothing consumes `env.blocking_fns`. That keeps this pass side-effect free.
//!
//! ## The leaf set (deliberately explicit — it is a design knob)
//!
//! The leaf set shrinks as the runtime gains nonblocking+yield forms (#369
//! §6: "later versions remove leaves from the set as each gets a
//! nonblocking+yield form"). Today the runtime does NO readiness/nonblocking
//! I/O, so every §5 surface is a hard blocker and is listed here. Membership
//! is the "which tier does green v1 support" question and is the owner's call;
//! this module just makes the mechanism correct and the set easy to edit.

use std::collections::{HashMap, HashSet};

use crate::parser::ast::*;
use crate::span::Spanned;
use crate::typeck::env::{mangle_method, MethodResolution, TypeEnv};
use crate::visit::{walk_expr, walk_stmt, Visitor};

/// Blocking C intrinsics (extern leaves) from the #369 §5 inventory. A call
/// to any of these names blocks the OS thread. Stdlib wrappers that call them
/// (e.g. `std.fs.read_all`) inherit the effect through the fixed point. Names
/// are the extern symbols as declared in the stdlib `extern fn` bindings.
pub const BLOCKING_EXTERN_LEAVES: &[&str] = &[
    // filesystem — no portable async form; a green scheduler must offload these
    "__pluto_fs_open", "__pluto_fs_open_read", "__pluto_fs_open_write",
    "__pluto_fs_read", "__pluto_fs_write", "__pluto_fs_pread", "__pluto_fs_pwrite",
    "__pluto_fs_sync", "__pluto_fs_fsync", "__pluto_fs_fdatasync",
    "__pluto_fs_read_all", "__pluto_fs_write_all", "__pluto_fs_copy",
    // sockets — kqueue-readiness convertible, but blocking today
    "__pluto_socket_accept", "__pluto_socket_connect",
    "__pluto_socket_read", "__pluto_socket_read_bytes",
    "__pluto_socket_write", "__pluto_socket_write_bytes",
    "__pluto_write_framed", "__pluto_read_framed",
    // stdin / sleep / http
    "__pluto_io_read_line", "__pluto_time_sleep_ns",
    "__pluto_http_read_request",
];

/// Is this method resolution a blocking leaf? Channel recv/send and task join
/// park the thread; the `try_` variants and detach/cancel return immediately.
fn resolution_is_blocking(res: &MethodResolution) -> bool {
    matches!(
        res,
        MethodResolution::ChannelSend
            | MethodResolution::ChannelRecv
            | MethodResolution::ChannelRecvTimeout
            | MethodResolution::TaskGet { .. }
    )
}

/// Compute `env.blocking_fns`: the set of function/method mangled names that
/// can block the OS thread transitively.
pub fn infer_blocking_effects(program: &Program, env: &mut TypeEnv) {
    let leaves: HashSet<String> =
        BLOCKING_EXTERN_LEAVES.iter().map(|s| s.to_string()).collect();

    // Step 1: per-node direct blocking + call edges.
    let mut directly_blocks: HashSet<String> = HashSet::new();
    let mut call_edges: HashMap<String, HashSet<String>> = HashMap::new();

    let mut collect = |node_name: String, body: &Block, env: &TypeEnv| {
        let (blocks, edges) = collect_block(body, &node_name, env, &leaves);
        if blocks {
            directly_blocks.insert(node_name.clone());
        }
        call_edges.entry(node_name).or_default().extend(edges);
    };

    for func in &program.functions {
        collect(func.node.name.node.clone(), &func.node.body.node, env);
    }
    for class in &program.classes {
        let class_name = &class.node.name.node;
        for method in &class.node.methods {
            collect(mangle_method(class_name, &method.node.name.node), &method.node.body.node, env);
        }
    }
    // Inherited default trait methods (keyed under the implementing class).
    for class in &program.classes {
        let class_name = &class.node.name.node;
        let own: Vec<String> =
            class.node.methods.iter().map(|m| m.node.name.node.clone()).collect();
        for impl_trait in &class.node.impl_traits {
            for trait_decl in &program.traits {
                if trait_decl.node.name.node == impl_trait.name.node {
                    for tm in &trait_decl.node.methods {
                        if let Some(body) = &tm.body {
                            if !own.contains(&tm.name.node) {
                                collect(mangle_method(class_name, &tm.name.node), &body.node, env);
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(app) = &program.app {
        let app_name = &app.node.name.node;
        for method in &app.node.methods {
            collect(mangle_method(app_name, &method.node.name.node), &method.node.body.node, env);
        }
    }
    for stage in &program.stages {
        let stage_name = &stage.node.name.node;
        for method in &stage.node.methods {
            collect(mangle_method(stage_name, &method.node.name.node), &method.node.body.node, env);
        }
    }

    // Step 2: fixed point — a node blocks if it directly blocks or calls a
    // node that blocks (an extern leaf counts as a blocking callee).
    let mut blocking: HashSet<String> = directly_blocks;
    loop {
        let mut changed = false;
        for (node, edges) in &call_edges {
            if blocking.contains(node) {
                continue;
            }
            if edges.iter().any(|c| blocking.contains(c) || leaves.contains(c)) {
                blocking.insert(node.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    env.blocking_fns = blocking;
}

/// Visitor collecting, for one node body: whether it directly hits a blocking
/// leaf, and its outgoing call edges (callee names/mangled names).
struct BlockingCollector<'a> {
    blocks: bool,
    edges: &'a mut HashSet<String>,
    current_fn: &'a str,
    env: &'a TypeEnv,
    leaves: &'a HashSet<String>,
}

impl Visitor for BlockingCollector<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        match &expr.node {
            Expr::Call { name, .. } => {
                let n = name.node.clone();
                if self.leaves.contains(&n) {
                    self.blocks = true;
                }
                self.edges.insert(n);
            }
            Expr::MethodCall { method, .. } => {
                let key = (self.current_fn.to_string(), method.span.start);
                if let Some(res) = self.env.method_resolutions.get(&key) {
                    if resolution_is_blocking(res) {
                        self.blocks = true;
                    }
                    if let MethodResolution::Class { mangled_name }
                    | MethodResolution::RemoteClass { mangled_name } = res
                    {
                        self.edges.insert(mangled_name.clone());
                    }
                }
            }
            Expr::Spawn { call } => {
                // A spawned body runs on its OWN unit of execution — whether it
                // blocks is a property of that unit, not of the spawner. Do not
                // propagate the spawned closure's blocking into this node.
                // (Still recurse into spawn-arg expressions, which run here.)
                if let Expr::Closure { body, .. } = &call.node {
                    for stmt in &body.node.stmts {
                        if let Stmt::Return(Some(ret)) = &stmt.node {
                            let args = match &ret.node {
                                Expr::Call { args, .. } | Expr::MethodCall { args, .. } => Some(args),
                                _ => None,
                            };
                            if let Some(args) = args {
                                for a in args {
                                    self.visit_expr(a);
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
            _ => {}
        }
        walk_expr(self, expr);
    }

    fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
        walk_stmt(self, stmt);
    }
}

fn collect_block(
    block: &Block,
    current_fn: &str,
    env: &TypeEnv,
    leaves: &HashSet<String>,
) -> (bool, HashSet<String>) {
    let mut edges = HashSet::new();
    let mut collector = BlockingCollector {
        blocks: false,
        edges: &mut edges,
        current_fn,
        env,
        leaves,
    };
    for stmt in &block.stmts {
        collector.visit_stmt(stmt);
    }
    let blocks = collector.blocks;
    (blocks, edges)
}

#[cfg(test)]
mod tests {
    use crate::lexer::lex;
    use crate::parser::Parser;

    fn blocking_fns(src: &str) -> std::collections::HashSet<String> {
        let tokens = lex(src).unwrap();
        let mut parser = Parser::new(&tokens, src);
        let mut program = parser.parse_program().unwrap();
        crate::modules::resolve_qualified_access_single_file(&mut program).unwrap();
        let result = crate::run_frontend(&mut program, false).unwrap();
        result.env.blocking_fns
    }

    #[test]
    fn channel_recv_is_a_blocking_leaf_and_propagates() {
        let b = blocking_fns(
            r#"
fn waiter(rx: Receiver<int>) int {
    return rx.recv() catch 0
}
fn indirect(rx: Receiver<int>) int {
    return waiter(rx)
}
fn pure(x: int) int {
    return x + 1
}
fn main() {
    let (tx, rx) = chan<int>(1)
    print(waiter(rx))
    print(indirect(rx))
    print(pure(3))
}
"#,
        );
        assert!(b.contains("waiter"), "waiter recv()s → blocks; got {b:?}");
        assert!(b.contains("indirect"), "indirect calls waiter → transitively blocks; got {b:?}");
        assert!(!b.contains("pure"), "pure does no blocking op; got {b:?}");
    }

    #[test]
    fn task_join_blocks_but_try_recv_does_not() {
        let b = blocking_fns(
            r#"
fn work(x: int) int {
    return x + 1
}
fn joiner() int {
    let t = spawn work(3)
    return t.get()
}
fn poller(rx: Receiver<int>) int {
    return rx.try_recv() catch 0
}
fn main() {
    print(joiner())
    let (tx, rx) = chan<int>(1)
    print(poller(rx))
}
"#,
        );
        assert!(b.contains("joiner"), "joiner .get()s a task → blocks; got {b:?}");
        assert!(!b.contains("poller"), "try_recv never parks → not blocking; got {b:?}");
    }

    #[test]
    fn extern_blocking_intrinsic_is_a_leaf() {
        let b = blocking_fns(
            r#"
extern fn __pluto_fs_read(fd: int) int
fn reads() int {
    return __pluto_fs_read(3)
}
fn pure(x: int) int {
    return x + 1
}
fn main() {
    print(reads())
    print(pure(1))
}
"#,
        );
        assert!(b.contains("reads"), "reads calls a blocking fs intrinsic → blocks; got {b:?}");
        assert!(!b.contains("pure"), "pure does no blocking op; got {b:?}");
    }
}
