#![no_main]
use libfuzzer_sys::fuzz_target;

// Source-text fuzzer over the full frontend-for-analysis pipeline:
// lex → parse → module resolve (single file) → prelude → stages → ambient
// → generic traits/methods → contracts → typeck. No codegen.
//
// Seeded with the newer grammar surface (fuzz/corpus/parse_src): typestate
// `where` constraints, `must_release`, state-carrying error declarations,
// `ensures`/`old()`, `guarded_by` field clauses, `object` declarations,
// and `at` placement — plus near-valid mutations of each.
//
// Errors at any stage are fine (malformed input must be *rejected*, not
// crash); panics and aborts are findings. The pipeline runs on a 16MB
// stack thread, exactly like `pluto::compile_to_object`, so stack
// headroom matches production.
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    // Bound input size: libFuzzer explores depth faster on small inputs and
    // the deep-nesting ceiling is governed by the production stack size.
    if s.len() > 8192 {
        return;
    }
    let source = s.to_string();
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            let Ok(mut program) = pluto::parse_source(&source) else {
                return;
            };
            if pluto::modules::resolve_qualified_access_single_file(&mut program).is_err() {
                return;
            }
            if pluto::prelude::inject_prelude(&mut program).is_err() {
                return;
            }
            if pluto::stages::flatten_stage_hierarchy(&mut program).is_err() {
                return;
            }
            if pluto::ambient::desugar_ambient(&mut program).is_err() {
                return;
            }
            if pluto::generic_traits::instantiate_generic_traits(&mut program).is_err() {
                return;
            }
            if pluto::generic_methods::hoist_generic_methods(&mut program).is_err() {
                return;
            }
            if pluto::contracts::validate_contracts(&mut program).is_err() {
                return;
            }
            let _ = pluto::typeck::type_check(&mut program);
        })
        .expect("failed to spawn pipeline thread")
        .join()
        .expect("pipeline thread panicked"); // propagate panics as crashes
});
