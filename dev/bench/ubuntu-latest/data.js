window.BENCHMARK_DATA = {
  "lastUpdate": 1770796603937,
  "repoUrl": "https://github.com/Mkerian10/pluto",
  "entries": {
    "Benchmark": [
      {
        "commit": {
          "author": {
            "email": "33331268+Mkerian10@users.noreply.github.com",
            "name": "Matthew Kerian",
            "username": "Mkerian10"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fc61e6bc2c88cb921596f1a317d140281332f6e7",
          "message": "Add project README and fix benchmark CI for missing gh-pages (#9)\n\nAdd a comprehensive README with language tour, feature highlights,\nstdlib docs, and code examples. Also fix the benchmark workflow's\nPR comparison step to continue-on-error when gh-pages doesn't exist\nyet (first push to master creates the baseline).\n\nCo-authored-by: Matthew Kerian <matthewkerian@Matthews-MacBook-Air.local>\nCo-authored-by: Claude Opus 4.6 <noreply@anthropic.com>",
          "timestamp": "2026-02-10T00:27:31Z",
          "tree_id": "3cf580c6cdf24b182dc221e7990ec9fdd4db8338",
          "url": "https://github.com/Mkerian10/pluto/commit/fc61e6bc2c88cb921596f1a317d140281332f6e7"
        },
        "date": 1770684585958,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "fib",
            "value": 71,
            "unit": "ms"
          },
          {
            "name": "loop_sum",
            "value": 62,
            "unit": "ms"
          },
          {
            "name": "string_concat",
            "value": 324,
            "unit": "ms"
          },
          {
            "name": "array_push",
            "value": 6,
            "unit": "ms"
          },
          {
            "name": "array_iter",
            "value": 2,
            "unit": "ms"
          },
          {
            "name": "class_method",
            "value": 6,
            "unit": "ms"
          },
          {
            "name": "closure_call",
            "value": 24,
            "unit": "ms"
          },
          {
            "name": "trait_dispatch",
            "value": 1351,
            "unit": "ms"
          },
          {
            "name": "gc_churn",
            "value": 132,
            "unit": "ms"
          },
          {
            "name": "gc_binary_trees",
            "value": 1916,
            "unit": "ms"
          },
          {
            "name": "gc_string_pressure",
            "value": 79,
            "unit": "ms"
          },
          {
            "name": "sieve",
            "value": 10,
            "unit": "ms"
          },
          {
            "name": "bounce",
            "value": 47,
            "unit": "ms"
          },
          {
            "name": "towers",
            "value": 1738,
            "unit": "ms"
          },
          {
            "name": "permute",
            "value": 104,
            "unit": "ms"
          },
          {
            "name": "queens",
            "value": 148,
            "unit": "ms"
          },
          {
            "name": "fannkuch_redux",
            "value": 1338,
            "unit": "ms"
          },
          {
            "name": "spectral_norm",
            "value": 50,
            "unit": "ms"
          },
          {
            "name": "nbody",
            "value": 8984,
            "unit": "ms"
          },
          {
            "name": "mandelbrot",
            "value": 286,
            "unit": "ms"
          },
          {
            "name": "monte_carlo",
            "value": 1434,
            "unit": "ms"
          },
          {
            "name": "storage",
            "value": 771,
            "unit": "ms"
          },
          {
            "name": "list",
            "value": 89,
            "unit": "ms"
          },
          {
            "name": "fft",
            "value": 2331,
            "unit": "ms"
          },
          {
            "name": "sor",
            "value": 575,
            "unit": "ms"
          },
          {
            "name": "sparse_matmul",
            "value": 539,
            "unit": "ms"
          },
          {
            "name": "lu",
            "value": 4059,
            "unit": "ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "33331268+Mkerian10@users.noreply.github.com",
            "name": "Matthew Kerian",
            "username": "Mkerian10"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "af3dcd2d4a28cccae546e670d6c64c211dbacbb8",
          "message": "Merge pull request #11 from Mkerian10/fix-flaky-test\n\nFix flaky race_shared_counter_lost_updates test",
          "timestamp": "2026-02-09T22:52:57-06:00",
          "tree_id": "f826b019ed1be18f5d74c2c7d1541f9439b01c43",
          "url": "https://github.com/Mkerian10/pluto/commit/af3dcd2d4a28cccae546e670d6c64c211dbacbb8"
        },
        "date": 1770699269795,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "fib",
            "value": 72,
            "unit": "ms"
          },
          {
            "name": "loop_sum",
            "value": 62,
            "unit": "ms"
          },
          {
            "name": "string_concat",
            "value": 522,
            "unit": "ms"
          },
          {
            "name": "array_push",
            "value": 6,
            "unit": "ms"
          },
          {
            "name": "array_iter",
            "value": 2,
            "unit": "ms"
          },
          {
            "name": "class_method",
            "value": 12,
            "unit": "ms"
          },
          {
            "name": "closure_call",
            "value": 21,
            "unit": "ms"
          },
          {
            "name": "trait_dispatch",
            "value": 1408,
            "unit": "ms"
          },
          {
            "name": "gc_churn",
            "value": 140,
            "unit": "ms"
          },
          {
            "name": "gc_binary_trees",
            "value": 1987,
            "unit": "ms"
          },
          {
            "name": "gc_string_pressure",
            "value": 74,
            "unit": "ms"
          },
          {
            "name": "sieve",
            "value": 10,
            "unit": "ms"
          },
          {
            "name": "bounce",
            "value": 47,
            "unit": "ms"
          },
          {
            "name": "towers",
            "value": 1801,
            "unit": "ms"
          },
          {
            "name": "permute",
            "value": 99,
            "unit": "ms"
          },
          {
            "name": "queens",
            "value": 151,
            "unit": "ms"
          },
          {
            "name": "fannkuch_redux",
            "value": 1325,
            "unit": "ms"
          },
          {
            "name": "spectral_norm",
            "value": 50,
            "unit": "ms"
          },
          {
            "name": "nbody",
            "value": 10648,
            "unit": "ms"
          },
          {
            "name": "mandelbrot",
            "value": 287,
            "unit": "ms"
          },
          {
            "name": "monte_carlo",
            "value": 1433,
            "unit": "ms"
          },
          {
            "name": "storage",
            "value": 811,
            "unit": "ms"
          },
          {
            "name": "list",
            "value": 87,
            "unit": "ms"
          },
          {
            "name": "fft",
            "value": 2418,
            "unit": "ms"
          },
          {
            "name": "sor",
            "value": 584,
            "unit": "ms"
          },
          {
            "name": "sparse_matmul",
            "value": 534,
            "unit": "ms"
          },
          {
            "name": "lu",
            "value": 4037,
            "unit": "ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "33331268+Mkerian10@users.noreply.github.com",
            "name": "Matthew Kerian",
            "username": "Mkerian10"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7d8f8ddf5e7cf335108acd5282a783e835371e0d",
          "message": "Merge pull request #12 from Mkerian10/fix-ci\n\nFix CI: rust_ffi linker failures + flaky channel timeouts + test duration tracking",
          "timestamp": "2026-02-10T00:10:52-06:00",
          "tree_id": "4bb3d5bfe7ce2822ae7608c06bab44a662bcd64d",
          "url": "https://github.com/Mkerian10/pluto/commit/7d8f8ddf5e7cf335108acd5282a783e835371e0d"
        },
        "date": 1770703953851,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "fib",
            "value": 72,
            "unit": "ms"
          },
          {
            "name": "loop_sum",
            "value": 124,
            "unit": "ms"
          },
          {
            "name": "string_concat",
            "value": 523,
            "unit": "ms"
          },
          {
            "name": "array_push",
            "value": 5,
            "unit": "ms"
          },
          {
            "name": "array_iter",
            "value": 3,
            "unit": "ms"
          },
          {
            "name": "class_method",
            "value": 6,
            "unit": "ms"
          },
          {
            "name": "closure_call",
            "value": 21,
            "unit": "ms"
          },
          {
            "name": "trait_dispatch",
            "value": 1423,
            "unit": "ms"
          },
          {
            "name": "gc_churn",
            "value": 140,
            "unit": "ms"
          },
          {
            "name": "gc_binary_trees",
            "value": 1984,
            "unit": "ms"
          },
          {
            "name": "gc_string_pressure",
            "value": 76,
            "unit": "ms"
          },
          {
            "name": "sieve",
            "value": 10,
            "unit": "ms"
          },
          {
            "name": "bounce",
            "value": 47,
            "unit": "ms"
          },
          {
            "name": "towers",
            "value": 1755,
            "unit": "ms"
          },
          {
            "name": "permute",
            "value": 99,
            "unit": "ms"
          },
          {
            "name": "queens",
            "value": 153,
            "unit": "ms"
          },
          {
            "name": "fannkuch_redux",
            "value": 1351,
            "unit": "ms"
          },
          {
            "name": "spectral_norm",
            "value": 47,
            "unit": "ms"
          },
          {
            "name": "nbody",
            "value": 9149,
            "unit": "ms"
          },
          {
            "name": "mandelbrot",
            "value": 287,
            "unit": "ms"
          },
          {
            "name": "monte_carlo",
            "value": 1430,
            "unit": "ms"
          },
          {
            "name": "storage",
            "value": 794,
            "unit": "ms"
          },
          {
            "name": "list",
            "value": 92,
            "unit": "ms"
          },
          {
            "name": "fft",
            "value": 2337,
            "unit": "ms"
          },
          {
            "name": "sor",
            "value": 588,
            "unit": "ms"
          },
          {
            "name": "sparse_matmul",
            "value": 528,
            "unit": "ms"
          },
          {
            "name": "lu",
            "value": 4049,
            "unit": "ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "33331268+Mkerian10@users.noreply.github.com",
            "name": "Matthew Kerian",
            "username": "Mkerian10"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c956c11ea3e3b5f820d6f4b3b3d1e9408969c6f9",
          "message": "Add generators (stream T + yield + for-in-stream)\n\n- Adds stream T return type, yield statement, and for x in stream iteration to Pluto\n- Generator functions compile to a state machine: creator + next functions\n- 24 integration tests covering all generator features\n- Example at examples/generators/main.pluto\n- Fix mutability errors in 4 benchmarks (class_method, trait_dispatch, bounce, nbody)\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>",
          "timestamp": "2026-02-11T00:02:09-06:00",
          "tree_id": "e3656a92e96068c9eb8faab936623cf84948f146",
          "url": "https://github.com/Mkerian10/pluto/commit/c956c11ea3e3b5f820d6f4b3b3d1e9408969c6f9"
        },
        "date": 1770789848122,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "fib",
            "value": 72,
            "unit": "ms"
          },
          {
            "name": "loop_sum",
            "value": 64,
            "unit": "ms"
          },
          {
            "name": "string_concat",
            "value": 573,
            "unit": "ms"
          },
          {
            "name": "array_push",
            "value": 5,
            "unit": "ms"
          },
          {
            "name": "array_iter",
            "value": 2,
            "unit": "ms"
          },
          {
            "name": "class_method",
            "value": 6,
            "unit": "ms"
          },
          {
            "name": "closure_call",
            "value": 21,
            "unit": "ms"
          },
          {
            "name": "trait_dispatch",
            "value": 1428,
            "unit": "ms"
          },
          {
            "name": "gc_churn",
            "value": 142,
            "unit": "ms"
          },
          {
            "name": "gc_binary_trees",
            "value": 2029,
            "unit": "ms"
          },
          {
            "name": "gc_string_pressure",
            "value": 74,
            "unit": "ms"
          },
          {
            "name": "sieve",
            "value": 10,
            "unit": "ms"
          },
          {
            "name": "bounce",
            "value": 47,
            "unit": "ms"
          },
          {
            "name": "towers",
            "value": 1725,
            "unit": "ms"
          },
          {
            "name": "permute",
            "value": 99,
            "unit": "ms"
          },
          {
            "name": "queens",
            "value": 156,
            "unit": "ms"
          },
          {
            "name": "fannkuch_redux",
            "value": 1317,
            "unit": "ms"
          },
          {
            "name": "spectral_norm",
            "value": 47,
            "unit": "ms"
          },
          {
            "name": "nbody",
            "value": 10622,
            "unit": "ms"
          },
          {
            "name": "mandelbrot",
            "value": 286,
            "unit": "ms"
          },
          {
            "name": "monte_carlo",
            "value": 1433,
            "unit": "ms"
          },
          {
            "name": "storage",
            "value": 801,
            "unit": "ms"
          },
          {
            "name": "list",
            "value": 91,
            "unit": "ms"
          },
          {
            "name": "fft",
            "value": 2343,
            "unit": "ms"
          },
          {
            "name": "sor",
            "value": 595,
            "unit": "ms"
          },
          {
            "name": "sparse_matmul",
            "value": 538,
            "unit": "ms"
          },
          {
            "name": "lu",
            "value": 4022,
            "unit": "ms"
          },
          {
            "name": "json_parse",
            "value": 1170,
            "unit": "ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "33331268+Mkerian10@users.noreply.github.com",
            "name": "Matthew Kerian",
            "username": "Mkerian10"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "578d01fcb1344ee8f07dd756cfb74a7dd85dca78",
          "message": "Merge compiler-hardening and recent improvements (#21)\n\n* Add lexer, AST, and parser support for stage inheritance\n\n- Add Override token to lexer\n- Add RequiredMethod struct, StageDecl.parent, StageDecl.required_methods,\n  and Function.is_override to AST\n- Parse `: ParentName` after stage name\n- Parse `requires fn` for abstract method signatures\n- Parse `override fn` for method overrides\n- Add is_override: false to all existing Function construction sites\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Add stage flattening pass for inheritance resolution\n\n- Create src/stages.rs with flatten_stage_hierarchy()\n- Validates parent references, detects cycles\n- Walks ancestor chains root-first to merge methods, inject_fields,\n  ambient_types, and lifecycle_overrides\n- Enforces override/shadowing rules\n- Removes abstract stages after flattening\n- Integrate into pipeline after prelude injection, before ambient desugar\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Add tests, modules support, pretty printer, and example for stage inheritance\n\n- Update modules.rs to rewrite required method param/return types\n- Update pretty.rs to emit `: Parent`, `requires fn`, and `override` prefix\n- Add 11 integration tests for stage inheritance\n- Update stages example to demonstrate abstract base + concrete stage\n- Report error for leaf stages with unimplemented required methods\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Fix GC root scanning on x86_64 with explicit register capture\n\nOn x86_64 glibc, setjmp applies pointer mangling (PTR_MANGLE) to RSP,\nRBP, and the return address in the jmp_buf, making those values\nunrecognizable during conservative GC root scanning. This caused live\nobjects whose only reference was in a mangled register to be incorrectly\nswept, leading to use-after-free crashes.\n\nThe fix adds explicit inline assembly to capture all GPRs into a stack\nbuffer before scanning, bypassing pointer mangling entirely. This is the\nstandard approach used by conservative GCs (e.g., Boehm GC). Added for\nboth x86_64 (16 GPRs) and aarch64 (31 GPRs x0-x30).\n\nAlso updates Dockerfile to rust:1.93 and re-enables the JSON conformance\ntest that was previously marked #[ignore] due to this bug.\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Reduce duplication across compiler with mechanical refactors (-113 LOC)\n\nSix pure refactors with zero behavior changes:\n- Spanned::dummy() constructor, replacing Span::new(0,0) boilerplate\n- resolve_builtin_generic() helper unifying duplicated type resolution\n- declare_global_data() merging singleton/rwlock global declarations\n- validate_contract_list() + extract_fn_contracts() for contract helpers\n- CompiledBinary struct centralizing test helper tempdir+compile pattern\n- PlutoType::map_inner_types()/any_inner_type() simplifying type recursion\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Add Stmt::Yield, TypeExpr::Stream, PlutoType::Stream scaffolding\n\nLexer: yield/stream tokens. AST: Stmt::Yield, TypeExpr::Stream,\nFunction.is_generator. Type system: PlutoType::Stream with Display,\nmangle, resolve, substitute. All walker passes updated with new arms.\nTypeck: generators/current_generator_elem fields in TypeEnv, basic\nStmt::Yield validation in check_stmt. Codegen: pluto_to_cranelift,\nneeds_deep_copy, resolve_type_expr_to_pluto for Stream. All 1069\nexisting tests pass.\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Complete typeck validation for generators\n\n- Set current_generator_elem in check_function_body for generator functions\n- Pass Void as expected return type for generator bodies (they yield, not return)\n- Reject return-with-value in generators with clear error message\n- Add PlutoType::Stream to for-loop iterable type matching\n- Clear generator context when entering closure bodies (prevent yield in closures)\n- Register generators in env.generators during function registration\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Implement generator codegen: creator, next (state machine), for-in-stream\n\nGenerator functions compile to two Cranelift functions:\n- Creator: allocates gen object [next_fn_ptr|state|done|result|params...|locals...],\n  stores next_fn_ptr and params, returns gen_ptr\n- Next: state machine dispatch with brif chain, saves/restores locals across\n  yield points, sets done flag on completion\n\nFor-in-stream iteration: loads next_fn_ptr from gen_ptr[0], calls indirect,\nchecks done flag, extracts result — follows the receiver iteration pattern.\n\nGenerator-aware lowering for if/while/for inside generator bodies ensures\nyields in control flow are handled correctly with proper save/restore.\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Fix generator codegen bugs, add 24 integration tests and example\n\nFix variable shadowing: intercept Stmt::Let in generators to use\npre-declared variables instead of lower_let() creating new ones.\nFix param persistence: save/restore params across yield points.\nFix early return: fill unused resume blocks with done terminators.\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Add CI testing guidance to CLAUDE.md\n\nInstruct agents to use CI (via PR) for full test suite runs instead of\nrunning cargo test locally. Local runs limited to individual test files.\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Expand enum tests from 8 to 96 with comprehensive coverage\n\nCovers: basic construction, field type variations (bool/float/string/array/\nclass/nested-enum), enums in various positions (arrays, class fields, closures,\nloops, function chains), match edge cases (shadowing, renaming, nesting,\nbreak/continue, all-arms-return), generic enums (basic, multi-instantiation,\ntwo type params, bounds), nullable enums, equality, complex patterns (state\nmachine), and 28 negative tests (construction errors, match errors, type\nerrors, generic errors).\n\nDocuments 5 compiler gaps as negative tests:\n- Enum variant fields referencing class types (forward reference)\n- Self-referential enums\n- Closure return type inference with match\n- None literal coercion in enum field context\n- If-as-expression not supported\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Expand trait tests from 17 to 57 (batches 1-2)\n\nBatch 1: Core trait features — multiple methods, mixed required/default,\nvoid/string returns, multi-type params, typed arrays (gap), class fields\n(gap), empty trait, triple indirection, vtable ordering, template method\npattern, recursive dispatch, string interpolation with dispatch.\n\nBatch 2: Negative tests & edge cases — wrong param count/type/extra,\nimpl class/enum name, non-implementing class, non-trait method on handle,\nfield access on handle, primitive/incompatible assignment, duplicate impl\n(gap: allowed), forward reference, class/enum types in trait signatures\n(gap), closure/enum params, many params stress test, default calling\nfree function.\n\nCompiler gaps documented as negative tests:\n- Trait-typed array push doesn't coerce concrete class\n- Trait-typed class field doesn't coerce concrete class\n- Duplicate trait in impl list silently accepted\n- Class/enum types in trait method signatures not found (forward ref)\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Expand trait tests to 95 (batches 3-4: closures, errors, generics, diamond)\n\nBatch 3: closures as capture, spawn dispatch, loop stress, method returns\ntrait, method takes trait param, default with loop, default two classes,\nbool/float returns, same object two handles, negative tests for unknown\ntypes in signatures, five traits on one class, method named len, long\nnames, param shadows builtin.\n\nBatch 4: diamond two defaults (rejected), diamond class overrides,\ndiamond different sigs (rejected), three traits same method, generic\nclass impl, generic two instantiations, error handling catch, error\npropagation, self method call in dispatch, four classes same trait,\ndefault not overridden plus overridden, nullable return/param, negative\ntests for print/map-key/self-referential trait type, array param,\nclosure return, trait equality (compiler gap: pointer comparison).\n\nCompiler gaps documented:\n- Self-referential trait type in method params not resolved\n- Trait handle == compares pointers (probably unintentional)\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Expand trait tests to 153 (batches 5-7: dispatch patterns, negatives, DI)\n\nBatch 5 (20 tests): reassign trait variable, dispatch in if/while/for\nconditions, multiple trait params, nested dispatch chain, nested function\ncalls, recursive with handle, dispatch result as array index/arithmetic/\nstring concat, 5-deep call chain, two handles same scope, string interp\nin method body, while/for loops in body, object creation, array ops,\nall-default trait, default calls default, 10-method vtable stress.\n\nBatch 6 (19 tests): trait same name as class (gap), duplicate trait name\n(gap), missing one of two methods, wrong return type on one method, map\nparam, string/int in interpolation, for-range with dispatch, many fields\ndispatch, different field counts, bool/float params, type cast, default\nwith if/else, free function in body, string comparison, nested if, method\nmodifies array, 6-class vtable stress, string fields heap dispatch.\n\nBatch 7 (19 tests): DI bracket deps + trait, generic field access, match\narm dispatch, bitwise ops, default string return, negative literals,\nearly return guard, two-trait cross dispatch, string method chain, both\nif branches dispatch, let binding reuse, enum forward ref (gap), concrete\nand trait same method, two traits dispatched separately, dispatch return\nas param, accumulate across calls, trait handle as return value.\n\nCompiler gaps documented:\n- Trait and class with same name allowed (no rejection)\n- Two traits with same name allowed (second silently overwrites)\n- Enum type in trait method return/param position not resolved (forward ref)\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Expand trait tests to 172 (batch 8: closures, factories, stress tests)\n\nBatch 8 (19 tests): closure captures trait handle, closure param takes\ntrait, factory function pattern, method called on return value, interleaved\nmethod calls, dispatch preserves class state, mixed string/int fields,\ndispatch across scopes, multiple string params, fibonacci computation,\ndefault method with match, zero/negative returns, 100-iteration loop\nstress, trait method without self (compiler crash bug), 3 generic\ninstantiations same trait, conditional raise, 8-class vtable stress,\nstring length after dispatch, bool in && condition.\n\nCompiler bugs found:\n- Trait method without self crashes compiler (panic in register.rs:1194)\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Expand trait tests to 190 (batch 9: corner cases, method chains, negatives)\n\nBatch 9 (18 tests): reassign trait var three times, math builtins\nin method body, min/max clamping, ternary-style dispatch, two traits\nsame method name class satisfies both, default chain (a->b->c),\noverride middle of chain, negate dispatch result, compare two dispatches,\ntrait method calls non-trait helper, default returns empty string, zero\nand max-int returns, two classes same fields different behavior, unary\nnot on dispatch, three traits dispatched independently, method body with\nbreak/early-return, wrong trait method call rejected.\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Expand trait tests to 230 (batches 10-11: recursion, for loops, string interp, arrays, defaults, negatives)\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Expand trait tests to 268 (batches 12-13: mut self gaps, void methods, generics, recursion, arrays)\n\nNew compiler gaps documented:\n- mut self not parsed in trait method declarations\n- Void method + non-void method in same trait fails to parse\n- Nested field access (self.inner.val) treated as enum reference\n- Void return assignable to let variable (no error)\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Expand trait tests to 286 (batch 14): error handling, closures, complex dispatch\n\nBatch 14 covers error handling + traits (raise/catch through dispatch), closure\nfactories, catch blocks with dispatch, strategy pattern, complex boolean logic,\nand field+param interleaving. Documents closure-with-trait-param runtime crash.\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Expand trait tests to 315 (batches 15-16) and remove 11 duplicates\n\nBatch 15: method ordering, field layout variations, self-calls, negatives\nBatch 16: trait+enum interaction, deep nesting, stress, more negatives\n\nDedup audit removed 11 functionally duplicate tests:\n- trait_default_method_returns_string (subset of string_return)\n- trait_default_method_overridden_with_string (subset of string_return)\n- trait_default_calls_another_default_no_override (dup of original)\n- trait_dispatch_result_used_as_array_index (dup of as_array_index)\n- trait_dispatch_in_array_index_expression (third array index dup)\n- trait_dispatch_result_negated_via_subtraction (dup of negated)\n- trait_dispatch_in_if_condition_bool_flag (dup of if_condition)\n- trait_method_calls_own_method_on_self (dup of calls_other)\n- trait_dispatch_handles_zero_field_class (dup of zero_value_field)\n- trait_five_classes_same_trait_sequential_dispatch (redundant scale)\n\nNew compiler gaps documented:\n- Enum types in trait method signatures cause \"unknown type\" error\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Expand trait tests to 335 (batch 17): sets, nullable, generics, error edge cases\n\nBatch 17 covers previously untested feature combinations:\n- Set<T> params and returns through trait dispatch\n- Nullable types (T?, none, ? propagation) in trait methods\n- Multiple error types raised in trait methods\n- Error handling in default methods\n- Generic class with type bounds implementing traits\n- Map return and mutation through dispatch\n- For-range and for-array in trait method bodies\n- Same object through two different trait handles in one call\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Expand trait tests to 355 (batch 18: channels, negatives, edge cases)\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Expand trait tests to 376 (batch 19: maps, sets, nullable, casting, complex patterns)\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Expand trait tests to 396 (batch 20: vtable stress, recursive dispatch, generics, param validation)\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Expand trait tests to 415 (batch 21: contracts, errors, reassignment, default-only, nested dispatch)\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Expand trait tests to 434 (batch 22: type casting, maps, bitwise, spawn, string ops, negatives)\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Expand trait tests to 453 (batch 23: closures, DI, generics, contracts, error propagation)\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Expand trait tests to 473 (batch 24: array dispatch, argument chains, void ordering, boundary)\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Expand trait tests to 493 (batch 25: class return, string methods, dispatch arithmetic, error catch, trait params)\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Add test dependency hash tracking to DerivedInfo for incremental test execution\n\n- Add test_dep_hashes field to DerivedInfo (indexed by display_name)\n- Compute stable hashes for each test during program analysis\n- Fix SDK match statements to handle new Yield and Stream types\n- Update MCP serialize.rs to handle Stream type in type_expr_to_string\n\nThis enables the foundation for Phase 3 incremental test execution where tests\ncan be skipped if their dependencies haven't changed. Uses display_name as the\nstable key for caching since it's consistent across re-parses.\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Expand trait tests to 513 (batch 26: empty trait, spawn, large params, defaults chain, comprehensive final)\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Add logging module to stdlib (std.log)\n\nImplements structured logging with configurable log levels:\n- Level enum: Debug, Info, Warn, Error\n- Functions: debug, info, warn, log_error for basic logging\n- set_level/get_level for controlling output\n- Runtime functions in builtins.c for log output to stderr with timestamps\n\nExample:\n  import std.log\n  log.set_level(log.Level.Debug)\n  log.info(\"Application started\")\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* Add missing examples: errors, modules, pattern_matching\n\nCreated three comprehensive examples as specified in the compiler hardening plan:\n\n1. errors - Demonstrates typed error system:\n   - Error declarations with multiple error types\n   - raise to throw errors\n   - ! postfix for error propagation\n   - catch with wildcard error handling\n   - Shorthand catch with default values\n   - Compiler-inferred error-ability (no annotations)\n\n2. modules - Demonstrates module system:\n   - import for importing modules\n   - pub visibility for exported items\n   - Module organization with separate files\n   - Accessing public functions and classes from imports\n   - Private items hidden from importers\n\n3. pattern_matching - Demonstrates enum matching:\n   - Unit variants (no data)\n   - Data-carrying variants with field destructuring\n   - Mixed variants (unit and data)\n   - Exhaustiveness checking\n   - Nested pattern matching within match arms\n\nAll three examples tested and working. Updated examples/README.md with entries in alphabetical order.\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Add logging to ambient-di and concurrency examples\n\n- ambient-di: Created Logger wrapper class to use std.log in ambient DI pattern\n- concurrency: Added logging throughout task lifecycle with DEBUG, INFO, and WARN levels\n- Verified logging output to stderr with timestamps, works correctly with concurrent execution\n\nCo-Authored-By: Claude Haiku 4.5 <noreply@anthropic.com>\n\n* Add comprehensive mutability enforcement tests\n\nCreated tests/integration/mutability.rs with 13 tests covering:\n\nCallee-side enforcement (mut self in method definitions):\n- Reject field assignment in non-mut-self methods\n- Reject field assignment in conditionals and loops\n- Reject calling mut-self methods from non-mut-self methods\n- Allow field assignment in mut-self methods\n- Allow mut-self methods calling other mut-self methods\n\nCaller-side enforcement (let mut at call sites):\n- Reject mut-method calls on immutable bindings\n- Reject field assignment on immutable bindings\n- Allow mut-method calls on mutable bindings\n- Allow field assignment on mutable bindings\n\nMixed scenarios:\n- Immutable methods allowed on both mutable and immutable bindings\n- Method chaining with mut-self methods on mutable bindings\n\nAll tests pass. Mut self enforcement is fully implemented and working.\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Implement real dependency hash computation for tests\n\n- Replace placeholder test name hashing with transitive dependency tracking\n- Collect all functions, classes, and enums that a test depends on\n- Walk AST to find all Call, StructLit, EnumUnit/Data expressions\n- Hash function bodies, class fields/methods, and enum variants\n- Sort dependencies for stable hashing across runs\n\nThis provides the foundation for incremental test execution by detecting\nwhich tests are affected by code changes.\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Reorganize codegen/lower.rs into lower/mod.rs module\n\nMoved src/codegen/lower.rs to src/codegen/lower/mod.rs to prepare for future modularization. This 4,546-line file contains all IR lowering logic and is a candidate for splitting into submodules (stmt lowering, expr lowering, contracts, generators).\n\nThe module directory structure makes it easier to add submodules in the future without breaking imports.\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Clean up Zed editor integration: remove generated tree-sitter files\n\n- Removed generated parser.c and tree-sitter grammar artifacts (25k+ lines)\n- Renamed extension to pluto-legacy (LSP-based editor support is the future)\n- Transitioning from tree-sitter to language server protocol\n- These generated files should be built/downloaded on demand, not stored in repo\n\nCo-Authored-By: Claude Haiku 4.5 <noreply@anthropic.com>\n\n* Add test cache management system\n\n- Create cache module for storing/loading test dependency hashes\n- Cache stored in .pluto-cache/test-hashes/<file>.json\n- Validate cache by hashing source file content\n- Cache entry includes source hash, test hashes, and timestamp\n- Add tests for hash stability and cache entry creation\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Implement incremental test execution with caching\n\n- Add use_cache parameter to compile_file_for_tests\n- Implement filter_tests_by_cache to compare current vs cached hashes\n- Filter program.test_info to only include tests with changed dependencies\n- Save cache after successful test compilation\n- Add --no-cache flag to 'plutoc test' command to force running all tests\n- Add status output showing \"X of Y tests (Z skipped, unchanged)\"\n- Exit early with success if all tests are skipped\n\nTests now skip unchanged code automatically, significantly speeding up\ntest iteration during development.\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Update Zed extension to use correct GitHub URL for tree-sitter grammar\n\n* Fix merge conflicts and compilation issues\n\n- Remove duplicate PlutoType::Stream match arm in types.rs\n- Add missing use_cache parameter to compile_file_for_tests call\n- Use Span::dummy() for Stream TypeExpr (consistent with other variants)\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n* Suppress unused_assignments warning in pretty printer\n\nThe has_output variable in emit_program is used in a macro pattern\nwhere the final assignment is never read. This is expected behavior\nfor the separator macro pattern.\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Test <test@test.com>\nCo-authored-by: Claude Opus 4.6 <noreply@anthropic.com>",
          "timestamp": "2026-02-11T00:55:10-06:00",
          "tree_id": "8b04d1ca141f7aeb7df662145812e349718b6a73",
          "url": "https://github.com/Mkerian10/pluto/commit/578d01fcb1344ee8f07dd756cfb74a7dd85dca78"
        },
        "date": 1770793022586,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "fib",
            "value": 71,
            "unit": "ms"
          },
          {
            "name": "loop_sum",
            "value": 62,
            "unit": "ms"
          },
          {
            "name": "string_concat",
            "value": 561,
            "unit": "ms"
          },
          {
            "name": "array_push",
            "value": 6,
            "unit": "ms"
          },
          {
            "name": "array_iter",
            "value": 3,
            "unit": "ms"
          },
          {
            "name": "class_method",
            "value": 6,
            "unit": "ms"
          },
          {
            "name": "closure_call",
            "value": 25,
            "unit": "ms"
          },
          {
            "name": "trait_dispatch",
            "value": 1442,
            "unit": "ms"
          },
          {
            "name": "gc_churn",
            "value": 140,
            "unit": "ms"
          },
          {
            "name": "gc_binary_trees",
            "value": 1998,
            "unit": "ms"
          },
          {
            "name": "gc_string_pressure",
            "value": 75,
            "unit": "ms"
          },
          {
            "name": "sieve",
            "value": 11,
            "unit": "ms"
          },
          {
            "name": "bounce",
            "value": 47,
            "unit": "ms"
          },
          {
            "name": "towers",
            "value": 1760,
            "unit": "ms"
          },
          {
            "name": "permute",
            "value": 102,
            "unit": "ms"
          },
          {
            "name": "queens",
            "value": 157,
            "unit": "ms"
          },
          {
            "name": "fannkuch_redux",
            "value": 1330,
            "unit": "ms"
          },
          {
            "name": "spectral_norm",
            "value": 53,
            "unit": "ms"
          },
          {
            "name": "nbody",
            "value": 8978,
            "unit": "ms"
          },
          {
            "name": "mandelbrot",
            "value": 286,
            "unit": "ms"
          },
          {
            "name": "monte_carlo",
            "value": 1433,
            "unit": "ms"
          },
          {
            "name": "storage",
            "value": 797,
            "unit": "ms"
          },
          {
            "name": "list",
            "value": 88,
            "unit": "ms"
          },
          {
            "name": "fft",
            "value": 2392,
            "unit": "ms"
          },
          {
            "name": "sor",
            "value": 575,
            "unit": "ms"
          },
          {
            "name": "sparse_matmul",
            "value": 541,
            "unit": "ms"
          },
          {
            "name": "lu",
            "value": 4054,
            "unit": "ms"
          },
          {
            "name": "json_parse",
            "value": 1178,
            "unit": "ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "33331268+Mkerian10@users.noreply.github.com",
            "name": "Matthew Kerian",
            "username": "Mkerian10"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bd64d0222b5f4372adcd8436550de121184e9ffd",
          "message": "Merge pull request #23 from Mkerian10/stdlib-modules\n\nAdd stdlib modules and examples: strings, paths, env",
          "timestamp": "2026-02-11T01:35:45-06:00",
          "tree_id": "8ee25b34e18707715f9b55c2438c1b001ba91c1d",
          "url": "https://github.com/Mkerian10/pluto/commit/bd64d0222b5f4372adcd8436550de121184e9ffd"
        },
        "date": 1770795457253,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "fib",
            "value": 71,
            "unit": "ms"
          },
          {
            "name": "loop_sum",
            "value": 62,
            "unit": "ms"
          },
          {
            "name": "string_concat",
            "value": 749,
            "unit": "ms"
          },
          {
            "name": "array_push",
            "value": 5,
            "unit": "ms"
          },
          {
            "name": "array_iter",
            "value": 3,
            "unit": "ms"
          },
          {
            "name": "class_method",
            "value": 6,
            "unit": "ms"
          },
          {
            "name": "closure_call",
            "value": 25,
            "unit": "ms"
          },
          {
            "name": "trait_dispatch",
            "value": 1419,
            "unit": "ms"
          },
          {
            "name": "gc_churn",
            "value": 142,
            "unit": "ms"
          },
          {
            "name": "gc_binary_trees",
            "value": 2024,
            "unit": "ms"
          },
          {
            "name": "gc_string_pressure",
            "value": 74,
            "unit": "ms"
          },
          {
            "name": "sieve",
            "value": 10,
            "unit": "ms"
          },
          {
            "name": "bounce",
            "value": 47,
            "unit": "ms"
          },
          {
            "name": "towers",
            "value": 1762,
            "unit": "ms"
          },
          {
            "name": "permute",
            "value": 103,
            "unit": "ms"
          },
          {
            "name": "queens",
            "value": 157,
            "unit": "ms"
          },
          {
            "name": "fannkuch_redux",
            "value": 1357,
            "unit": "ms"
          },
          {
            "name": "spectral_norm",
            "value": 52,
            "unit": "ms"
          },
          {
            "name": "nbody",
            "value": 9073,
            "unit": "ms"
          },
          {
            "name": "mandelbrot",
            "value": 286,
            "unit": "ms"
          },
          {
            "name": "monte_carlo",
            "value": 1434,
            "unit": "ms"
          },
          {
            "name": "storage",
            "value": 810,
            "unit": "ms"
          },
          {
            "name": "list",
            "value": 88,
            "unit": "ms"
          },
          {
            "name": "fft",
            "value": 2387,
            "unit": "ms"
          },
          {
            "name": "sor",
            "value": 576,
            "unit": "ms"
          },
          {
            "name": "sparse_matmul",
            "value": 542,
            "unit": "ms"
          },
          {
            "name": "lu",
            "value": 4068,
            "unit": "ms"
          },
          {
            "name": "json_parse",
            "value": 1163,
            "unit": "ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "33331268+Mkerian10@users.noreply.github.com",
            "name": "Matthew Kerian",
            "username": "Mkerian10"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "835942ecf4fc6ce8510210b20d43544b4e1c1a4c",
          "message": "Add watch mode for automatic recompilation (#24)\n\nImplement watch mode to automatically detect file changes and recompile/rerun Pluto programs. This significantly reduces the feedback loop during development.\n\nFeatures:\n- File watching with notify crate (cross-platform)\n- Debouncing (100ms window after last change)\n- Graceful process termination (SIGTERM → SIGKILL)\n- Transitive import tracking via module resolution\n- Continue watching even on compilation errors\n- Optional --no-clear flag to preserve output\n\nUsage:\n  plutoc watch run <file> [--stdlib <path>] [--no-clear]\n\nCo-authored-by: Test <test@test.com>\nCo-authored-by: Claude Sonnet 4.5 <noreply@anthropic.com>",
          "timestamp": "2026-02-11T07:54:38Z",
          "tree_id": "2c43bc85d6b830792ee27e147c6d1328418a8512",
          "url": "https://github.com/Mkerian10/pluto/commit/835942ecf4fc6ce8510210b20d43544b4e1c1a4c"
        },
        "date": 1770796603229,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "fib",
            "value": 71,
            "unit": "ms"
          },
          {
            "name": "loop_sum",
            "value": 62,
            "unit": "ms"
          },
          {
            "name": "string_concat",
            "value": 578,
            "unit": "ms"
          },
          {
            "name": "array_push",
            "value": 5,
            "unit": "ms"
          },
          {
            "name": "array_iter",
            "value": 3,
            "unit": "ms"
          },
          {
            "name": "class_method",
            "value": 6,
            "unit": "ms"
          },
          {
            "name": "closure_call",
            "value": 25,
            "unit": "ms"
          },
          {
            "name": "trait_dispatch",
            "value": 1428,
            "unit": "ms"
          },
          {
            "name": "gc_churn",
            "value": 138,
            "unit": "ms"
          },
          {
            "name": "gc_binary_trees",
            "value": 2050,
            "unit": "ms"
          },
          {
            "name": "gc_string_pressure",
            "value": 75,
            "unit": "ms"
          },
          {
            "name": "sieve",
            "value": 10,
            "unit": "ms"
          },
          {
            "name": "bounce",
            "value": 47,
            "unit": "ms"
          },
          {
            "name": "towers",
            "value": 1765,
            "unit": "ms"
          },
          {
            "name": "permute",
            "value": 103,
            "unit": "ms"
          },
          {
            "name": "queens",
            "value": 157,
            "unit": "ms"
          },
          {
            "name": "fannkuch_redux",
            "value": 1330,
            "unit": "ms"
          },
          {
            "name": "spectral_norm",
            "value": 52,
            "unit": "ms"
          },
          {
            "name": "nbody",
            "value": 8985,
            "unit": "ms"
          },
          {
            "name": "mandelbrot",
            "value": 287,
            "unit": "ms"
          },
          {
            "name": "monte_carlo",
            "value": 1435,
            "unit": "ms"
          },
          {
            "name": "storage",
            "value": 821,
            "unit": "ms"
          },
          {
            "name": "list",
            "value": 91,
            "unit": "ms"
          },
          {
            "name": "fft",
            "value": 2381,
            "unit": "ms"
          },
          {
            "name": "sor",
            "value": 576,
            "unit": "ms"
          },
          {
            "name": "sparse_matmul",
            "value": 545,
            "unit": "ms"
          },
          {
            "name": "lu",
            "value": 4071,
            "unit": "ms"
          },
          {
            "name": "json_parse",
            "value": 1153,
            "unit": "ms"
          }
        ]
      }
    ]
  }
}