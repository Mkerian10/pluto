use crate::diagnostics::CompileError;
use crate::parser::ast::{ClassDecl, EnumDecl, Program, TraitDecl};
use crate::span::Spanned;
use std::collections::HashSet;
use std::sync::OnceLock;

const PRELUDE_SOURCE: &str = include_str!("../stdlib/prelude.pt");

/// Cached prelude data: parsed AST enums, classes, traits + sets of their names.
/// Parsed once on first access, shared by all callers.
struct PreludeData {
    enums: Vec<Spanned<EnumDecl>>,
    classes: Vec<Spanned<ClassDecl>>,
    traits: Vec<Spanned<TraitDecl>>,
    enum_names: HashSet<String>,
    class_names: HashSet<String>,
    trait_names: HashSet<String>,
    /// Error from validating the prelude's own contract expressions, if any.
    /// Surfaced as a proper diagnostic by `inject_prelude` instead of letting
    /// an invalid prelude contract reach (and panic in) later pipeline stages.
    contract_error: Option<CompileError>,
}

static PRELUDE: OnceLock<PreludeData> = OnceLock::new();

fn get_prelude() -> &'static PreludeData {
    PRELUDE.get_or_init(|| {
        let tokens = crate::lexer::lex(PRELUDE_SOURCE).expect("prelude must lex");
        let mut parser = crate::parser::Parser::new_without_prelude(&tokens, PRELUDE_SOURCE);
        let mut program = parser.parse_program().expect("prelude must parse");
        // The prelude is injected AFTER module flattening (pipeline stage 5 vs 4),
        // so it never goes through the QualifiedAccess resolution that user code
        // gets. Resolve here so prelude declarations (contract expressions like
        // `self.offset >= 0`, method bodies, defaults) arrive in the same resolved
        // shape as user code. The prelude imports no modules, so the single-file
        // resolution (empty module set) is exactly right.
        crate::modules::resolve_qualified_access_single_file(&mut program)
            .expect("prelude qualified-access resolution must succeed");
        // Validate the prelude's own contracts now, while we still know the
        // expressions came from the prelude. Stage 9 validation runs over the
        // merged program and would attribute the error to the user's source.
        let contract_error = crate::contracts::validate_contracts(&program).err();
        let enum_names = program
            .enums
            .iter()
            .map(|e| e.node.name.node.clone())
            .collect();
        let class_names = program
            .classes
            .iter()
            .map(|c| c.node.name.node.clone())
            .collect();
        let trait_names = program
            .traits
            .iter()
            .map(|t| t.node.name.node.clone())
            .collect();
        PreludeData {
            enums: program.enums,
            classes: program.classes,
            traits: program.traits,
            enum_names,
            class_names,
            trait_names,
            contract_error,
        }
    })
}

/// Returns prelude enum names (for parser seeding). Cached.
pub fn prelude_enum_names() -> &'static HashSet<String> {
    &get_prelude().enum_names
}

