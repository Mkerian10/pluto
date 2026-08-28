//! Instantiate generic traits into concrete traits before type checking.
//!
//! A generic trait (`trait Convert<U> { fn convert(self) U }`) is a template.
//! Every use with concrete type arguments — an `impl Convert<int>` clause or
//! a `Convert<int>` type mention — stamps out a concrete trait named with the
//! generic mangling (`Convert$$int`), with the type parameters substituted
//! through the method signatures. Impl clauses and type mentions are then
//! rewritten to the concrete name, the templates are removed, and everything
//! downstream (registration, conformance, vtables, dispatch) sees ordinary
//! traits.
//!
//! Default method bodies are substituted along with the signatures, so a
//! stamped trait's defaults behave like any concrete trait's.
//!
//! MVP limits (each rejected with a targeted error):
//! - instantiating with a class's own type parameters (`class Box<V> impl T<V>`)
//! - type arguments other than named types and generic instances

use std::collections::{HashMap, HashSet};

use crate::diagnostics::CompileError;
use crate::parser::ast::{ImplTraitRef, Program, TraitDecl, TypeExpr};
use crate::span::{Span, Spanned};
use crate::visit::{walk_program_mut, walk_type_expr_mut, VisitMut};

pub fn instantiate_generic_traits(program: &mut Program) -> Result<(), CompileError> {
    // Collect templates. They STAY in the program: registration skips them,
    // monomorphize instantiates class-parameterized impls from them and
    // strips them after phase 3.
    let mut templates: HashMap<String, TraitDecl> = HashMap::new();
    for tr in &program.traits {
        if !tr.node.type_params.is_empty() {
            validate_template(&tr.node)?;
            templates.insert(tr.node.name.node.clone(), tr.node.clone());
        }
    }
    if templates.is_empty() {
        // Still reject stray type args on non-generic traits
        for class in &program.classes {
            for r in &class.node.impl_traits {
                if !r.type_args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("trait '{}' is not generic and does not accept type arguments", r.name.node),
                        r.name.span,
                    ));
                }
            }
        }
        return Ok(());
    }

    let mut instantiated: HashSet<String> = HashSet::new();
    let mut new_traits: Vec<Spanned<TraitDecl>> = Vec::new();

    // Class impl clauses
    for ci in 0..program.classes.len() {
        let class_type_params: Vec<String> = program.classes[ci]
            .node
            .type_params
            .iter()
            .map(|tp| tp.node.clone())
            .collect();
        let mut refs = std::mem::take(&mut program.classes[ci].node.impl_traits);
        for r in &mut refs {
            rewrite_impl_ref(
                r,
                &templates,
                &class_type_params,
                program,
                &mut instantiated,
                &mut new_traits,
            )?;
        }
        program.classes[ci].node.impl_traits = refs;
    }

    // Type mentions anywhere in the program (T<int> as a trait-object type).
    // Mentions whose arguments involve an enclosing declaration's type
    // parameters are left for monomorphize; template decls are not walked.
    {
        let index = ClassImplIndex::build(program);
        let mut rewriter = TypeMentionRewriter {
            templates: &templates,
            program_classes: index,
            instantiated: &mut instantiated,
            new_traits: &mut new_traits,
            skip_params: Vec::new(),
            error: None,
        };
        for f in &mut program.functions {
            rewriter.visit_function_mut(f);
        }
        for c in &mut program.classes {
            rewriter.skip_params = c.node.type_params.iter().map(|tp| tp.node.clone()).collect();
            crate::visit::walk_class_mut(&mut rewriter, c);
            rewriter.skip_params.clear();
        }
        for tr in &mut program.traits {
            if tr.node.type_params.is_empty() {
                rewriter.visit_trait_mut(tr);
            }
        }
        for e in &mut program.enums {
            rewriter.skip_params = e.node.type_params.iter().map(|tp| tp.node.clone()).collect();
            crate::visit::walk_enum_mut(&mut rewriter, e);
            rewriter.skip_params.clear();
        }
        if let Some(app) = &mut program.app {
            rewriter.visit_app_mut(app);
        }
        for st in &mut program.stages {
            rewriter.visit_stage_mut(st);
        }
        if let Some(err) = rewriter.error {
            return Err(err);
        }
    }

    // Newly stamped traits may themselves mention generic traits in their
    // substituted signatures — process to a fixed point.
    loop {
        let mut round = std::mem::take(&mut new_traits);
        if round.is_empty() {
            break;
        }
        let index = ClassImplIndex::build(program);
        for tr in &mut round {
            let mut rewriter = TypeMentionRewriter {
                templates: &templates,
                program_classes: index.clone(),
                instantiated: &mut instantiated,
                new_traits: &mut new_traits,
                skip_params: Vec::new(),
                error: None,
            };
            rewriter.visit_trait_mut(tr);
            if let Some(err) = rewriter.error {
                return Err(err);
            }
        }
        program.traits.extend(round);
    }

    Ok(())
}

