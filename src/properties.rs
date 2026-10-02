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

    // Phase 5 (rfc-properties.md): fn-level `provides`, extern `assume`,
    // and fn-type property requirements.
    process_fn_provides(program, &table)?;
    process_extern_assumes(program, &table)?;
    validate_fn_type_provides(program, &table)?;
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
        PropertyParamKind::Expr => "expr".to_string(),
    }
}

/// The short (unqualified) property name for diagnostics.
fn short_name(name: &str) -> &str {
    name.rsplit_once('.').map(|(_, s)| s).unwrap_or(name)
}

/// Does this property's body contain type-level atoms (invariant /
/// guarded_by — instantiated on a class/object via `satisfies`)?
fn has_type_level_atoms(prop: &PropertyDecl) -> bool {
    prop.atoms.iter().any(|a| {
        matches!(
            a.node.kind,
            PropertyAtomKind::Invariant { .. } | PropertyAtomKind::Guarded { .. }
        )
    })
}

/// Does this property's body contain method-level atoms (ensures / dedup —
/// instantiated on a class/object method via `provides`)?
fn has_method_level_atoms(prop: &PropertyDecl) -> bool {
    prop.atoms.iter().any(|a| {
        matches!(
            a.node.kind,
            PropertyAtomKind::Ensures { .. } | PropertyAtomKind::Dedup { .. }
        )
    })
}

pub(crate) fn has_dedup_atoms(prop: &PropertyDecl) -> bool {
    prop.atoms.iter().any(|a| matches!(a.node.kind, PropertyAtomKind::Dedup { .. }))
}

/// The dedup obligation a `provides` clause of a dedup-shaped property puts
/// on its providing method (consumed by src/typeck/idempotency.rs): the
/// instantiated key as a path over the method's parameters, plus the atom's
/// source line for two-sided blame. `None` when the property carries no
/// dedup atom. Argument shapes were validated by `validate_provides_args`.
pub(crate) struct DedupKeySpec {
    /// The key path's root: a parameter name of the providing method.
    pub root: String,
    /// One-level field of the root (`req.id` → `Some("id")`).
    pub field: Option<String>,
    /// 1-based line of the dedup atom in the property's defining file.
    pub atom_line: usize,
}

