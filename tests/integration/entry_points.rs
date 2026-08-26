//! Entry-point semantics (rfc-entry-point-semantics.md, issue #127).
//!
//! main is the process boundary: `int`/`int?` returns are the exit code
//! (`none` maps to 0), `?` propagation from main is a successful early
//! exit, escaped errors exit 1, and only exit-code-shaped return types
//! are allowed.
mod common;
use common::{compile_and_run, compile_and_run_stdout, compile_should_fail_with};

// ── Exit codes from return values ────────────────────────────────────────────

#[test]
fn main_void_exits_zero() {
    assert_eq!(compile_and_run("fn main(){\n    print(1)\n}"), 0);
}

#[test]
fn main_int_return_is_exit_code() {
    assert_eq!(compile_and_run("fn main() int {\n    return 3\n}"), 3);
    assert_eq!(compile_and_run("fn main() int {\n    return 0\n}"), 0);
}

#[test]
fn main_nullable_none_exits_zero() {
    assert_eq!(compile_and_run("fn main() int? {\n    return none\n}"), 0);
}

#[test]
fn main_nullable_value_is_exit_code() {
    assert_eq!(compile_and_run("fn main() int? {\n    return 5\n}"), 5);
}

// ── ? in main ────────────────────────────────────────────────────────────────

#[test]
fn question_in_void_main_none_exits_zero() {
    let code = compile_and_run(
        "fn get() int? {\n    return none\n}\n\nfn main(){\n    let v = get()?\n    print(v)\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn question_in_void_main_value_continues() {
    let out = compile_and_run_stdout(
        "fn get() int? {\n    return 7\n}\n\nfn main(){\n    let v = get()?\n    print(v)\n}",
    );
    assert_eq!(out.trim(), "7");
}

#[test]
fn question_in_nullable_main_composes() {
    // Propagated none becomes main's return value, which maps to exit 0
    let code = compile_and_run(
        "fn get() int? {\n    return none\n}\n\nfn main() int? {\n    let v = get()?\n    print(v)\n    return 9\n}",
    );
    assert_eq!(code, 0);
}

// ── Errors at the boundary ───────────────────────────────────────────────────

#[test]
fn raise_in_main_exits_one() {
    assert_eq!(
        compile_and_run("error E {}\n\nfn main(){\n    raise E {}\n}"),
        1
    );
}

#[test]
fn propagated_error_from_main_exits_one() {
    assert_eq!(
        compile_and_run(
            "error E {}\n\nfn f() int {\n    raise E {}\n}\n\nfn main(){\n    let v = f()!\n    print(v)\n}"
        ),
        1
    );
}

#[test]
fn caught_error_maps_to_chosen_exit_code() {
    assert_eq!(
        compile_and_run(
            "error E {}\n\nfn run() {\n    raise E {}\n}\n\nfn main() int {\n    run() catch e {\n        return 2\n    }\n    return 0\n}"
        ),
        2
    );
}

#[test]
fn unhandled_fallible_call_in_main_still_rejected() {
    // main is not an implicit catch-all: calls require ! or catch
    compile_should_fail_with(
        "error E {}\n\nfn f() {\n    raise E {}\n}\n\nfn main(){\n    f()\n}",
        "must be handled with ! or catch",
    );
}

// ── Return-type restriction ──────────────────────────────────────────────────

#[test]
fn main_string_return_rejected() {
    compile_should_fail_with(
        "fn main() string {\n    return \"hi\"\n}",
        "main must return void, int, or int?",
    );
}

#[test]
fn main_nullable_string_return_rejected() {
    compile_should_fail_with(
        "fn main() string? {\n    return \"hi\"\n}",
        "main must return void, int, or int?",
    );
}

#[test]
fn main_float_return_rejected() {
    compile_should_fail_with(
        "fn main() float {\n    return 1.5\n}",
        "main must return void, int, or int?",
    );
}

// ── App entry points ─────────────────────────────────────────────────────────

#[test]
fn app_main_must_be_void() {
    compile_should_fail_with(
        "app A {\n    fn main(self) int {\n        return 3\n    }\n}",
        "app main method must not have a return type",
    );
}
