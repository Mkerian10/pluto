use std::collections::HashSet;

use uuid::Uuid;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::{Span, Spanned};
use crate::visit::{walk_block_mut, walk_expr_mut, walk_stmt_mut, VisitMut};

/// Desugar ambient DI (`uses` on classes, `ambient` in app).
///
/// For each class with `uses Logger`:
///   - Adds a hidden injected field `logger: Logger` (is_injected=true, is_ambient=true)
///   - Rewrites bare `logger` references in methods to `self.logger`
///
/// For the app:
///   - Adds hidden injected fields for each `ambient Logger` declaration
///   - Rewrites bare references in app methods similarly
pub fn desugar_ambient(program: &mut Program) -> Result<(), CompileError> {
    // Process each class with `uses`
    for class in &mut program.classes {
        let c = &mut class.node;
        if c.uses.is_empty() {
            continue;
        }

        // Reject `uses` on generic classes
        if !c.type_params.is_empty() {
            return Err(CompileError::type_err(
                format!("generic class '{}' cannot use ambient dependencies", c.name.node),
                class.span,
            ));
        }

        // Build the set of ambient variable names for this class
        let mut ambient_vars: HashSet<String> = HashSet::new();

        // Collect existing field and bracket dep names for collision checking
        let existing_names: HashSet<String> = c.fields.iter()
            .map(|f| f.name.node.clone())
            .collect();

        // Find the insertion point: after injected fields, before regular fields
        let insert_pos = c.fields.iter().position(|f| !f.is_injected).unwrap_or(c.fields.len());

        // Process uses in reverse so insert_pos stays valid
        let uses = std::mem::take(&mut c.uses);
        let mut fields_to_insert = Vec::new();

        for type_name in &uses {
            let var_name = lowercase_first(&type_name.node);

            // Check for duplicate ambient var names
            if !ambient_vars.insert(var_name.clone()) {
                return Err(CompileError::type_err(
                    format!("duplicate ambient dependency '{}' in class '{}'", type_name.node, c.name.node),
                    type_name.span,
                ));
            }

            // Check collision with existing fields
            if existing_names.contains(&var_name) {
                return Err(CompileError::type_err(
                    format!("ambient variable '{}' conflicts with existing field in class '{}'", var_name, c.name.node),
                    type_name.span,
                ));
            }

            fields_to_insert.push(Field {
                id: Uuid::new_v4(),
                name: Spanned::new(var_name, type_name.span),
                ty: Spanned::new(TypeExpr::Named(type_name.node.clone()), type_name.span),
                is_injected: true,
                is_ambient: true,
            });
        }

        // Insert ambient fields at the right position
        for (i, field) in fields_to_insert.into_iter().enumerate() {
            c.fields.insert(insert_pos + i, field);
        }

        c.uses = uses;

        // Rewrite method bodies
        for method in &mut c.methods {
            let params: HashSet<String> = method.node.params.iter()
                .map(|p| p.name.node.clone())
                .collect();
            let mut active = ambient_vars.clone();
            // Remove method params from active set
            for p in &params {
                active.remove(p);
            }
            rewrite_block(&mut method.node.body.node, &active);
        }
    }

    // Process app ambient types
    if let Some(app_spanned) = &mut program.app {
        let app = &mut app_spanned.node;
        if !app.ambient_types.is_empty() {
            let mut ambient_vars: HashSet<String> = HashSet::new();

            let existing_names: HashSet<String> = app.inject_fields.iter()
                .map(|f| f.name.node.clone())
                .collect();

            let mut fields_to_add = Vec::new();

            for type_name in &app.ambient_types {
                let var_name = lowercase_first(&type_name.node);

                if !ambient_vars.insert(var_name.clone()) {
                    return Err(CompileError::type_err(
                        format!("duplicate ambient type '{}' in app", type_name.node),
                        type_name.span,
                    ));
                }

                if existing_names.contains(&var_name) {
                    return Err(CompileError::type_err(
                        format!("ambient variable '{}' conflicts with existing app dependency", var_name),
                        type_name.span,
                    ));
                }

                fields_to_add.push(Field {
                    id: Uuid::new_v4(),
                    name: Spanned::new(var_name, type_name.span),
                    ty: Spanned::new(TypeExpr::Named(type_name.node.clone()), type_name.span),
                    is_injected: true,
                    is_ambient: true,
                });
            }

            app.inject_fields.extend(fields_to_add);

            // Rewrite app method bodies
            for method in &mut app.methods {
                let params: HashSet<String> = method.node.params.iter()
                    .map(|p| p.name.node.clone())
                    .collect();
                let mut active = ambient_vars.clone();
                for p in &params {
                    active.remove(p);
                }
                rewrite_block(&mut method.node.body.node, &active);
            }
        }
    }

    // Process stage ambient types
    for stage_spanned in &mut program.stages {
        let stage = &mut stage_spanned.node;
        if !stage.ambient_types.is_empty() {
            let mut ambient_vars: HashSet<String> = HashSet::new();

            let existing_names: HashSet<String> = stage.inject_fields.iter()
                .map(|f| f.name.node.clone())
                .collect();

            let mut fields_to_add = Vec::new();

            for type_name in &stage.ambient_types {
                let var_name = lowercase_first(&type_name.node);

                if !ambient_vars.insert(var_name.clone()) {
                    return Err(CompileError::type_err(
                        format!("duplicate ambient type '{}' in stage", type_name.node),
                        type_name.span,
                    ));
                }

                if existing_names.contains(&var_name) {
                    return Err(CompileError::type_err(
                        format!("ambient variable '{}' conflicts with existing stage dependency", var_name),
                        type_name.span,
                    ));
                }

                fields_to_add.push(Field {
                    id: Uuid::new_v4(),
                    name: Spanned::new(var_name, type_name.span),
                    ty: Spanned::new(TypeExpr::Named(type_name.node.clone()), type_name.span),
                    is_injected: true,
                    is_ambient: true,
                });
            }

            stage.inject_fields.extend(fields_to_add);

            // Rewrite stage method bodies
            for method in &mut stage.methods {
                let params: HashSet<String> = method.node.params.iter()
                    .map(|p| p.name.node.clone())
                    .collect();
                let mut active = ambient_vars.clone();
                for p in &params {
                    active.remove(p);
                }
                rewrite_block(&mut method.node.body.node, &active);
            }
        }
    }

    Ok(())
}