fn validate_template(tr: &TraitDecl) -> Result<(), CompileError> {
    let mut seen = HashSet::new();
    for tp in &tr.type_params {
        if !seen.insert(tp.node.as_str()) {
            return Err(CompileError::type_err(
                format!("type parameter '{}' is already declared in trait '{}'", tp.node, tr.name.node),
                tp.span,
            ));
        }
    }
    Ok(())
}

/// Class name -> traits it declares in its impl list (by base name), used for
/// the syntactic bound check at instantiation time.
#[derive(Clone)]
struct ClassImplIndex {
    impls: HashMap<String, HashSet<String>>,
}

impl ClassImplIndex {
    fn build(program: &Program) -> Self {
        let mut impls: HashMap<String, HashSet<String>> = HashMap::new();
        for c in &program.classes {
            impls.insert(
                c.node.name.node.clone(),
                c.node.impl_traits.iter().map(|r| r.name.node.clone()).collect(),
            );
        }
        ClassImplIndex { impls }
    }

    fn satisfies(&self, arg: &TypeExpr, bound: &str) -> bool {
        match arg {
            TypeExpr::Named(n) => self.impls.get(n).is_some_and(|s| s.contains(bound)),
            // Generic instances: check the base class's impl list
            TypeExpr::Generic { name, .. } => {
                self.impls.get(name).is_some_and(|s| s.contains(bound))
            }
            _ => false,
        }
    }
}

fn rewrite_impl_ref(
    r: &mut ImplTraitRef,
    templates: &HashMap<String, TraitDecl>,
    class_type_params: &[String],
    program: &Program,
    instantiated: &mut HashSet<String>,
    new_traits: &mut Vec<Spanned<TraitDecl>>,
) -> Result<(), CompileError> {
    let is_template = templates.contains_key(&r.name.node);
    if !is_template {
        if !r.type_args.is_empty() {
            return Err(CompileError::type_err(
                format!("trait '{}' is not generic and does not accept type arguments", r.name.node),
                r.name.span,
            ));
        }
        return Ok(());
    }
    let template = &templates[&r.name.node];
    if r.type_args.len() != template.type_params.len() {
        return Err(CompileError::type_err(
            format!(
                "trait '{}' expects {} type arguments, got {}",
                r.name.node,
                template.type_params.len(),
                r.type_args.len()
            ),
            r.name.span,
        ));
    }
    // Arguments naming the class's own type parameters resolve per class
    // instantiation — monomorphize handles them (the ref stays as-is here)
    if r.type_args.iter().any(|ta| mentions_any(&ta.node, class_type_params)) {
        return Ok(());
    }
    let index = ClassImplIndex::build(program);
    let mangled = instantiate(template, &r.type_args, &index, instantiated, new_traits)?;
    r.name.node = mangled;
    r.type_args.clear();
    Ok(())
}

fn mentions_any(te: &TypeExpr, names: &[String]) -> bool {
    match te {
        TypeExpr::Named(n) => names.iter().any(|p| p == n),
        TypeExpr::Array(inner) | TypeExpr::Nullable(inner) | TypeExpr::Stream(inner) => {
            mentions_any(&inner.node, names)
        }
        TypeExpr::Generic { type_args, .. } => {
            type_args.iter().any(|a| mentions_any(&a.node, names))
        }
        TypeExpr::Fn { params, return_type, .. } => {
            params.iter().any(|p| mentions_any(&p.node, names))
                || mentions_any(&return_type.node, names)
        }
        TypeExpr::Qualified { .. } | TypeExpr::Infer => false,
    }
}

