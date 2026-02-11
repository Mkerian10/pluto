use std::collections::{HashMap, HashSet};

use crate::diagnostics::CompileError;
use crate::parser::ast::{Program, RequiredMethod, StageDecl};
use crate::span::Spanned;

/// Flatten stage inheritance hierarchies.
///
/// Resolves single-inheritance chains by copying methods, inject_fields,
/// ambient_types, and lifecycle_overrides from ancestors into concrete stages.
/// After flattening, abstract stages (those with unresolved `requires fn`)
/// are removed — only concrete stages with complete method sets remain.
///
/// Runs before typeck so downstream passes see flat stages with no inheritance.
pub fn flatten_stage_hierarchy(program: &mut Program) -> Result<(), CompileError> {
    if program.stages.is_empty() {
        return Ok(());
    }

    // Build name → index map
    let name_to_idx: HashMap<String, usize> = program.stages.iter().enumerate()
        .map(|(i, s)| (s.node.name.node.clone(), i))
        .collect();

    // Validate parent references exist
    for stage in &program.stages {
        if let Some(ref parent_name) = stage.node.parent {
            if !name_to_idx.contains_key(&parent_name.node) {
                return Err(CompileError::type_err(
                    format!("stage '{}' inherits from unknown stage '{}'", stage.node.name.node, parent_name.node),
                    parent_name.span,
                ));
            }
        }
    }

    // Detect cycles: walk parent chain from each stage
    for stage in &program.stages {
        let mut visited = HashSet::new();
        visited.insert(stage.node.name.node.clone());
        let mut current = &stage.node;
        while let Some(ref parent_name) = current.parent {
            if !visited.insert(parent_name.node.clone()) {
                return Err(CompileError::type_err(
                    format!("circular stage inheritance: '{}' eventually inherits from itself", stage.node.name.node),
                    parent_name.span,
                ));
            }
            if let Some(&idx) = name_to_idx.get(&parent_name.node) {
                current = &program.stages[idx].node;
            } else {
                break; // already validated above
            }
        }
    }

    // Compute ancestor chains (root-first order) for each stage
    let ancestor_chains: Vec<Vec<usize>> = program.stages.iter().map(|stage| {
        let mut chain = vec![];
        let mut current = &stage.node;
        // Walk up to root
        while let Some(ref parent_name) = current.parent {
            if let Some(&idx) = name_to_idx.get(&parent_name.node) {
                chain.push(idx);
                current = &program.stages[idx].node;
            } else {
                break;
            }
        }
        chain.reverse(); // root-first
        chain
    }).collect();

    // Flatten each stage by walking root-to-self
    let stages_snapshot: Vec<Spanned<StageDecl>> = program.stages.clone();
    for (i, stage) in program.stages.iter_mut().enumerate() {
        let ancestors = &ancestor_chains[i];
        if ancestors.is_empty() && stage.node.parent.is_none() {
            // No inheritance — nothing to flatten
            continue;
        }

        let mut effective_methods: HashMap<String, Spanned<crate::parser::ast::Function>> = HashMap::new();
        let mut effective_requires: HashMap<String, Spanned<RequiredMethod>> = HashMap::new();
        let mut merged_inject_fields = Vec::new();
        let mut merged_ambient_types = Vec::new();
        let mut merged_lifecycle_overrides = Vec::new();
        let mut seen_field_names: HashSet<String> = HashSet::new();
        let mut seen_ambient_names: HashSet<String> = HashSet::new();

        // Process each ancestor (root-first), then self
        let all_indices: Vec<usize> = ancestors.iter().copied().chain(std::iter::once(i)).collect();
        for &idx in &all_indices {
            let source = &stages_snapshot[idx];

            // Merge inject_fields
            for field in &source.node.inject_fields {
                if !seen_field_names.insert(field.name.node.clone()) {
                    return Err(CompileError::type_err(
                        format!("duplicate injected field '{}' in stage inheritance chain for '{}'",
                            field.name.node, stage.node.name.node),
                        field.name.span,
                    ));
                }
                merged_inject_fields.push(field.clone());
            }

            // Merge ambient_types (deduplicate)
            for amb in &source.node.ambient_types {
                if seen_ambient_names.insert(amb.node.clone()) {
                    merged_ambient_types.push(amb.clone());
                }
            }

            // Merge lifecycle_overrides (child wins — just append, last wins in downstream processing)
            for lc in &source.node.lifecycle_overrides {
                merged_lifecycle_overrides.push(lc.clone());
            }

            // Process requires fn
            for req in &source.node.required_methods {
                effective_requires.insert(req.node.name.node.clone(), req.clone());
            }

            // Process concrete methods
            for method in &source.node.methods {
                let method_name = &method.node.name.node;

                if method.node.is_override {
                    // override fn: must exist in parent methods or requires
                    if !effective_methods.contains_key(method_name) && !effective_requires.contains_key(method_name) {
                        return Err(CompileError::type_err(
                            format!("'override fn {}' in stage '{}' does not override any method from parent stage",
                                method_name, source.node.name.node),
                            method.node.name.span,
                        ));
                    }
                } else if idx != all_indices[0] || source.node.parent.is_some() {
                    // Not override: must NOT shadow a parent method (only check if this stage has a parent)
                    if effective_methods.contains_key(method_name) || effective_requires.contains_key(method_name) {
                        return Err(CompileError::type_err(
                            format!("method '{}' in stage '{}' shadows a parent method — use 'override fn' to override",
                                method_name, source.node.name.node),
                            method.node.name.span,
                        ));
                    }
                }

                // Satisfies requires
                effective_requires.remove(method_name);
                // Insert/replace in effective methods
                effective_methods.insert(method_name.clone(), method.clone());
            }
        }

        // Write flattened data back into the stage
        stage.node.inject_fields = merged_inject_fields;
        stage.node.ambient_types = merged_ambient_types;
        stage.node.lifecycle_overrides = merged_lifecycle_overrides;
        stage.node.required_methods = effective_requires.into_values()
            .collect();
        stage.node.methods = effective_methods.into_values()
            .collect();
    }

    // Remove abstract stages (those with remaining required methods)
    program.stages.retain(|s| s.node.required_methods.is_empty());

    Ok(())
}
