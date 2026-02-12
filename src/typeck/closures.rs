use std::collections::HashSet;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::Spanned;
use crate::visit::{walk_expr, Visitor};
use super::env::TypeEnv;
use super::types::PlutoType;
use super::resolve::resolve_type;
use super::infer::infer_expr;
use super::check::check_block;

pub(crate) fn infer_closure(
    params: &[Param],
    return_type: &Option<Spanned<TypeExpr>>,
    body: &Spanned<Block>,
    span: crate::span::Span,
    env: &mut TypeEnv,
) -> Result<PlutoType, CompileError> {
    let outer_depth = env.scope_depth();

    // Push a scope for the closure parameters
    env.push_scope();

    // Resolve and define each param
    let mut param_types = Vec::new();
    for p in params {
        let ty = resolve_type(&p.ty, env)?;
        env.define(p.name.node.clone(), ty.clone());
        param_types.push(ty);
    }

    // Determine the return type: annotated or inferred from body
    let final_ret = if let Some(rt) = return_type {
        resolve_type(rt, env)?
    } else {
        // Infer from first return-with-value in the body
        infer_closure_return_type(&body.node, env)?
    };

    // Check the body against the determined return type
    // Reset loop_depth so break/continue inside closures can't escape to enclosing loop
    let saved_loop_depth = env.loop_depth;
    env.loop_depth = 0;
    // Clear generator context so yield cannot be used inside closures
    let saved_gen_elem = env.current_generator_elem.take();
    check_block(&body.node, env, &final_ret)?;
    env.current_generator_elem = saved_gen_elem;
    env.loop_depth = saved_loop_depth;

    // Collect captures: find free variables that come from outer scopes
    let param_names: HashSet<&str> = params.iter().map(|p| p.name.node.as_str()).collect();
    let mut captures = Vec::new();
    let mut seen = HashSet::new();
    collect_free_vars_block(&body.node, &param_names, outer_depth, env, &mut captures, &mut seen);

    // Store captures keyed by span
    env.closure_captures.insert((span.start, span.end), captures.clone());

    // Check if any captured variable is a scope binding → mark closure as tainted
    if !env.scope_binding_names.is_empty() {
        let is_tainted = captures.iter().any(|(name, _)| {
            env.scope_binding_names.iter().any(|set| set.contains(name))
        });
        if is_tainted {
            env.scope_tainted_closures.insert((span.start, span.end));
        }
    }

    // Store return type for closure lifting (fixes Finding 5)
    env.closure_return_types.insert((span.start, span.end), final_ret.clone());

    env.pop_scope();

    Ok(PlutoType::Fn(param_types, Box::new(final_ret)))
}

/// Infer the return type of a closure body by looking for return statements.
/// If the body has a single return with an expression, we infer from that.
/// Otherwise default to Void.
fn infer_closure_return_type(block: &Block, env: &mut TypeEnv) -> Result<PlutoType, CompileError> {
    // Walk statements sequentially, processing let bindings so that
    // variables are in scope when we encounter a return statement.
    for stmt in &block.stmts {
        match &stmt.node {
            Stmt::Let { name, ty, value, .. } => {
                let val_type = infer_expr(&value.node, value.span, env)?;
                if let Some(declared_ty) = ty {
                    let expected = resolve_type(declared_ty, env)?;
                    env.define(name.node.clone(), expected);
                } else {
                    env.define(name.node.clone(), val_type);
                }
            }
            Stmt::LetChan { sender, receiver, elem_type, .. } => {
                let resolved = resolve_type(elem_type, env)?;
                env.define(sender.node.clone(), PlutoType::Sender(Box::new(resolved.clone())));
                env.define(receiver.node.clone(), PlutoType::Receiver(Box::new(resolved)));
            }
            Stmt::Return(Some(expr)) => {
                return infer_expr(&expr.node, expr.span, env);
            }
            _ => {}
        }
    }
    Ok(PlutoType::Void)
}

/// Collect free variables in a block that resolve from scopes at depth < outer_depth.
/// These are the variables captured by a closure.
struct FreeVarCollector<'a> {
    param_names: &'a HashSet<&'a str>,
    outer_depth: usize,
    env: &'a TypeEnv,
    captures: &'a mut Vec<(String, PlutoType)>,
    seen: &'a mut HashSet<String>,
}

impl Visitor for FreeVarCollector<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        // Check if this is a free variable
        if let Expr::Ident(name) = &expr.node {
            // Skip if it's a closure param, a function name, or a builtin
            if self.param_names.contains(name.as_str()) { return; }
            if self.env.functions.contains_key(name) { return; }
            if self.env.builtins.contains(name) { return; }
            if self.seen.contains(name) { return; }
            // Check if this variable resolves from an outer scope (depth < outer_depth)
            if let Some((ty, depth)) = self.env.lookup_with_depth(name) && depth < self.outer_depth {
                self.seen.insert(name.clone());
                self.captures.push((name.clone(), ty.clone()));
            }
            return;
        }

        // Handle QualifiedAccess panic
        if let Expr::QualifiedAccess { segments } = &expr.node {
            panic!(
                "QualifiedAccess should be resolved by module flattening before closures. Segments: {:?}",
                segments.iter().map(|s| &s.node).collect::<Vec<_>>()
            )
        }

        // Handle StringInterp specially (walk doesn't descend into StringInterpPart)
        if let Expr::StringInterp { parts } = &expr.node {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    self.visit_expr(e);
                }
            }
            return;
        }

        // For all other expressions, use the default walker
        walk_expr(self, expr);
    }
}

fn collect_free_vars_block(
    block: &Block,
    param_names: &HashSet<&str>,
    outer_depth: usize,
    env: &TypeEnv,
    captures: &mut Vec<(String, PlutoType)>,
    seen: &mut HashSet<String>,
) {
    let mut collector = FreeVarCollector {
        param_names,
        outer_depth,
        env,
        captures,
        seen,
    };
    for stmt in &block.stmts {
        collector.visit_stmt(stmt);
    }
}
