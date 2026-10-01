//! Property instantiation — phase 4 of docs/design/rfc-properties.md
//! (slice 2, the `property` form).
//!
//! A `property` declaration is a named, parameterized bundle of proof atoms
//! (invariant clauses — single- or two-state — and `guarded_by` clauses). A
//! `satisfies` clause on a class/object instantiates the bundle by **pure
//! substitution**: the instantiated atoms join the type's own invariant and
//! guard sets and are discharged by the standard machinery
//! (src/typeck/discharge.rs, src/typeck/dominance.rs) — the property layer
//! never forks the provers.
//!
//! Judgmental only, generative never: an instantiation can reject a program;
//! it cannot change what the program does (the injected clauses are proof
//! obligations, not code).
//!
//! # Validation split
//!
//! - **Declaration time** (here, per `property` decl): structure, parameter
//!   kinds, arity of atoms over parameters. Invariant atoms may reference
//!   only `field<int>` and `const int` parameters (the invariant fragment is
//!   integer-only); guarded atoms name a `field`-kind target and may use the
//!   binder's fields, `field<int>` parameters, and `const int` parameters in
//!   the predicate. Property bodies are parametric: direct references to
//!   concrete fields (`self.x`) are rejected.
//! - **Instantiation time**: argument kinds against concrete fields/types,
//!   then — because the substituted clauses are ordinary clauses — the exact
//!   fragment validation the hand-written forms get, in
//!   `discharge::register_invariants` / `dominance::register_guards`, with
//!   property provenance attached for two-sided blame.
//!
//! Runs first in the frontend pipeline (after module flattening), so every
//! later pass sees the instantiated atoms as if they were hand-written —
//! including marshal decode validation of single-state invariants.

use std::collections::HashMap;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::{Span, Spanned};
use crate::visit::{walk_expr_mut, VisitMut};

