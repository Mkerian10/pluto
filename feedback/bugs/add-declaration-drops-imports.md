# MCP Bug: `add_declaration` drops imports and test metadata — FIXED

- **Date**: 2026-02-10
- **Project**: cask
- **Severity**: High — broke structured-edit-only workflow
- **Status**: FIXED in `sdk/src/editor.rs`

## Description

`ModuleEditor::add_many_from_source()` parsed the input source correctly (imports, test_info, declarations), but only drained declarations into the program AST. Both `program.imports` and `program.test_info` were silently discarded.

This caused:
1. Import statements dropped — files with `import std.time` lost the import
2. Test blocks mangled — `test "name" { ... }` became `fn __test_N()` because test metadata was lost

## Fix

Added to `sdk/src/editor.rs`:
- `merge_imports()` helper — deduplicates and merges import statements by path + alias
- Called in both `add_many_from_source()` and `replace_from_source()`
- `test_info` and `tests` (scheduler decl) now merged alongside declarations
