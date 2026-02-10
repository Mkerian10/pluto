use std::collections::HashSet;

use uuid::Uuid;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::{Span, Spanned};

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

fn rewrite_block(block: &mut Block, active: &HashSet<String>) {
    let mut active = active.clone();
    for stmt in &mut block.stmts {
        rewrite_stmt(stmt, &active);
        // If this is a `let` or `let chan` statement, remove bindings from active for subsequent stmts
        if let Stmt::Let { name, .. } = &stmt.node {
            active.remove(&name.node);
        }
        if let Stmt::LetChan { sender, receiver, .. } = &stmt.node {
            active.remove(&sender.node);
            active.remove(&receiver.node);
        }
    }
}

fn rewrite_stmt(stmt: &mut Spanned<Stmt>, active: &HashSet<String>) {
    match &mut stmt.node {
        Stmt::Let { value, .. } => {
            rewrite_expr(&mut value.node, value.span, active);
        }
        Stmt::Return(Some(expr)) => {
            rewrite_expr(&mut expr.node, expr.span, active);
        }
        Stmt::Return(None) => {}
        Stmt::Assign { target, value } => {
            if active.contains(&target.node) {
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
                    rewrite_expr(&mut value.node, value.span, active);
                }
            } else {
                rewrite_expr(&mut value.node, value.span, active);
            }
        }
        Stmt::FieldAssign { object, value, .. } => {
            rewrite_expr(&mut object.node, object.span, active);
            rewrite_expr(&mut value.node, value.span, active);
        }
        Stmt::If { condition, then_block, else_block } => {
            rewrite_expr(&mut condition.node, condition.span, active);
            rewrite_block(&mut then_block.node, active);
            if let Some(eb) = else_block {
                rewrite_block(&mut eb.node, active);
            }
        }
        Stmt::While { condition, body } => {
            rewrite_expr(&mut condition.node, condition.span, active);
            rewrite_block(&mut body.node, active);
        }
        Stmt::For { var, iterable, body } => {
            rewrite_expr(&mut iterable.node, iterable.span, active);
            let mut inner = active.clone();
            inner.remove(&var.node);
            rewrite_block(&mut body.node, &inner);
        }
        Stmt::IndexAssign { object, index, value } => {
            rewrite_expr(&mut object.node, object.span, active);
            rewrite_expr(&mut index.node, index.span, active);
            rewrite_expr(&mut value.node, value.span, active);
        }
        Stmt::Match { expr, arms } => {
            rewrite_expr(&mut expr.node, expr.span, active);
            for arm in arms {
                let mut inner = active.clone();
                for (binding, rename) in &arm.bindings {
                    let name = rename.as_ref().unwrap_or(binding);
                    inner.remove(&name.node);
                }
                rewrite_block(&mut arm.body.node, &inner);
            }
        }
        Stmt::Raise { fields, .. } => {
            for (_, val) in fields {
                rewrite_expr(&mut val.node, val.span, active);
            }
        }
        Stmt::Expr(expr) => {
            rewrite_expr(&mut expr.node, expr.span, active);
        }
        Stmt::LetChan { capacity, .. } => {
            if let Some(cap) = capacity {
                rewrite_expr(&mut cap.node, cap.span, active);
            }
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &mut arm.op {
                    SelectOp::Recv { channel, .. } => {
                        rewrite_expr(&mut channel.node, channel.span, active);
                    }
                    SelectOp::Send { channel, value } => {
                        rewrite_expr(&mut channel.node, channel.span, active);
                        rewrite_expr(&mut value.node, value.span, active);
                    }
                }
                rewrite_block(&mut arm.body.node, active);
            }
            if let Some(def) = default {
                rewrite_block(&mut def.node, active);
            }
        }
        Stmt::Break | Stmt::Continue => {}
    }
}

fn rewrite_expr(expr: &mut Expr, span: Span, active: &HashSet<String>) {
    match expr {
        Expr::Ident(name) if active.contains(name) => {
            // Rewrite `logger` to `self.logger`
            let field_name = name.clone();
            *expr = Expr::FieldAccess {
                object: Box::new(Spanned::new(Expr::Ident("self".into()), span)),
                field: Spanned::new(field_name, span),
            };
        }
        Expr::Ident(_) => {}
        Expr::BinOp { lhs, rhs, .. } => {
            rewrite_expr(&mut lhs.node, lhs.span, active);
            rewrite_expr(&mut rhs.node, rhs.span, active);
        }
        Expr::UnaryOp { operand, .. } => {
            rewrite_expr(&mut operand.node, operand.span, active);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                rewrite_expr(&mut arg.node, arg.span, active);
            }
        }
        Expr::FieldAccess { object, .. } => {
            rewrite_expr(&mut object.node, object.span, active);
        }
        Expr::MethodCall { object, args, .. } => {
            rewrite_expr(&mut object.node, object.span, active);
            for arg in args {
                rewrite_expr(&mut arg.node, arg.span, active);
            }
        }
        Expr::StructLit { fields, .. } => {
            for (_, val) in fields {
                rewrite_expr(&mut val.node, val.span, active);
            }
        }
        Expr::ArrayLit { elements } => {
            for elem in elements {
                rewrite_expr(&mut elem.node, elem.span, active);
            }
        }
        Expr::Index { object, index } => {
            rewrite_expr(&mut object.node, object.span, active);
            rewrite_expr(&mut index.node, index.span, active);
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    rewrite_expr(&mut e.node, e.span, active);
                }
            }
        }
        Expr::EnumData { fields, .. } => {
            for (_, val) in fields {
                rewrite_expr(&mut val.node, val.span, active);
            }
        }
        Expr::Closure { params, body, .. } => {
            let mut inner = active.clone();
            for p in params.iter() {
                inner.remove(&p.name.node);
            }
            rewrite_block(&mut body.node, &inner);
        }
        Expr::Propagate { expr: inner } => {
            rewrite_expr(&mut inner.node, inner.span, active);
        }
        Expr::Catch { expr: inner, handler } => {
            rewrite_expr(&mut inner.node, inner.span, active);
            match handler {
                CatchHandler::Wildcard { var, body } => {
                    let mut inner_active = active.clone();
                    inner_active.remove(&var.node);
                    rewrite_block(&mut body.node, &inner_active);
                }
                CatchHandler::Shorthand(fb) => {
                    rewrite_expr(&mut fb.node, fb.span, active);
                }
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                rewrite_expr(&mut k.node, k.span, active);
                rewrite_expr(&mut v.node, v.span, active);
            }
        }
        Expr::SetLit { elements, .. } => {
            for elem in elements {
                rewrite_expr(&mut elem.node, elem.span, active);
            }
        }
        Expr::Cast { expr: inner, .. } => {
            rewrite_expr(&mut inner.node, inner.span, active);
        }
        Expr::Range { start, end, .. } => {
            rewrite_expr(&mut start.node, start.span, active);
            rewrite_expr(&mut end.node, end.span, active);
        }
        Expr::Spawn { call } => {
            rewrite_expr(&mut call.node, call.span, active);
        }
        Expr::NullPropagate { expr: inner } => {
            rewrite_expr(&mut inner.node, inner.span, active);
        }
        // Literals and non-rewritable expressions
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_)
        | Expr::EnumUnit { .. } | Expr::ClosureCreate { .. } | Expr::NoneLit => {}
    }
}