/// Inject prelude types into a parsed program.
/// Checks for name conflicts across enums, classes, traits, and errors.
pub fn inject_prelude(program: &mut Program) -> Result<(), CompileError> {
    let data = get_prelude();

    // An invalid contract in the prelude itself is a compiler/stdlib bug, but
    // it must surface as a diagnostic, not a panic deeper in the pipeline. Its
    // span points into the prelude source (not the user's file), so report the
    // message with a synthetic span and name the prelude explicitly.
    if let Some(err) = &data.contract_error {
        return Err(CompileError::type_err(
            format!("invalid contract in prelude (stdlib/prelude.pt): {err}"),
            crate::span::Span::synthetic(),
        ));
    }

    // Check if prelude is already injected (idempotency check)
    // If the first enum is a prelude enum, assume prelude is already there
    if !program.enums.is_empty() && data.enum_names.contains(&program.enums[0].node.name.node) {
        return Ok(());
    }

    // Check for conflicts with prelude enums
    for prelude_name in &data.enum_names {
        // Check enums
        for e in &program.enums {
            if &e.node.name.node == prelude_name {
                return Err(CompileError::type_err(
                    format!(
                        "cannot define enum '{}': conflicts with built-in prelude type",
                        prelude_name
                    ),
                    e.node.name.span,
                ));
            }
        }
        // Check classes
        for c in &program.classes {
            if &c.node.name.node == prelude_name {
                return Err(CompileError::type_err(
                    format!(
                        "cannot define class '{}': conflicts with built-in prelude type",
                        prelude_name
                    ),
                    c.node.name.span,
                ));
            }
        }
        // Check traits
        for t in &program.traits {
            if &t.node.name.node == prelude_name {
                return Err(CompileError::type_err(
                    format!(
                        "cannot define trait '{}': conflicts with built-in prelude type",
                        prelude_name
                    ),
                    t.node.name.span,
                ));
            }
        }
        // Check errors
        for err in &program.errors {
            if &err.node.name.node == prelude_name {
                return Err(CompileError::type_err(
                    format!(
                        "cannot define error '{}': conflicts with built-in prelude type",
                        prelude_name
                    ),
                    err.node.name.span,
                ));
            }
        }
    }

    // Check for conflicts with prelude classes
    for prelude_name in &data.class_names {
        // Check enums
        for e in &program.enums {
            if &e.node.name.node == prelude_name {
                return Err(CompileError::type_err(
                    format!(
                        "cannot define enum '{}': conflicts with built-in prelude type",
                        prelude_name
                    ),
                    e.node.name.span,
                ));
            }
        }
        // Check classes
        for c in &program.classes {
            if &c.node.name.node == prelude_name {
                return Err(CompileError::type_err(
                    format!(
                        "cannot define class '{}': conflicts with built-in prelude type",
                        prelude_name
                    ),
                    c.node.name.span,
                ));
            }
        }
        // Check traits
        for t in &program.traits {
            if &t.node.name.node == prelude_name {
                return Err(CompileError::type_err(
                    format!(
                        "cannot define trait '{}': conflicts with built-in prelude type",
                        prelude_name
                    ),
                    t.node.name.span,
                ));
            }
        }
        // Check errors
        for err in &program.errors {
            if &err.node.name.node == prelude_name {
                return Err(CompileError::type_err(
                    format!(
                        "cannot define error '{}': conflicts with built-in prelude type",
                        prelude_name
                    ),
                    err.node.name.span,
                ));
            }
        }
    }

    // Check for conflicts with prelude traits
    for prelude_name in &data.trait_names {
        // Check enums
        for e in &program.enums {
            if &e.node.name.node == prelude_name {
                return Err(CompileError::type_err(
                    format!(
                        "cannot define enum '{}': conflicts with built-in prelude type",
                        prelude_name
                    ),
                    e.node.name.span,
                ));
            }
        }
        // Check classes
        for c in &program.classes {
            if &c.node.name.node == prelude_name {
                return Err(CompileError::type_err(
                    format!(
                        "cannot define class '{}': conflicts with built-in prelude type",
                        prelude_name
                    ),
                    c.node.name.span,
                ));
            }
        }
        // Check traits
        for t in &program.traits {
            if &t.node.name.node == prelude_name {
                return Err(CompileError::type_err(
                    format!(
                        "cannot define trait '{}': conflicts with built-in prelude type",
                        prelude_name
                    ),
                    t.node.name.span,
                ));
            }
        }
        // Check errors
        for err in &program.errors {
            if &err.node.name.node == prelude_name {
                return Err(CompileError::type_err(
                    format!(
                        "cannot define error '{}': conflicts with built-in prelude type",
                        prelude_name
                    ),
                    err.node.name.span,
                ));
            }
        }
    }

    // Prepend prelude enums to the program
    let mut prelude_enums = data.enums.clone();
    prelude_enums.append(&mut program.enums);
    program.enums = prelude_enums;

    // Prepend prelude classes to the program
    let mut prelude_classes = data.classes.clone();
    prelude_classes.append(&mut program.classes);
    program.classes = prelude_classes;

    // Prepend prelude traits to the program
    let mut prelude_traits = data.traits.clone();
    prelude_traits.append(&mut program.traits);
    program.traits = prelude_traits;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::Expr;

    /// The prelude is injected after module flattening, so its contract
    /// expressions must already be resolved (no QualifiedAccess left) when
    /// they reach contract validation — otherwise contracts.rs panics.
    #[test]
    fn prelude_contracts_are_resolved_and_valid() {
        let data = get_prelude();
        assert!(
            data.contract_error.is_none(),
            "shipped prelude has an invalid contract: {:?}",
            data.contract_error
        );
        for class in &data.classes {
            for inv in &class.node.invariants {
                assert_resolved(&inv.node.expr.node, &class.node.name.node);
            }
            for method in &class.node.methods {
                for contract in &method.node.contracts {
                    assert_resolved(&contract.node.expr.node, &class.node.name.node);
                }
            }
        }
    }

    fn assert_resolved(expr: &Expr, class_name: &str) {
        struct Checker<'a> {
            class_name: &'a str,
        }
        impl crate::visit::Visitor for Checker<'_> {
            fn visit_expr(&mut self, expr: &crate::span::Spanned<Expr>) {
                if let Expr::QualifiedAccess { segments } = &expr.node {
                    panic!(
                        "unresolved QualifiedAccess {:?} in a contract of prelude class '{}'",
                        segments.iter().map(|s| &s.node).collect::<Vec<_>>(),
                        self.class_name
                    );
                }
                crate::visit::walk_expr(self, expr);
            }
        }
        let spanned = crate::span::Spanned::new(expr.clone(), crate::span::Span::new(0, 0));
        let mut checker = Checker { class_name };
        crate::visit::Visitor::visit_expr(&mut checker, &spanned);
    }
}