/// Instantiate every `satisfies` clause in the program. See module docs.
pub fn instantiate_properties(program: &mut Program) -> Result<(), CompileError> {
    // Property name table (duplicate declarations rejected).
    let mut table: HashMap<String, usize> = HashMap::new();
    for (i, p) in program.properties.iter().enumerate() {
        if table.insert(p.node.name.node.clone(), i).is_some() {
            return Err(CompileError::type_err(
                format!("duplicate property declaration '{}'", p.node.name.node),
                p.node.name.span,
            ));
        }
    }

    for p in &program.properties {
        validate_property_decl(&p.node)?;
    }

    let properties = &program.properties;
    for class in &mut program.classes {
        if class.node.satisfies.is_empty() {
            continue;
        }
        // Generic classes are allowed: the substituted atoms are ordinary
        // clauses and get the generic contract validation (param-independent
        // vocabulary only) in discharge::register_invariants /
        // dominance::register_guards. Param-dependent arguments already fail
        // here: `field<int>` requires the field's declared type to be `int`,
        // which a param-typed field is not.
        let clauses = class.node.satisfies.clone();
        for clause in &clauses {
            let Some(&idx) = table.get(&clause.node.name.node) else {
                return Err(CompileError::type_err(
                    format!(
                        "unknown property '{}': no such property declaration is in scope \
                         (properties are declared with 'property name(params) {{ ... }}' \
                         and imported like any other declaration)",
                        clause.node.name.node
                    ),
                    clause.node.name.span,
                ));
            };
            let prop = &properties[idx].node;
            instantiate_clause(&mut class.node, clause, prop)?;
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Declaration-time validation
// ─────────────────────────────────────────────────────────────────────────────

fn param_kind_name(kind: &PropertyParamKind) -> String {
    match kind {
        PropertyParamKind::Field { ty: Some(ty) } => {
            format!("field<{}>", format_type_expr(&ty.node))
        }
        PropertyParamKind::Field { ty: None } => "field".to_string(),
        PropertyParamKind::Type => "type".to_string(),
        PropertyParamKind::ConstInt => "const int".to_string(),
    }
}

fn is_int_field_kind(kind: &PropertyParamKind) -> bool {
    matches!(
        kind,
        PropertyParamKind::Field { ty: Some(t) } if t.node == TypeExpr::Named("int".to_string())
    )
}

fn validate_property_decl(prop: &PropertyDecl) -> Result<(), CompileError> {
    let mut seen: HashMap<&str, ()> = HashMap::new();
    for param in &prop.params {
        if seen.insert(param.name.node.as_str(), ()).is_some() {
            return Err(CompileError::type_err(
                format!(
                    "duplicate parameter '{}' in property '{}'",
                    param.name.node, prop.name.node
                ),
                param.name.span,
            ));
        }
    }
    if prop.atoms.is_empty() {
        return Err(CompileError::type_err(
            format!(
                "property '{}' has an empty body: a property is a conjunction of at least \
                 one proof atom (an 'invariant' clause or a 'guarded_by' clause)",
                prop.name.node
            ),
            prop.name.span,
        ));
    }
    let params: HashMap<&str, &PropertyParam> =
        prop.params.iter().map(|p| (p.name.node.as_str(), p)).collect();

    for atom in &prop.atoms {
        match &atom.node.kind {
            PropertyAtomKind::Invariant { expr } => {
                validate_atom_expr(expr, prop, &params, None)?;
            }
            PropertyAtomKind::Guarded { target, clause } => {
                match params.get(target.node.as_str()) {
                    Some(p) if matches!(p.kind.node, PropertyParamKind::Field { .. }) => {}
                    Some(p) => {
                        return Err(CompileError::type_err(
                            format!(
                                "guarded_by target '{}' in property '{}' has kind '{}': the \
                                 target of a guard atom must be a 'field' parameter",
                                target.node,
                                prop.name.node,
                                param_kind_name(&p.kind.node)
                            ),
                            target.span,
                        ));
                    }
                    None => {
                        return Err(CompileError::type_err(
                            format!(
                                "unknown name '{}' in property '{}': the guarded_by target \
                                 must be one of the property's 'field' parameters",
                                target.node, prop.name.node
                            ),
                            target.span,
                        ));
                    }
                }
                if params.contains_key(clause.binder.node.as_str()) {
                    return Err(CompileError::type_err(
                        format!(
                            "guard binder '{}' in property '{}' shadows a property \
                             parameter; rename one of them",
                            clause.binder.node, prop.name.node
                        ),
                        clause.binder.span,
                    ));
                }
                match &clause.binder_ty.node {
                    TypeExpr::Named(n) => {
                        if let Some(p) = params.get(n.as_str()) {
                            if p.kind.node != PropertyParamKind::Type {
                                return Err(CompileError::type_err(
                                    format!(
                                        "guard binder type '{}' in property '{}' names a \
                                         parameter of kind '{}': a binder type parameter \
                                         must have kind 'type'",
                                        n,
                                        prop.name.node,
                                        param_kind_name(&p.kind.node)
                                    ),
                                    clause.binder_ty.span,
                                ));
                            }
                        }
                        // A non-parameter name is a concrete class, validated
                        // (as a value class, non-entity) at guard registration.
                    }
                    _ => {
                        return Err(CompileError::type_err(
                            format!(
                                "guard binder type in property '{}' must be a named type — \
                                 either a 'type' parameter or a concrete value class",
                                prop.name.node
                            ),
                            clause.binder_ty.span,
                        ));
                    }
                }
                validate_atom_expr(&clause.predicate, prop, &params, Some(&clause.binder.node))?;
            }
        }
    }
    Ok(())
}

/// Validate the names an atom expression references. Structure beyond naming
/// (linearity, comparison shape) is left to the standard fragment validators,
/// which run on the substituted clause at instantiation time with provenance
/// attached.
fn validate_atom_expr(
    expr: &Spanned<Expr>,
    prop: &PropertyDecl,
    params: &HashMap<&str, &PropertyParam>,
    binder: Option<&str>,
) -> Result<(), CompileError> {
    match &expr.node {
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) => Ok(()),
        Expr::Ident(name) => match params.get(name.as_str()) {
            Some(p)
                if is_int_field_kind(&p.kind.node)
                    || p.kind.node == PropertyParamKind::ConstInt =>
            {
                Ok(())
            }
            Some(p) => Err(CompileError::type_err(
                format!(
                    "parameter '{}' of property '{}' has kind '{}' and cannot appear in \
                     this position: only 'field<int>' and 'const int' parameters are \
                     terms of the integer proof fragment",
                    name,
                    prop.name.node,
                    param_kind_name(&p.kind.node)
                ),
                expr.span,
            )),
            None if Some(name.as_str()) == binder => Err(CompileError::type_err(
                format!(
                    "guard binder '{}' used as a bare term in property '{}': compare the \
                     binder's int fields (e.g. '{}.token'), not the binder itself",
                    name, prop.name.node, name
                ),
                expr.span,
            )),
            None => Err(CompileError::type_err(
                format!(
                    "unknown name '{}' in the body of property '{}': property bodies are \
                     parametric — every free name must be one of the declared parameters{}",
                    name,
                    prop.name.node,
                    if binder.is_some() { " or the guard binder" } else { "" }
                ),
                expr.span,
            )),
        },
        Expr::FieldAccess { object, field: _ } => {
            match (&object.node, binder) {
                (Expr::Ident(root), Some(b)) if root == b => Ok(()),
                (Expr::Ident(root), _) if root == "self" => Err(CompileError::type_err(
                    format!(
                        "property '{}' references 'self' directly: property bodies are \
                         parametric over the carrying type — name the field through a \
                         'field' parameter instead",
                        prop.name.node
                    ),
                    expr.span,
                )),
                _ => Err(CompileError::type_err(
                    format!(
                        "unsupported field access in the body of property '{}': only \
                         one-level int fields of the guard binder are usable here",
                        prop.name.node
                    ),
                    expr.span,
                )),
            }
        }
        Expr::QualifiedAccess { segments } => {
            // Pre-resolution shape of a dotted path (resolution rewrites these
            // to FieldAccess chains); validate the root the same way.
            match (segments.first(), binder) {
                (Some(root), Some(b)) if root.node == *b && segments.len() == 2 => Ok(()),
                (Some(root), _) if root.node == "self" => Err(CompileError::type_err(
                    format!(
                        "property '{}' references 'self' directly: property bodies are \
                         parametric over the carrying type — name the field through a \
                         'field' parameter instead",
                        prop.name.node
                    ),
                    expr.span,
                )),
                _ => Err(CompileError::type_err(
                    format!(
                        "unsupported dotted path in the body of property '{}': only \
                         one-level int fields of the guard binder are usable here",
                        prop.name.node
                    ),
                    expr.span,
                )),
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            validate_atom_expr(lhs, prop, params, binder)?;
            validate_atom_expr(rhs, prop, params, binder)
        }
        Expr::UnaryOp { operand, .. } => validate_atom_expr(operand, prop, params, binder),
        Expr::Call { .. } => {
            if let Some(inner) = old_call_arg(&expr.node) {
                validate_atom_expr(inner, prop, params, binder)
            } else {
                Err(CompileError::type_err(
                    format!(
                        "calls are not part of the property body language (property '{}'); \
                         only the 'old(...)' intrinsic is allowed",
                        prop.name.node
                    ),
                    expr.span,
                ))
            }
        }
        _ => Err(CompileError::type_err(
            format!(
                "unsupported expression in the body of property '{}': atoms are built from \
                 &&, ||, ! over integer comparisons of linear arithmetic over the \
                 property's parameters",
                prop.name.node
            ),
            expr.span,
        )),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Instantiation
// ─────────────────────────────────────────────────────────────────────────────

/// The resolved value of one meta-argument.
enum ArgValue {
    /// A field of the carrying type (its declared name).
    Field(String),
    /// A type name (dotted if module-qualified).
    Type(String),
    /// An integer constant.
    Const(i64),
}

impl ArgValue {
    fn render(&self) -> String {
        match self {
            ArgValue::Field(f) => format!("self.{f}"),
            ArgValue::Type(t) => t.clone(),
            ArgValue::Const(n) => n.to_string(),
        }
    }
}

fn instantiate_clause(
    class: &mut ClassDecl,
    clause: &Spanned<SatisfiesClause>,
    prop: &PropertyDecl,
) -> Result<(), CompileError> {
    let cname = &clause.node.name.node;
    if clause.node.args.len() != prop.params.len() {
        let sig = prop
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name.node, param_kind_name(&p.kind.node)))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(CompileError::type_err(
            format!(
                "property '{cname}' expects {} argument{} ({sig}), found {}",
                prop.params.len(),
                if prop.params.len() == 1 { "" } else { "s" },
                clause.node.args.len()
            ),
            clause.span,
        ));
    }

    // Resolve and kind-check each argument.
    let mut values: Vec<ArgValue> = Vec::new();
    for (param, arg) in prop.params.iter().zip(&clause.node.args) {
        let value = resolve_arg(class, cname, param, arg)?;
        values.push(value);
    }

    let bindings = prop
        .params
        .iter()
        .zip(&values)
        .map(|(p, v)| format!("{} = {}", p.name.node, v.render()))
        .collect::<Vec<_>>()
        .join(", ");

    // Substitution map over the body's free names.
    let mut subst: HashMap<String, Expr> = HashMap::new();
    let mut type_subst: HashMap<String, String> = HashMap::new();
    for (param, value) in prop.params.iter().zip(&values) {
        match value {
            ArgValue::Field(f) => {
                subst.insert(
                    param.name.node.clone(),
                    Expr::FieldAccess {
                        object: Box::new(Spanned::new(Expr::Ident("self".to_string()), clause.span)),
                        field: Spanned::new(f.clone(), clause.span),
                    },
                );
            }
            ArgValue::Const(n) => {
                subst.insert(param.name.node.clone(), Expr::IntLit(*n));
            }
            ArgValue::Type(t) => {
                type_subst.insert(param.name.node.clone(), t.clone());
            }
        }
    }

    for atom in &prop.atoms {
        let provenance = PropertyProvenance {
            property: prop.name.node.clone(),
            line: atom.node.line,
            bindings: bindings.clone(),
        };
        match &atom.node.kind {
            PropertyAtomKind::Invariant { expr } => {
                let mut expr = expr.clone();
                substitute_expr(&mut expr, &subst, clause.span);
                class.invariants.push(Spanned::new(
                    ContractClause {
                        kind: ContractKind::Invariant,
                        expr,
                        provenance: Some(provenance),
                    },
                    clause.span,
                ));
            }
            PropertyAtomKind::Guarded { target, clause: guard } => {
                let field_name = match prop
                    .params
                    .iter()
                    .zip(&values)
                    .find(|(p, _)| p.name.node == target.node)
                {
                    Some((_, ArgValue::Field(f))) => f.clone(),
                    _ => unreachable!("declaration validation checked the target is a field param"),
                };
                let mut predicate = guard.predicate.clone();
                substitute_expr(&mut predicate, &subst, clause.span);
                let binder_ty = match &guard.binder_ty.node {
                    TypeExpr::Named(n) => {
                        let resolved = type_subst.get(n).cloned().unwrap_or_else(|| n.clone());
                        Spanned::new(TypeExpr::Named(resolved), clause.span)
                    }
                    other => Spanned::new(other.clone(), clause.span),
                };
                let field = class
                    .fields
                    .iter_mut()
                    .find(|f| f.name.node == field_name)
                    .expect("resolve_arg checked the field exists");
                if let Some(existing) = &field.guarded_by {
                    let origin = match &existing.provenance {
                        Some(p) => format!("by property '{}'", p.property),
                        None => "by a hand-written guarded_by clause".to_string(),
                    };
                    return Err(CompileError::type_err(
                        format!(
                            "field '{}' of '{}' is already guarded {origin}: a field \
                             carries at most one guarded_by clause",
                            field_name, class.name.node
                        ),
                        clause.span,
                    ));
                }
                field.guarded_by = Some(GuardClause {
                    binder: Spanned::new(guard.binder.node.clone(), clause.span),
                    binder_ty,
                    predicate,
                    provenance: Some(provenance),
                });
            }
        }
    }
    Ok(())
}

/// Resolve one satisfies argument against its parameter's kind.
fn resolve_arg(
    class: &ClassDecl,
    prop_name: &str,
    param: &PropertyParam,
    arg: &Spanned<Expr>,
) -> Result<ArgValue, CompileError> {
    match &param.kind.node {
        PropertyParamKind::Field { ty } => {
            let field_name = match &arg.node {
                Expr::FieldAccess { object, field }
                    if matches!(&object.node, Expr::Ident(s) if s == "self") =>
                {
                    field.node.clone()
                }
                // Pre-resolution shape of `self.<field>` (dotted paths parse
                // as QualifiedAccess until module resolution rewrites them).
                Expr::QualifiedAccess { segments }
                    if segments.len() == 2 && segments[0].node == "self" =>
                {
                    segments[1].node.clone()
                }
                _ => {
                    return Err(CompileError::type_err(
                        format!(
                            "argument for parameter '{}: {}' of property '{prop_name}' must \
                             be a field of the carrying type, written 'self.<field>'",
                            param.name.node,
                            param_kind_name(&param.kind.node)
                        ),
                        arg.span,
                    ));
                }
            };
            let Some(field) = class.fields.iter().find(|f| f.name.node == field_name) else {
                return Err(CompileError::type_err(
                    format!(
                        "'{}' has no field '{field_name}' (argument for parameter '{}' of \
                         property '{prop_name}')",
                        class.name.node, param.name.node
                    ),
                    arg.span,
                ));
            };
            if let Some(expected) = ty {
                if field.ty.node != expected.node {
                    return Err(CompileError::type_err(
                        format!(
                            "parameter '{}' of property '{prop_name}' requires a field of \
                             type {}, but field '{field_name}' of '{}' has type {}",
                            param.name.node,
                            format_type_expr(&expected.node),
                            class.name.node,
                            format_type_expr(&field.ty.node)
                        ),
                        arg.span,
                    ));
                }
            }
            Ok(ArgValue::Field(field_name))
        }
        PropertyParamKind::Type => match type_name_of_expr(&arg.node) {
            Some(name) => Ok(ArgValue::Type(name)),
            None => Err(CompileError::type_err(
                format!(
                    "argument for parameter '{}: type' of property '{prop_name}' must be a \
                     type name",
                    param.name.node
                ),
                arg.span,
            )),
        },
        PropertyParamKind::ConstInt => match const_int_of_expr(&arg.node) {
            Some(n) => Ok(ArgValue::Const(n)),
            None => Err(CompileError::type_err(
                format!(
                    "argument for parameter '{}: const int' of property '{prop_name}' must \
                     be an integer literal",
                    param.name.node
                ),
                arg.span,
            )),
        },
    }
}

/// A dotted type name from an argument expression (`Grant`, `wire.Grant`).
fn type_name_of_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(n) => Some(n.clone()),
        Expr::FieldAccess { object, field } => {
            type_name_of_expr(&object.node).map(|base| format!("{base}.{}", field.node))
        }
        Expr::QualifiedAccess { segments } => Some(
            segments
                .iter()
                .map(|s| s.node.as_str())
                .collect::<Vec<_>>()
                .join("."),
        ),
        _ => None,
    }
}

/// Render a satisfies argument as written (`self.epoch`, `WriteGrant`, `3`).
/// Used for the provided-properties registry and blame bindings.
pub(crate) fn render_satisfies_arg(expr: &Expr) -> String {
    match expr {
        Expr::Ident(n) => n.clone(),
        Expr::IntLit(n) => n.to_string(),
        Expr::UnaryOp { op: UnaryOp::Neg, operand } => {
            format!("-{}", render_satisfies_arg(&operand.node))
        }
        Expr::FieldAccess { object, field } => {
            format!("{}.{}", render_satisfies_arg(&object.node), field.node)
        }
        Expr::QualifiedAccess { segments } => segments
            .iter()
            .map(|s| s.node.as_str())
            .collect::<Vec<_>>()
            .join("."),
        _ => "<expr>".to_string(),
    }
}

fn const_int_of_expr(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::IntLit(n) => Some(*n),
        Expr::UnaryOp { op: UnaryOp::Neg, operand } => const_int_of_expr(&operand.node).map(|n| -n),
        _ => None,
    }
}