/// Structural mangling of a type argument, consistent with `mangle_name`
/// ("T$$int", "T$$Box$$int") for the supported subset.
pub(crate) fn mangle_te(te: &Spanned<TypeExpr>) -> Result<String, CompileError> {
    match &te.node {
        TypeExpr::Named(n) => Ok(n.clone()),
        TypeExpr::Generic { name, type_args } => {
            let args: Result<Vec<_>, _> = type_args.iter().map(mangle_te).collect();
            Ok(format!("{}$${}", name, args?.join("$")))
        }
        other => Err(CompileError::type_err(
            format!("unsupported type argument for generic trait instantiation: {other:?}"),
            te.span,
        )),
    }
}

fn instantiate(
    template: &TraitDecl,
    type_args: &[Spanned<TypeExpr>],
    index: &ClassImplIndex,
    instantiated: &mut HashSet<String>,
    new_traits: &mut Vec<Spanned<TraitDecl>>,
) -> Result<String, CompileError> {
    // Validate bounds syntactically: a bound is satisfied when the argument
    // names a class whose impl list declares the bound trait
    for (param, bounds) in &template.type_param_bounds {
        let Some(idx) = template.type_params.iter().position(|tp| &tp.node == param) else {
            continue;
        };
        let arg = &type_args[idx];
        for bound in bounds {
            if !index.satisfies(&arg.node, &bound.node) {
                return Err(CompileError::type_err(
                    format!(
                        "type argument for '{}' of trait '{}' does not satisfy trait bound '{}'",
                        param, template.name.node, bound.node
                    ),
                    arg.span,
                ));
            }
        }
    }

    let (decl, mangled) = stamp(template, type_args)?;
    if instantiated.contains(&mangled) {
        return Ok(mangled);
    }
    instantiated.insert(mangled.clone());
    let span = Span::new(template.name.span.start, template.name.span.end);
    new_traits.push(Spanned::new(decl, span));
    Ok(mangled)
}

/// Stamp a concrete trait from a template and concrete type-argument
/// expressions: substituted signatures, default bodies, and contracts, named
/// with the generic mangling. Pure — no bounds checking or dedup.
pub(crate) fn stamp(
    template: &TraitDecl,
    type_args: &[Spanned<TypeExpr>],
) -> Result<(TraitDecl, String), CompileError> {
    let suffixes: Result<Vec<_>, _> = type_args.iter().map(mangle_te).collect();
    let mangled = format!("{}$${}", template.name.node, suffixes?.join("$"));

    let bindings: HashMap<&str, &TypeExpr> = template
        .type_params
        .iter()
        .zip(type_args.iter())
        .map(|(p, a)| (p.node.as_str(), &a.node))
        .collect();

    let mut decl = template.clone();
    decl.id = uuid::Uuid::new_v4();
    decl.name = Spanned::new(mangled.clone(), template.name.span);
    decl.type_params.clear();
    decl.type_param_bounds.clear();
    let mut body_subst = BodySubst { bindings: &bindings };
    for m in &mut decl.methods {
        for p in &mut m.params {
            subst_te(&mut p.ty, &bindings);
        }
        if let Some(rt) = &mut m.return_type {
            subst_te(rt, &bindings);
        }
        // Default method bodies may mention type parameters in type
        // positions (casts, literals, explicit call type args)
        if let Some(body) = &mut m.body {
            body_subst.visit_block_mut(body);
        }
        for contract in &mut m.contracts {
            body_subst.visit_expr_mut(&mut contract.node.expr);
        }
    }
    Ok((decl, mangled))
}

