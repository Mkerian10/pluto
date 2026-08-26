//! Hoist generic methods into top-level generic functions.
//!
//! A method with its own type parameters (`fn foo<T>(self, val: T) T` inside
//! `class C`) is moved out of the class and registered as a top-level generic
//! function named with the method mangling (`C$foo`), its `self` parameter
//! typed as the concrete class. From there the entire existing
//! generic-function machinery applies unchanged: registration, skolem
//! template checking, monomorphization, and template removal.
//!
//! Call sites remain `Expr::MethodCall`; type checking resolves them against
//! the hoisted template (`typeck/infer.rs`), and monomorphization rewrites
//! the method name to the instantiated suffix so codegen's usual
//! `mangle_method(class, method)` lookup finds the concrete copy.

use crate::diagnostics::CompileError;
use crate::parser::ast::{Program, TypeExpr};
use crate::span::Spanned;
use crate::typeck::env::mangle_method;

pub fn hoist_generic_methods(program: &mut Program) -> Result<(), CompileError> {
    // Generic methods are only supported on non-generic classes for now:
    // methods of a generic class monomorphize with the class, and mixing
    // class-level and method-level parameters is future work.
    for class in &program.classes {
        let c = &class.node;
        if !c.type_params.is_empty()
            && let Some(m) = c.methods.iter().find(|m| !m.node.type_params.is_empty())
        {
            return Err(CompileError::type_err(
                format!(
                    "generic method '{}' on generic class '{}' is not supported; use class-level type parameters",
                    m.node.name.node, c.name.node
                ),
                m.node.name.span,
            ));
        }
    }
    if let Some(app) = &program.app
        && let Some(m) = app.node.methods.iter().find(|m| !m.node.type_params.is_empty())
    {
        return Err(CompileError::type_err(
            format!("generic method '{}' is not supported on an app", m.node.name.node),
            m.node.name.span,
        ));
    }
    for stage in &program.stages {
        if let Some(m) = stage.node.methods.iter().find(|m| !m.node.type_params.is_empty()) {
            return Err(CompileError::type_err(
                format!("generic method '{}' is not supported on a stage", m.node.name.node),
                m.node.name.span,
            ));
        }
    }
    let mut hoisted: Vec<Spanned<crate::parser::ast::Function>> = Vec::new();
    for class in &mut program.classes {
        let c = &mut class.node;
        if c.methods.iter().all(|m| m.node.type_params.is_empty()) {
            continue;
        }
        let class_name = c.name.node.clone();
        let mut kept = Vec::new();
        for mut m in std::mem::take(&mut c.methods) {
            if m.node.type_params.is_empty() {
                kept.push(m);
                continue;
            }
            if m.node.params.is_empty() || m.node.params[0].name.node != "self" {
                return Err(CompileError::type_err(
                    format!(
                        "generic method '{}' must take self; static generic methods are not supported",
                        m.node.name.node
                    ),
                    m.node.name.span,
                ));
            }
            // Type the receiver concretely so free-function registration
            // resolves it like any other parameter.
            let self_span = m.node.params[0].ty.span;
            m.node.params[0].ty = Spanned::new(TypeExpr::Named(class_name.clone()), self_span);
            let name_span = m.node.name.span;
            m.node.name = Spanned::new(mangle_method(&class_name, &m.node.name.node), name_span);
            hoisted.push(m);
        }
        c.methods = kept;
    }
    program.functions.extend(hoisted);
    Ok(())
}
