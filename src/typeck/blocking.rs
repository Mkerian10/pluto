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

    // Names a bare `Expr::Call` may target without being opaque: top-level
    // functions, extern bindings, and builtins. A call to any other name is a
    // closure / function-reference variable whose target we cannot resolve —
    // conservatively opaque (it might reach a blocking op we can't see).
    let mut known_bare: HashSet<String> = env.builtins.clone();
    for func in &program.functions {
        known_bare.insert(func.node.name.node.clone());
    }
    for ext in &program.extern_fns {
        known_bare.insert(ext.node.name.node.clone());
    }

    // Step 1: per-node direct cooperative-block, direct opacity, and call edges.
    // (Extern blocking leaves need no direct flag — a call edge to the leaf name
    // + the leaf set catches them in the fixpoint.)
    let mut directly_coop: HashSet<String> = HashSet::new();
    let mut directly_opaque: HashSet<String> = HashSet::new();
    let mut call_edges: HashMap<String, HashSet<String>> = HashMap::new();

    let mut collect = |node_name: String, body: &Block, env: &TypeEnv| {
        let (coop, opaque, edges) = collect_block(body, &node_name, env, &known_bare);
        if coop {
            directly_coop.insert(node_name.clone());
        }
        if opaque {
            directly_opaque.insert(node_name.clone());
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

    // Step 2: union fixed points over the same call graph.
    //   blocking_fns     — blocks the OS thread TODAY: reaches an extern leaf
    //                      (via edge + leaf set) OR a cooperative block
    //                      (channel/task, seeded by directly_coop).
    //   green_illegal_fns — blocks with NO cooperative form: reaches an extern
    //                      leaf only. Channel/task are omitted (they yield under
    //                      green), so this is strictly narrower.
    //   blocking_opaque_fns — reaches an unresolvable call (seeded by opacity).
    let empty = HashSet::new();
    env.blocking_fns = fixpoint(&call_edges, directly_coop, &leaves);
    env.green_illegal_fns = fixpoint(&call_edges, HashSet::new(), &leaves);
    env.blocking_opaque_fns = fixpoint(&call_edges, directly_opaque, &empty);
}

/// Union fixed point: a node joins `set` if it is seeded, or it has an edge to
/// a node already in `set` or to a `leaf`.
fn fixpoint(
    call_edges: &HashMap<String, HashSet<String>>,
    seed: HashSet<String>,
    leaves: &HashSet<String>,
) -> HashSet<String> {
    let mut set = seed;
    loop {
        let mut changed = false;
        for (node, edges) in call_edges {
            if set.contains(node) {
                continue;
            }
            if edges.iter().any(|c| set.contains(c) || leaves.contains(c)) {
                set.insert(node.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    set
}

/// Visitor collecting, for one node body: whether it directly hits a blocking
/// leaf, and its outgoing call edges (callee names/mangled names).
struct BlockingCollector<'a> {
    // Reached a COOPERATIVE blocking op (channel send/recv, task join): blocks
    // the OS thread today, but becomes a scheduler yield under green — so it
    // feeds `blocking_fns` but NOT `green_illegal_fns`.
    coop: bool,
    opaque: bool,
    edges: &'a mut HashSet<String>,
    current_fn: &'a str,
    env: &'a TypeEnv,
    known_bare: &'a HashSet<String>,
}

impl Visitor for BlockingCollector<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        match &expr.node {
            Expr::Call { name, .. } => {
                let n = name.node.clone();
                // An extern blocking leaf is caught by the fixpoint through this
                // edge + the leaf set (no direct flag needed). A target that is
                // not a known function/extern/builtin is a call through a
                // closure or fn-ref variable we cannot inspect — opaque.
                if !self.known_bare.contains(&n) {
                    self.opaque = true;
                }
                self.edges.insert(n);
            }
            Expr::MethodCall { method, .. } => {
                let key = (self.current_fn.to_string(), method.span.start);
                match self.env.method_resolutions.get(&key) {
                    Some(res) if resolution_is_blocking(res) => {
                        self.coop = true;
                    }
                    Some(MethodResolution::Class { mangled_name })
                    | Some(MethodResolution::RemoteClass { mangled_name }) => {
                        self.edges.insert(mangled_name.clone());
                    }
                    // Dynamic dispatch: the concrete impl is unknown and may
                    // block. Conservatively opaque.
                    Some(MethodResolution::TraitDynamic { .. }) => {
                        self.opaque = true;
                    }
                    // Infallible, non-blocking builtins and try_*/detach/cancel.
                    Some(_) => {}
                    // No resolution recorded for a method call — unanalyzable.
                    None => {
                        self.opaque = true;
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
    known_bare: &HashSet<String>,
) -> (bool, bool, HashSet<String>) {
    let mut edges = HashSet::new();
    let mut collector = BlockingCollector {
        coop: false,
        opaque: false,
        edges: &mut edges,
        current_fn,
        env,
        known_bare,
    };
    for stmt in &block.stmts {
        collector.visit_stmt(stmt);
    }
    let coop = collector.coop;
    let opaque = collector.opaque;
    (coop, opaque, edges)
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

    fn opaque_fns(src: &str) -> std::collections::HashSet<String> {
        let tokens = lex(src).unwrap();
        let mut parser = Parser::new(&tokens, src);
        let mut program = parser.parse_program().unwrap();
        crate::modules::resolve_qualified_access_single_file(&mut program).unwrap();
        let result = crate::run_frontend(&mut program, false).unwrap();
        result.env.blocking_opaque_fns
    }

    #[test]
    fn green_guardrail_leaf_set_is_narrower_than_blocking() {
        // A fn that only recv()s a channel blocks the OS thread today, but under
        // green that recv is a cooperative yield — so it is NOT green-illegal.
        // A fn that touches fs has no cooperative form — green-illegal.
        let src = r#"
extern fn __pluto_fs_read(fd: int) int
fn chan_only(rx: Receiver<int>) int {
    return rx.recv() catch 0
}
fn fs_only() int {
    return __pluto_fs_read(3)
}
fn main() {
    let (tx, rx) = chan<int>(1)
    print(chan_only(rx))
    print(fs_only())
}
"#;
        let tokens = lex(src).unwrap();
        let mut parser = Parser::new(&tokens, src);
        let mut program = parser.parse_program().unwrap();
        crate::modules::resolve_qualified_access_single_file(&mut program).unwrap();
        let env = crate::run_frontend(&mut program, false).unwrap().env;
        assert!(env.blocking_fns.contains("chan_only"), "chan recv blocks the OS thread today");
        assert!(!env.green_illegal_fns.contains("chan_only"), "chan recv yields under green — not green-illegal");
        assert!(env.blocking_fns.contains("fs_only"), "fs blocks");
        assert!(env.green_illegal_fns.contains("fs_only"), "fs has no cooperative form — green-illegal");
    }

    #[test]
    fn unresolvable_calls_are_conservatively_opaque() {
        let src = r#"
trait Greeter {
    fn greet(self) int
}
fn via_trait(g: Greeter) int {
    return g.greet()
}
fn apply(f: fn(int) int, x: int) int {
    return f(x)
}
fn b(x: int) int {
    return x * 2
}
fn a(x: int) int {
    return b(x) + 1
}
fn main() {
    print(a(3))
}
"#;
        let o = opaque_fns(src);
        assert!(o.contains("via_trait"), "trait dynamic dispatch is opaque; got {o:?}");
        assert!(o.contains("apply"), "call through a fn-ref variable is opaque; got {o:?}");
        assert!(!o.contains("a"), "a → b are fully resolved named calls; got {o:?}");
        assert!(!o.contains("b"), "b is pure; got {o:?}");
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