/// Convert a concrete PlutoType back to a TypeExpr for trait stamping —
/// class-instantiation-time impl resolution has PlutoType arguments in hand.
pub(crate) fn pluto_type_to_type_expr(t: &crate::typeck::types::PlutoType) -> Option<TypeExpr> {
    use crate::typeck::types::PlutoType as P;
    Some(match t {
        P::Int => TypeExpr::Named("int".to_string()),
        P::Float => TypeExpr::Named("float".to_string()),
        P::Bool => TypeExpr::Named("bool".to_string()),
        P::Byte => TypeExpr::Named("byte".to_string()),
        P::Bytes => TypeExpr::Named("bytes".to_string()),
        P::String => TypeExpr::Named("string".to_string()),
        P::Class(n) | P::Enum(n) | P::Trait(n) => TypeExpr::Named(n.clone()),
        P::GenericInstance(_, name, args) => {
            let mut te_args = Vec::new();
            for a in args {
                te_args.push(Spanned::new(pluto_type_to_type_expr(a)?, Span::dummy()));
            }
            TypeExpr::Generic { name: name.clone(), type_args: te_args }
        }
        _ => return None,
    })
}

pub(crate) fn subst_te(te: &mut Spanned<TypeExpr>, bindings: &HashMap<&str, &TypeExpr>) {
    if let TypeExpr::Named(n) = &te.node
        && let Some(replacement) = bindings.get(n.as_str())
    {
        te.node = (*replacement).clone();
        return;
    }
    match &mut te.node {
        TypeExpr::Array(inner) | TypeExpr::Nullable(inner) | TypeExpr::Stream(inner) => {
            subst_te(inner, bindings)
        }
        TypeExpr::Generic { type_args, .. } => {
            for a in type_args {
                subst_te(a, bindings);
            }
        }
        TypeExpr::Fn { params, return_type, .. } => {
            for p in params {
                subst_te(p, bindings);
            }
            subst_te(return_type, bindings);
        }
        TypeExpr::Named(_) | TypeExpr::Qualified { .. } | TypeExpr::Infer => {}
    }
}

/// Substitutes trait type parameters through a default method body.
struct BodySubst<'a> {
    bindings: &'a HashMap<&'a str, &'a TypeExpr>,
}

impl VisitMut for BodySubst<'_> {
    fn visit_type_expr_mut(&mut self, te: &mut Spanned<TypeExpr>) {
        subst_te(te, self.bindings);
    }
}

/// Rewrites `T<int>` type mentions (trait-object types) to the concrete
/// instantiated trait name wherever they appear.
struct TypeMentionRewriter<'a> {
    templates: &'a HashMap<String, TraitDecl>,
    program_classes: ClassImplIndex,
    instantiated: &'a mut HashSet<String>,
    new_traits: &'a mut Vec<Spanned<TraitDecl>>,
    /// Type parameters of the enclosing declaration — mentions involving
    /// them are deferred to monomorphize
    skip_params: Vec<String>,
    error: Option<CompileError>,
}

impl VisitMut for TypeMentionRewriter<'_> {
    fn visit_function_mut(&mut self, func: &mut Spanned<crate::parser::ast::Function>) {
        let added = func.node.type_params.len();
        self.skip_params
            .extend(func.node.type_params.iter().map(|tp| tp.node.clone()));
        crate::visit::walk_function_mut(self, func);
        self.skip_params.truncate(self.skip_params.len() - added);
    }

    fn visit_type_expr_mut(&mut self, te: &mut Spanned<TypeExpr>) {
        if self.error.is_some() {
            return;
        }
        // Children first, so nested mentions resolve inside-out
        walk_type_expr_mut(self, te);
        if !self.skip_params.is_empty() && mentions_any(&te.node, &self.skip_params) {
            return;
        }
        if let TypeExpr::Generic { name, type_args } = &te.node
            && let Some(template) = self.templates.get(name)
        {
            if type_args.len() != template.type_params.len() {
                self.error = Some(CompileError::type_err(
                    format!(
                        "trait '{}' expects {} type arguments, got {}",
                        name,
                        template.type_params.len(),
                        type_args.len()
                    ),
                    te.span,
                ));
                return;
            }
            match instantiate(
                template,
                type_args,
                &self.program_classes,
                self.instantiated,
                self.new_traits,
            ) {
                Ok(mangled) => te.node = TypeExpr::Named(mangled),
                Err(e) => self.error = Some(e),
            }
        }
    }
}