/// Lowercase the first character of a type name to produce the variable name.
/// e.g. "Logger" -> "logger", "UserDB" -> "userDB"
fn lowercase_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_lowercase().to_string() + chars.as_str(),
    }
}

struct AmbientRewriter<'a> {
    active: &'a HashSet<String>,
}

impl VisitMut for AmbientRewriter<'_> {
    fn visit_expr_mut(&mut self, expr: &mut Spanned<Expr>) {
        // Check if this is an ambient identifier that needs rewriting
        if let Expr::Ident(name) = &expr.node {
            if self.active.contains(name) {
                // Rewrite `logger` to `self.logger`
                let field_name = name.clone();
                expr.node = Expr::FieldAccess {
                    object: Box::new(Spanned::new(Expr::Ident("self".into()), expr.span)),
                    field: Spanned::new(field_name, expr.span),
                };
                return; // Don't recurse into the rewritten expression
            }
        }

        // Handle expressions that introduce new scopes
        match &mut expr.node {
            Expr::Closure { params, body, .. } => {
                let mut inner = self.active.clone();
                for p in params.iter() {
                    inner.remove(&p.name.node);
                }
                let mut inner_rewriter = AmbientRewriter { active: &inner };
                inner_rewriter.visit_block_mut(body);
                return; // Don't use walk_expr_mut since we handled it manually
            }
            Expr::Catch { expr: inner, handler } => {
                // Recurse into the expression
                self.visit_expr_mut(inner);
                // Handle the catch handler with proper scoping
                match handler {
                    CatchHandler::Wildcard { var, body } => {
                        let mut inner_active = self.active.clone();
                        inner_active.remove(&var.node);
                        let mut inner_rewriter = AmbientRewriter { active: &inner_active };
                        inner_rewriter.visit_block_mut(body);
                    }
                    CatchHandler::Shorthand(fb) => {
                        self.visit_expr_mut(fb);
                    }
                }
                return; // Don't use walk_expr_mut
            }
            _ => {}
        }

        // For all other expressions, use the default walker
        walk_expr_mut(self, expr);
    }

    fn visit_stmt_mut(&mut self, stmt: &mut Spanned<Stmt>) {
        // Handle ambient assignment rewriting
        if let Stmt::Assign { target, value } = &mut stmt.node {
            if self.active.contains(&target.node) {
                // Rewrite `logger = x` to `self.logger = x`
                let field = target.clone();
                let dummy_val = Spanned::new(Expr::IntLit(0), Span::dummy());
                let val = std::mem::replace(value, dummy_val);
                let self_expr = Spanned::new(Expr::Ident("self".into()), field.span);
                stmt.node = Stmt::FieldAssign {
                    object: self_expr,
                    field,
                    value: val,
                };
                // Rewrite the value expression
                if let Stmt::FieldAssign { value, .. } = &mut stmt.node {
                    self.visit_expr_mut(value);
                }
                return; // Don't use walk_stmt_mut
            }
        }

        // Handle statements that introduce new scopes
        match &mut stmt.node {
            Stmt::For { var, iterable, body } => {
                self.visit_expr_mut(iterable);
                let mut inner = self.active.clone();
                inner.remove(&var.node);
                let mut inner_rewriter = AmbientRewriter { active: &inner };
                inner_rewriter.visit_block_mut(body);
                return;
            }
            Stmt::Match { expr, arms } => {
                self.visit_expr_mut(expr);
                for arm in arms {
                    let mut inner = self.active.clone();
                    for (binding, rename) in &arm.bindings {
                        let name = rename.as_ref().unwrap_or(binding);
                        inner.remove(&name.node);
                    }
                    let mut inner_rewriter = AmbientRewriter { active: &inner };
                    inner_rewriter.visit_block_mut(&mut arm.body);
                }
                return;
            }
            Stmt::Scope { seeds, bindings, body } => {
                for seed in seeds {
                    self.visit_expr_mut(seed);
                }
                let mut inner = self.active.clone();
                for binding in bindings {
                    inner.remove(&binding.name.node);
                }
                let mut inner_rewriter = AmbientRewriter { active: &inner };
                inner_rewriter.visit_block_mut(body);
                return;
            }
            _ => {}
        }

        // Default: use the walker
        walk_stmt_mut(self, stmt);
    }

    fn visit_block_mut(&mut self, block: &mut Spanned<Block>) {
        let mut active = self.active.clone();
        for stmt in &mut block.node.stmts {
            let mut stmt_rewriter = AmbientRewriter { active: &active };
            stmt_rewriter.visit_stmt_mut(stmt);

            // Update active set based on bindings in this statement
            if let Stmt::Let { name, .. } = &stmt.node {
                active.remove(&name.node);
            }
            if let Stmt::LetChan { sender, receiver, .. } = &stmt.node {
                active.remove(&sender.node);
                active.remove(&receiver.node);
            }
        }
    }
}

fn rewrite_block(block: &mut Block, active: &HashSet<String>) {
    let mut spanned = Spanned::new(std::mem::replace(block, Block { stmts: vec![] }), Span::dummy());
    let mut rewriter = AmbientRewriter { active };
    rewriter.visit_block_mut(&mut spanned);
    *block = spanned.node;
}


