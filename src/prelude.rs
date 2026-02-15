use crate::diagnostics::CompileError;
use crate::parser::ast::{ClassDecl, EnumDecl, Program, TraitDecl};
use crate::span::Spanned;
use std::collections::HashSet;
use std::sync::OnceLock;

const PRELUDE_SOURCE: &str = include_str!("../stdlib/prelude.pluto");

/// Cached prelude data: parsed AST enums, classes, traits + sets of their names.
/// Parsed once on first access, shared by all callers.
struct PreludeData {
    enums: Vec<Spanned<EnumDecl>>,
    classes: Vec<Spanned<ClassDecl>>,
    traits: Vec<Spanned<TraitDecl>>,
    enum_names: HashSet<String>,
    class_names: HashSet<String>,
    trait_names: HashSet<String>,
}

static PRELUDE: OnceLock<PreludeData> = OnceLock::new();

fn get_prelude() -> &'static PreludeData {
    PRELUDE.get_or_init(|| {
        let tokens = crate::lexer::lex(PRELUDE_SOURCE).expect("prelude must lex");
        let mut parser = crate::parser::Parser::new_without_prelude(&tokens, PRELUDE_SOURCE);
        let program = parser.parse_program().expect("prelude must parse");
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