pub(crate) fn dedup_key_spec(prop: &PropertyDecl, clause: &ProvidesClause) -> Option<DedupKeySpec> {
    let (key_param, atom_line) = prop.atoms.iter().find_map(|a| match &a.node.kind {
        PropertyAtomKind::Dedup { key } => Some((key.node.clone(), a.node.line)),
        _ => None,
    })?;
    let idx = prop.params.iter().position(|p| p.name.node == key_param)?;
    let arg = clause.args.get(idx)?;
    match &arg.value.node {
        Expr::Ident(name) => Some(DedupKeySpec { root: name.clone(), field: None, atom_line }),
        Expr::FieldAccess { object, field } => match &object.node {
            Expr::Ident(root) => Some(DedupKeySpec {
                root: root.clone(),
                field: Some(field.node.clone()),
                atom_line,
            }),
            _ => None,
        },
        Expr::QualifiedAccess { segments } if segments.len() == 2 => Some(DedupKeySpec {
            root: segments[0].node.clone(),
            field: Some(segments[1].node.clone()),
            atom_line,
        }),
        _ => None,
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
    // An empty body is a DECLARED-ONLY property (rfc-properties.md phase
    // 5): its proof shape is not yet expressible in the kernel, so it has
    // no in-unit discharge path — it can only be assumed at an extern
    // boundary. Both `satisfies` and in-unit `provides` of it are rejected
    // at the instantiation site (never silently promoted, and never
    // vacuously satisfied).
    if has_type_level_atoms(prop) && has_method_level_atoms(prop) {
        return Err(CompileError::type_err(
            format!(
                "property '{}' mixes type-level atoms (invariant / guarded_by) with \
                 method-level atoms (ensures / dedup): a property is instantiated either \
                 on a type ('satisfies') or on a method ('provides') — split it into two \
                 properties",
                prop.name.node
            ),
            prop.name.span,
        ));
    }
    // One discharge shape per property: `ensures` atoms substitute into
    // proven postconditions, `dedup` is the checked guard-placement proof —
    // mixing them would give one claim two discharge modes. And one dedup
    // atom per property: a single operation dedups on a single key.
    let dedup_count = prop
        .atoms
        .iter()
        .filter(|a| matches!(a.node.kind, PropertyAtomKind::Dedup { .. }))
        .count();
    if dedup_count > 0 && prop.atoms.len() != dedup_count {
        return Err(CompileError::type_err(
            format!(
                "property '{}' mixes 'dedup' with other atoms: dedup is the checked \
                 dedup-guard proof shape and stands alone — split the property",
                prop.name.node
            ),
            prop.name.span,
        ));
    }
    if dedup_count > 1 {
        return Err(CompileError::type_err(
            format!(
                "property '{}' declares {} dedup atoms: a property carries at most one \
                 (a deduplicated operation has a single key)",
                prop.name.node, dedup_count
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
            PropertyAtomKind::Ensures { expr } => {
                // Same integer fragment as invariant atoms: field<int> and
                // const int parameters, plus old(...) of those. `expr`
                // parameters stay out of atoms in slice 1.
                validate_atom_expr(expr, prop, &params, None)?;
            }
            PropertyAtomKind::Dedup { key } => {
                // The one place an `expr` parameter is consumed by an atom:
                // the dedup key names the instantiation expression the
                // providing method's dedup guard must be keyed on.
                match params.get(key.node.as_str()) {
                    Some(p) if p.kind.node == PropertyParamKind::Expr => {}
                    Some(p) => {
                        return Err(CompileError::type_err(
                            format!(
                                "dedup key '{}' in property '{}' has kind '{}': the key of \
                                 a dedup atom must be an 'expr' parameter (it ranges over \
                                 the providing method's parameters)",
                                key.node,
                                prop.name.node,
                                param_kind_name(&p.kind.node)
                            ),
                            key.span,
                        ));
                    }
                    None => {
                        return Err(CompileError::type_err(
                            format!(
                                "unknown name '{}' in property '{}': the dedup key must be \
                                 one of the property's 'expr' parameters",
                                key.node, prop.name.node
                            ),
                            key.span,
                        ));
                    }
                }
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
            Some(p) if p.kind.node == PropertyParamKind::Expr => Err(CompileError::type_err(
                format!(
                    "parameter '{}' of property '{}' has kind 'expr' and cannot appear \
                     inside the property's atoms: expr parameters are instantiation \
                     metadata, bound at fn-level 'provides' / extern 'assume' sites \
                     (rfc-properties.md phase 5)",
                    name, prop.name.node
                ),
                expr.span,
            )),
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
    if prop.atoms.is_empty() {
        return Err(CompileError::type_err(
            format!(
                "property '{}' has no checkable atoms (a declared-only property): there \
                 is no in-unit discharge for it, so 'satisfies' cannot instantiate it — \
                 declared-only properties are claimed only at extern boundaries, with \
                 'assume {}(...)' on an extern fn (recorded on the assumption surface)",
                short_name(cname),
                short_name(cname)
            ),
            clause.span,
        ));
    }
    if has_method_level_atoms(prop) {
        return Err(CompileError::type_err(
            format!(
                "property '{}' contains method-level atoms (ensures / dedup): it is \
                 provided by a class/object method ('fn m(mut self, ...) provides \
                 {}(...)'), not satisfied by a type",
                short_name(cname),
                short_name(cname)
            ),
            clause.span,
        ));
    }
    if let Some(p) = prop
        .params
        .iter()
        .find(|p| p.kind.node == PropertyParamKind::Expr)
    {
        return Err(CompileError::type_err(
            format!(
                "property '{}' has an 'expr' parameter ('{}'): expr parameters range \
                 over a function's parameters, so the property attaches at fn-level \
                 'provides' / extern 'assume' sites — 'satisfies' cannot supply it",
                short_name(cname),
                p.name.node
            ),
            clause.span,
        ));
    }
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
            PropertyAtomKind::Ensures { .. } | PropertyAtomKind::Dedup { .. } => {
                unreachable!("method-level-atom properties are rejected for 'satisfies' above")
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
        PropertyParamKind::Expr => Err(CompileError::type_err(
            format!(
                "parameter '{}: expr' of property '{prop_name}' cannot be supplied by a \
                 'satisfies' clause (expr parameters bind at fn-level provides/assume \
                 sites)",
                param.name.node
            ),
            arg.span,
        )),
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
        TypeExpr::Fn { params, return_type, fallible, provides } => format!(
            "fn({}) {}{}{}",
            params
                .iter()
                .map(|t| format_type_expr(&t.node))
                .collect::<Vec<_>>()
                .join(", "),
            format_type_expr(&return_type.node),
            if *fallible { "!" } else { "" },
            provides
                .iter()
                .map(|p| format!(" provides {p}"))
                .collect::<String>()
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


// ─────────────────────────────────────────────────────────────────────────────
// Phase 5: fn-level provides, extern assume, fn-type requirements
// (docs/design/rfc-properties.md phase 5)
// ─────────────────────────────────────────────────────────────────────────────

/// Render the instantiation bindings of a provides/assume clause
/// (`key = req.id` / `f = self.count, by = 1`) for provenance, the
/// fn-properties registry, and the assumption surface.
pub(crate) fn render_provides_args(clause: &ProvidesClause) -> String {
    clause
        .args
        .iter()
        .map(|a| match &a.name {
            Some(n) => format!("{} = {}", n.node, render_satisfies_arg(&a.value.node)),
            None => render_satisfies_arg(&a.value.node),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn lookup_property<'a>(
    table: &HashMap<String, usize>,
    properties: &'a [Spanned<PropertyDecl>],
    name: &Spanned<String>,
) -> Result<&'a PropertyDecl, CompileError> {
    match table.get(&name.node) {
        Some(&idx) => Ok(&properties[idx].node),
        None => Err(CompileError::type_err(
            format!(
                "unknown property '{}': no such property declaration is in scope \
                 (properties are declared with 'property name(params) {{ ... }}' \
                 and imported like any other declaration)",
                name.node
            ),
            name.span,
        )),
    }
}

/// What a provides/assume clause is attached to, for argument resolution
/// and diagnostics.
struct ProvidesTarget<'a> {
    /// The receiver class when the provider is a class/object method
    /// (field-kind arguments resolve against it).
    class: Option<&'a ClassDecl>,
    /// The provider's parameter names, excluding `self` (expr-kind
    /// arguments range over these).
    params: Vec<String>,
    /// `function 'f'` / `method 'C.m'` / `extern fn 'f'` for diagnostics.
    desc: String,
}

/// Validate one provides/assume clause's arguments against the property's
/// parameter kinds and return the substitution for ensures-atom injection
/// (field params → `self.<field>`, const params → literals; expr params
/// bind no atom terms in slice 1).
fn validate_provides_args(
    clause: &Spanned<ProvidesClause>,
    prop: &PropertyDecl,
    target: &ProvidesTarget,
) -> Result<HashMap<String, Expr>, CompileError> {
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
                "property '{}' expects {} argument{} ({sig}), found {}",
                short_name(cname),
                prop.params.len(),
                if prop.params.len() == 1 { "" } else { "s" },
                clause.node.args.len()
            ),
            clause.span,
        ));
    }

    let mut subst: HashMap<String, Expr> = HashMap::new();
    for (param, arg) in prop.params.iter().zip(&clause.node.args) {
        let is_expr_param = param.kind.node == PropertyParamKind::Expr;
        match (&arg.name, is_expr_param) {
            (None, true) => {
                return Err(CompileError::type_err(
                    format!(
                        "parameter '{}: expr' of property '{}' takes the named form: \
                         write '{} = <expr over the function's parameters>'",
                        param.name.node,
                        short_name(cname),
                        param.name.node
                    ),
                    arg.value.span,
                ));
            }
            (Some(n), true) if n.node != param.name.node => {
                return Err(CompileError::type_err(
                    format!(
                        "named argument '{}' does not match parameter '{}' of property \
                         '{}' at this position (arguments are matched positionally; the \
                         name documents the expr parameter being bound)",
                        n.node,
                        param.name.node,
                        short_name(cname)
                    ),
                    n.span,
                ));
            }
            (Some(n), false) => {
                return Err(CompileError::type_err(
                    format!(
                        "parameter '{}: {}' of property '{}' is positional: the named \
                         form ('{} = ...') is reserved for 'expr' parameters",
                        param.name.node,
                        param_kind_name(&param.kind.node),
                        short_name(cname),
                        n.node
                    ),
                    n.span,
                ));
            }
            _ => {}
        }

        match &param.kind.node {
            PropertyParamKind::Expr => {
                validate_expr_arg(&arg.value, cname, param, target)?;
            }
            PropertyParamKind::Field { .. } => {
                let Some(class) = target.class else {
                    return Err(CompileError::type_err(
                        format!(
                            "parameter '{}: {}' of property '{}' names a field of the \
                             carrying type, but {} has no carrying type — field-parameter \
                             properties attach to class/object methods",
                            param.name.node,
                            param_kind_name(&param.kind.node),
                            short_name(cname),
                            target.desc
                        ),
                        arg.value.span,
                    ));
                };
                match resolve_arg(class, cname, param, &arg.value)? {
                    ArgValue::Field(f) => {
                        subst.insert(
                            param.name.node.clone(),
                            Expr::FieldAccess {
                                object: Box::new(Spanned::new(
                                    Expr::Ident("self".to_string()),
                                    clause.span,
                                )),
                                field: Spanned::new(f, clause.span),
                            },
                        );
                    }
                    _ => unreachable!("field params resolve to fields"),
                }
            }
            PropertyParamKind::Type => {
                // Validated for shape; type params are only meaningful to
                // guard atoms, which cannot appear in provides-capable
                // bodies — resolve for the error message alone.
                resolve_arg_shape_type(cname, param, &arg.value)?;
            }
            PropertyParamKind::ConstInt => match const_int_of_expr(&arg.value.node) {
                Some(n) => {
                    subst.insert(param.name.node.clone(), Expr::IntLit(n));
                }
                None => {
                    return Err(CompileError::type_err(
                        format!(
                            "argument for parameter '{}: const int' of property '{}' \
                             must be an integer literal",
                            param.name.node,
                            short_name(cname)
                        ),
                        arg.value.span,
                    ));
                }
            },
        }
    }
    Ok(subst)
}

fn resolve_arg_shape_type(
    prop_name: &str,
    param: &PropertyParam,
    arg: &Spanned<Expr>,
) -> Result<(), CompileError> {
    match type_name_of_expr(&arg.node) {
        Some(_) => Ok(()),
        None => Err(CompileError::type_err(
            format!(
                "argument for parameter '{}: type' of property '{}' must be a type name",
                param.name.node,
                short_name(prop_name)
            ),
            arg.span,
        )),
    }
}

/// An expr-kind argument is an expression over the provider's parameters:
/// a bare parameter name or a one-level path rooted at one (`req.id`).
fn validate_expr_arg(
    value: &Spanned<Expr>,
    prop_name: &str,
    param: &PropertyParam,
    target: &ProvidesTarget,
) -> Result<(), CompileError> {
    let root_ok = |root: &str| target.params.iter().any(|p| p == root);
    let err = |detail: String| {
        CompileError::type_err(
            format!(
                "argument for parameter '{}: expr' of property '{}' must be an \
                 expression over {}'s parameters — a parameter name or a one-level \
                 path rooted at one (e.g. 'req.id'): {detail}",
                param.name.node,
                short_name(prop_name),
                target.desc
            ),
            value.span,
        )
    };
    match &value.node {
        Expr::Ident(name) if root_ok(name) => Ok(()),
        Expr::Ident(name) => Err(err(format!("'{name}' is not a parameter"))),
        Expr::FieldAccess { object, field: _ } => match &object.node {
            Expr::Ident(root) if root_ok(root) => Ok(()),
            Expr::Ident(root) => Err(err(format!("path root '{root}' is not a parameter"))),
            _ => Err(err("paths deeper than one level are not supported".to_string())),
        },
        Expr::QualifiedAccess { segments } => match segments.first() {
            Some(root) if segments.len() == 2 && root_ok(&root.node) => Ok(()),
            Some(root) if segments.len() == 2 => {
                Err(err(format!("path root '{}' is not a parameter", root.node)))
            }
            _ => Err(err("paths deeper than one level are not supported".to_string())),
        },
        _ => Err(err("unsupported expression shape".to_string())),
    }
}

/// How a provides-capable property classifies for a given provider.
enum ProvidesShape {
    /// All atoms are ensures-shaped: provable on a class/object method.
    EnsuresOnly,
    /// The single atom is a dedup guard (rfc-properties.md phase 5.5):
    /// discharged CHECKED on a class/object method by the guard-placement
    /// proof in src/typeck/idempotency.rs, or ASSUMED at an extern boundary
    /// (an external system can implement the dedup internally).
    DedupOnly,
    /// No atoms: declared-only — no in-unit discharge path exists.
    DeclaredOnly,
    /// Contains invariant/guarded_by atoms: a type-level property.
    TypeLevel,
}

fn provides_shape(prop: &PropertyDecl) -> ProvidesShape {
    if has_type_level_atoms(prop) {
        ProvidesShape::TypeLevel
    } else if has_dedup_atoms(prop) {
        ProvidesShape::DedupOnly
    } else if prop.atoms.is_empty() {
        ProvidesShape::DeclaredOnly
    } else {
        ProvidesShape::EnsuresOnly
    }
}

fn type_level_provides_err(cname: &str, span: Span) -> CompileError {
    CompileError::type_err(
        format!(
            "property '{}' is type-level: its atoms (invariant / guarded_by) attach to \
             a class or object via 'satisfies', not to a function via 'provides'",
            short_name(cname)
        ),
        span,
    )
}

fn discharge_gap_err(cname: &str, desc: &str, span: Span) -> CompileError {
    CompileError::type_err(
        format!(
            "cannot discharge 'provides {short}' in-unit: property '{short}' declares \
             no checkable atoms — its proof shape is not yet expressible in the \
             verification kernel (rfc-properties.md phase 5.5), and a claim is never \
             silently promoted (epistemics.md). If {desc} fronts an external system \
             that guarantees the property, declare the claim at the trust boundary \
             instead — 'extern fn ... assume {short}(...)' — which discharges it as \
             ASSUMED and reports it on the assumption surface ('pluto analyze')",
            short = short_name(cname),
        ),
        span,
    )
}

fn ensures_needs_receiver_err(cname: &str, desc: &str, span: Span) -> CompileError {
    CompileError::type_err(
        format!(
            "property '{}' contains method-level 'ensures' atoms, which relate a \
             receiver's exit state to its entry state: only a class/object method can \
             provide it, and {desc} has no receiver",
            short_name(cname)
        ),
        span,
    )
}

fn dedup_needs_receiver_err(cname: &str, desc: &str, span: Span) -> CompileError {
    CompileError::type_err(
        format!(
            "property '{}' carries a 'dedup' atom, whose proof needs receiver state (a \
             monotone Set field holding the processed keys): only a class/object method \
             can provide it in-unit, and {desc} has no receiver. If {desc} fronts an \
             external system that dedups internally, claim it at the trust boundary \
             instead: 'extern fn ... assume {}(...)'",
            short_name(cname),
            short_name(cname)
        ),
        span,
    )
}

/// Process every fn-level `provides` clause: validate it, then discharge it
/// or fail. In-unit discharge paths: PROVEN for class/object methods
/// providing ensures-bodied properties (desugared into ordinary ensures
/// contracts, discharged by src/typeck/discharge.rs); CHECKED for
/// class/object methods providing dedup-bodied properties (the
/// guard-placement proof, src/typeck/idempotency.rs, phase 5.5); nothing
/// else — a declared-only property has no in-unit path (extern `assume` is
/// the boundary mode), and type-level properties are a kind error here.
fn process_fn_provides(
    program: &mut Program,
    table: &HashMap<String, usize>,
) -> Result<(), CompileError> {
    let Program { properties, functions, classes, app, stages, .. } = program;

    for func in functions.iter() {
        for clause in &func.node.provides {
            let prop = lookup_property(table, properties, &clause.node.name)?;
            let desc = format!("function '{}'", func.node.name.node);
            let target = ProvidesTarget {
                class: None,
                params: func.node.params.iter().map(|p| p.name.node.clone()).collect(),
                desc: desc.clone(),
            };
            validate_provides_args(clause, prop, &target)?;
            return Err(match provides_shape(prop) {
                ProvidesShape::TypeLevel => {
                    type_level_provides_err(&clause.node.name.node, clause.span)
                }
                ProvidesShape::DeclaredOnly => {
                    discharge_gap_err(&clause.node.name.node, &desc, clause.span)
                }
                ProvidesShape::EnsuresOnly => {
                    ensures_needs_receiver_err(&clause.node.name.node, &desc, clause.span)
                }
                ProvidesShape::DedupOnly => {
                    dedup_needs_receiver_err(&clause.node.name.node, &desc, clause.span)
                }
            });
        }
    }

    for methods in app
        .iter_mut()
        .map(|a| &mut a.node.methods)
        .chain(stages.iter_mut().map(|s| &mut s.node.methods))
    {
        for method in methods.iter() {
            if let Some(clause) = method.node.provides.first() {
                return Err(CompileError::type_err(
                    "'provides' on app/stage methods is not supported: property claims \
                     attach to class/object methods (proven) or extern fns (assumed)"
                        .to_string(),
                    clause.span,
                ));
            }
        }
    }

    for class in classes.iter_mut() {
        let has_provides = class
            .node
            .methods
            .iter()
            .any(|m| !m.node.provides.is_empty());
        if !has_provides {
            continue;
        }
        if !class.node.type_params.is_empty() {
            let clause = class
                .node
                .methods
                .iter()
                .flat_map(|m| m.node.provides.first())
                .next()
                .expect("has_provides checked");
            return Err(CompileError::type_err(
                format!(
                    "'provides' on methods of generic classes is not yet supported: \
                     property instantiations are compile-time proof obligations, and \
                     generic bodies are checked against opaque type parameters. Provide \
                     the property on a concrete class wrapping '{}' instead",
                    class.node.name.node
                ),
                clause.span,
            ));
        }
        let class_name = class.node.name.node.clone();
        // Field args resolve against the receiver class; split the borrow
        // by taking the method list out while resolving against the class.
        let mut methods = std::mem::take(&mut class.node.methods);
        let mut result = Ok(());
        'outer: for method in methods.iter_mut() {
            let clauses = method.node.provides.clone();
            for clause in &clauses {
                let prop = match lookup_property(table, properties, &clause.node.name) {
                    Ok(p) => p,
                    Err(e) => {
                        result = Err(e);
                        break 'outer;
                    }
                };
                let desc = format!("method '{}.{}'", class_name, method.node.name.node);
                let target = ProvidesTarget {
                    class: Some(&class.node),
                    params: method
                        .node
                        .params
                        .iter()
                        .filter(|p| p.name.node != "self")
                        .map(|p| p.name.node.clone())
                        .collect(),
                    desc: desc.clone(),
                };
                let subst = match validate_provides_args(clause, prop, &target) {
                    Ok(s) => s,
                    Err(e) => {
                        result = Err(e);
                        break 'outer;
                    }
                };
                match provides_shape(prop) {
                    ProvidesShape::TypeLevel => {
                        result = Err(type_level_provides_err(&clause.node.name.node, clause.span));
                        break 'outer;
                    }
                    ProvidesShape::DeclaredOnly => {
                        result = Err(discharge_gap_err(&clause.node.name.node, &desc, clause.span));
                        break 'outer;
                    }
                    ProvidesShape::DedupOnly => {
                        // CHECKED path (rfc-properties.md phase 5.5): the
                        // obligation is the dedup-guard placement proof over
                        // this method's effect sites, discharged after body
                        // checking by src/typeck/idempotency.rs (which
                        // re-reads this clause — nothing to inject here).
                    }
                    ProvidesShape::EnsuresOnly => {
                        // PROVEN path: substitute the atoms into ordinary
                        // ensures contracts; the standard discharge
                        // machinery (register_ensures + the symbolic
                        // prover) checks them with provenance blame.
                        let bindings = prop
                            .params
                            .iter()
                            .zip(&clause.node.args)
                            .map(|(p, a)| {
                                format!(
                                    "{} = {}",
                                    p.name.node,
                                    render_satisfies_arg(&a.value.node)
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        for atom in &prop.atoms {
                            let PropertyAtomKind::Ensures { expr } = &atom.node.kind else {
                                unreachable!("EnsuresOnly shape")
                            };
                            let mut expr = expr.clone();
                            substitute_expr(&mut expr, &subst, clause.span);
                            method.node.contracts.push(Spanned::new(
                                ContractClause {
                                    kind: ContractKind::Ensures,
                                    expr,
                                    provenance: Some(PropertyProvenance {
                                        property: clause.node.name.node.clone(),
                                        line: atom.node.line,
                                        bindings: bindings.clone(),
                                    }),
                                },
                                clause.span,
                            ));
                        }
                    }
                }
            }
        }
        class.node.methods = methods;
        result?;
    }
    Ok(())
}

/// Validate every extern `assume` clause: the ASSUMED discharge mode
/// (epistemics.md) — explicit, attributed, reported, never promoted. Only
/// declared-only and dedup-shaped properties are assumable: state-relating
/// atoms (ensures/invariant/guarded_by) describe fields an invisible body
/// cannot have, but a dedup atom describes *behavior* — an external system
/// can implement the dedup internally (an idempotent PUT keyed on the
/// request), which is exactly the claim the boundary vouches for.
fn process_extern_assumes(
    program: &Program,
    table: &HashMap<String, usize>,
) -> Result<(), CompileError> {
    for ext in &program.extern_fns {
        for clause in &ext.node.assumes {
            let prop = lookup_property(table, &program.properties, &clause.node.name)?;
            let desc = format!("extern fn '{}'", ext.node.name.node);
            match provides_shape(prop) {
                ProvidesShape::TypeLevel => {
                    return Err(type_level_provides_err(&clause.node.name.node, clause.span));
                }
                ProvidesShape::EnsuresOnly => {
                    return Err(ensures_needs_receiver_err(
                        &clause.node.name.node,
                        &desc,
                        clause.span,
                    ));
                }
                ProvidesShape::DeclaredOnly | ProvidesShape::DedupOnly => {}
            }
            let target = ProvidesTarget {
                class: None,
                params: ext.node.params.iter().map(|p| p.name.node.clone()).collect(),
                desc,
            };
            validate_provides_args(clause, prop, &target)?;
        }
    }
    Ok(())
}

/// Every `provides <name>` requirement on a fn TYPE must name a property
/// declaration in scope (matching is by resolved name).
fn validate_fn_type_provides(
    program: &Program,
    table: &HashMap<String, usize>,
) -> Result<(), CompileError> {
    use crate::visit::{walk_type_expr, Visitor};
    struct Check<'a> {
        table: &'a HashMap<String, usize>,
        err: Option<CompileError>,
    }
    impl Visitor for Check<'_> {
        fn visit_type_expr(&mut self, te: &Spanned<TypeExpr>) {
            if self.err.is_some() {
                return;
            }
            if let TypeExpr::Fn { provides, .. } = &te.node {
                for name in provides {
                    if !self.table.contains_key(name) {
                        self.err = Some(CompileError::type_err(
                            format!(
                                "unknown property '{name}' in fn-type requirement \
                                 'provides {name}': no such property declaration is in \
                                 scope (properties are declared with 'property \
                                 name(params) {{ ... }}' and imported like any other \
                                 declaration)"
                            ),
                            te.span,
                        ));
                        return;
                    }
                }
            }
            walk_type_expr(self, te);
        }
    }
    let mut check = Check { table, err: None };
    check.visit_program(program);
    match check.err {
        Some(e) => Err(e),
        None => Ok(()),
    }
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