fn format_type_expr(te: &TypeExpr) -> String {
    match te {
        TypeExpr::Named(n) => n.clone(),
        TypeExpr::Array(inner) => format!("[{}]", format_type_expr(&inner.node)),
        TypeExpr::Qualified { module, name } => format!("{module}.{name}"),
        TypeExpr::Generic { name, type_args } => format!(
            "{name}<{}>",
            type_args
                .iter()
                .map(|t| format_type_expr(&t.node))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeExpr::Nullable(inner) => format!("{}?", format_type_expr(&inner.node)),
        TypeExpr::Stream(inner) => format!("stream {}", format_type_expr(&inner.node)),
        TypeExpr::Fn { params, return_type, fallible } => format!(
            "fn({}) {}{}",
            params
                .iter()
                .map(|t| format_type_expr(&t.node))
                .collect::<Vec<_>>()
                .join(", "),
            format_type_expr(&return_type.node),
            if *fallible { "!" } else { "" }
        ),
        TypeExpr::Infer => "_".to_string(),
    }
}

/// Substitute parameter names in a body expression and re-span every node to
/// the satisfies clause (so fragment/discharge diagnostics point at the
/// instantiation site in the user's file, with the property-body side carried
/// by the provenance blame suffix).
fn substitute_expr(expr: &mut Spanned<Expr>, subst: &HashMap<String, Expr>, span: Span) {
    struct Subst<'a> {
        subst: &'a HashMap<String, Expr>,
        span: Span,
    }
    impl VisitMut for Subst<'_> {
        fn visit_expr_mut(&mut self, expr: &mut Spanned<Expr>) {
            expr.span = self.span;
            if let Expr::Ident(name) = &expr.node {
                if let Some(replacement) = self.subst.get(name) {
                    expr.node = replacement.clone();
                    return; // replacements carry the clause span already
                }
            }
            walk_expr_mut(self, expr);
        }
    }
    let mut s = Subst { subst, span };
    s.visit_expr_mut(expr);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    fn parse(src: &str) -> Program {
        let tokens = crate::lexer::lex(src).expect("lex");
        let mut p = Parser::new(&tokens, src);
        p.parse_program().expect("parse")
    }

    #[test]
    fn monotonic_instantiation_injects_invariant() {
        let src = "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nclass C satisfies monotonic(self.epoch) {\n    epoch: int\n}\n";
        let mut program = parse(src);
        instantiate_properties(&mut program).expect("instantiation succeeds");
        let class = &program.classes[0].node;
        assert_eq!(class.invariants.len(), 1);
        let inv = &class.invariants[0].node;
        assert_eq!(inv.kind, ContractKind::Invariant);
        let prov = inv.provenance.as_ref().expect("provenance recorded");
        assert_eq!(prov.property, "monotonic");
        assert_eq!(prov.line, 2);
        assert_eq!(prov.bindings, "f = self.epoch");
        // f substituted to self.epoch on both sides (incl. inside old()).
        let rendered = crate::codegen::format_invariant_expr(&inv.expr.node);
        assert_eq!(rendered, "self.epoch >= old(self.epoch)");
    }

    #[test]
    fn fenced_instantiation_injects_guard() {
        let src = "property fenced(f: field, authority: field<int>, grant: type) {\n    f guarded_by (g: grant) g.token == authority\n}\n\nclass Grant {\n    token: int\n}\n\nclass C satisfies fenced(self.data, self.epoch, Grant) {\n    data: int\n    epoch: int\n}\n";
        let mut program = parse(src);
        // Mirror the pipeline: dotted paths are resolved before instantiation.
        crate::modules::resolve_qualified_access_single_file(&mut program).unwrap();
        instantiate_properties(&mut program).expect("instantiation succeeds");
        let class = program.classes.iter().find(|c| c.node.name.node == "C").unwrap();
        let field = class.node.fields.iter().find(|f| f.name.node == "data").unwrap();
        let guard = field.guarded_by.as_ref().expect("guard injected");
        assert_eq!(guard.binder.node, "g");
        assert_eq!(guard.binder_ty.node, TypeExpr::Named("Grant".to_string()));
        assert!(guard.provenance.is_some());
        let rendered = crate::codegen::format_invariant_expr(&guard.predicate.node);
        assert_eq!(rendered, "g.token == self.epoch");
    }

    #[test]
    fn arity_mismatch_rejected() {
        let src = "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nclass C satisfies monotonic(self.a, self.b) {\n    a: int\n    b: int\n}\n";
        let mut program = parse(src);
        let err = instantiate_properties(&mut program).unwrap_err();
        assert!(err.to_string().contains("expects 1 argument"), "got: {err}");
    }

    #[test]
    fn field_kind_mismatch_rejected() {
        let src = "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nclass C satisfies monotonic(self.name) {\n    name: string\n}\n";
        let mut program = parse(src);
        let err = instantiate_properties(&mut program).unwrap_err();
        assert!(
            err.to_string().contains("requires a field of type int"),
            "got: {err}"
        );
    }

    #[test]
    fn unknown_property_rejected() {
        let src = "class C satisfies nope(self.a) {\n    a: int\n}\n";
        let mut program = parse(src);
        let err = instantiate_properties(&mut program).unwrap_err();
        assert!(err.to_string().contains("unknown property 'nope'"), "got: {err}");
    }

    #[test]
    fn generic_class_satisfies_injects_invariant() {
        let src = "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nclass Box<T> satisfies monotonic(self.n) {\n    n: int\n}\n";
        let mut program = parse(src);
        instantiate_properties(&mut program).expect("generic instantiation succeeds");
        let class = &program.classes[0].node;
        assert_eq!(class.invariants.len(), 1);
        let rendered = crate::codegen::format_invariant_expr(&class.invariants[0].node.expr.node);
        assert_eq!(rendered, "self.n >= old(self.n)");
    }

    #[test]
    fn generic_class_satisfies_param_typed_field_rejected() {
        // The kind check carries the param-independence rule: a field whose
        // declared type is a type parameter is not a field of type int.
        let src = "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nclass Box<T> satisfies monotonic(self.x) {\n    x: T\n}\n";
        let mut program = parse(src);
        let err = instantiate_properties(&mut program).unwrap_err();
        assert!(
            err.to_string().contains("requires a field of type int"),
            "got: {err}"
        );
    }

    #[test]
    fn self_reference_in_body_rejected() {
        let src = "property bad(f: field<int>) {\n    invariant self.x >= 0\n}\n";
        let mut program = parse(src);
        let err = instantiate_properties(&mut program).unwrap_err();
        assert!(err.to_string().contains("parametric"), "got: {err}");
    }

    #[test]
    fn unknown_name_in_body_rejected() {
        let src = "property bad(f: field<int>) {\n    invariant g >= 0\n}\n";
        let mut program = parse(src);
        let err = instantiate_properties(&mut program).unwrap_err();
        assert!(err.to_string().contains("unknown name 'g'"), "got: {err}");
    }

    #[test]
    fn const_int_substitutes() {
        let src = "property bounded(f: field<int>, lo: const int) {\n    invariant f >= lo\n}\n\nclass C satisfies bounded(self.n, 3) {\n    n: int\n}\n";
        let mut program = parse(src);
        instantiate_properties(&mut program).expect("instantiation succeeds");
        let inv = &program.classes[0].node.invariants[0].node;
        let rendered = crate::codegen::format_invariant_expr(&inv.expr.node);
        assert_eq!(rendered, "self.n >= 3");
        assert_eq!(inv.provenance.as_ref().unwrap().bindings, "f = self.n, lo = 3");
    }
}
