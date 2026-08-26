//! Select statement forms.
//!
//! `select { v = rx.recv() { ... } default { ... } }` — arms bind the
//! received value; `default` makes it non-blocking. A blocking select
//! (no default) is legal. Select is a statement, not an expression.
#[path = "../common.rs"]
mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

#[test]
fn select_with_default_runs() {
    let out = compile_and_run_stdout(
        "fn main(){\n    let (tx, rx) = chan<int>(1)\n    tx.send(7) catch e {}\n    select {\n        v = rx.recv() {\n            print(v)\n        }\n        default {\n            print(-1)\n        }\n    }\n}",
    );
    assert_eq!(out.trim(), "7");
}

#[test]
fn select_default_taken_when_empty() {
    let out = compile_and_run_stdout(
        "fn main(){\n    let (tx, rx) = chan<int>(1)\n    select {\n        v = rx.recv() {\n            print(v)\n        }\n        default {\n            print(-1)\n        }\n    }\n}",
    );
    assert_eq!(out.trim(), "-1");
}

#[test]
fn select_no_default_compiles() {
    // Blocking select is legal; run with a value already buffered
    let out = compile_and_run_stdout(
        "fn main(){\n    let (tx, rx) = chan<int>(1)\n    tx.send(42) catch e {}\n    select {\n        v = rx.recv() {\n            print(v)\n        }\n    }\n}",
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn select_multiple_arms() {
    let out = compile_and_run_stdout(
        "fn main(){\n    let (tx1, rx1) = chan<int>(1)\n    let (tx2, rx2) = chan<string>(1)\n    tx1.send(5) catch e {}\n    select {\n        a = rx1.recv() {\n            print(a)\n        }\n        b = rx2.recv() {\n            print(b)\n        }\n        default {\n            print(-1)\n        }\n    }\n}",
    );
    assert_eq!(out.trim(), "5");
}

#[test]
fn select_in_function() {
    let out = compile_and_run_stdout(
        "fn drain(rx: Receiver<int>) int {\n    select {\n        v = rx.recv() {\n            return v\n        }\n        default {\n            return -1\n        }\n    }\n    return -2\n}\n\nfn main(){\n    let (tx, rx) = chan<int>(1)\n    tx.send(9) catch e {}\n    print(drain(rx))\n}",
    );
    assert_eq!(out.trim(), "9");
}

#[test]
fn nested_select() {
    let out = compile_and_run_stdout(
        "fn main(){\n    let (tx1, rx1) = chan<int>(1)\n    let (tx2, rx2) = chan<int>(1)\n    tx1.send(1) catch e {}\n    tx2.send(2) catch e {}\n    select {\n        a = rx1.recv() {\n            select {\n                b = rx2.recv() {\n                    print(a + b)\n                }\n                default {\n                    print(a)\n                }\n            }\n        }\n        default {\n            print(-1)\n        }\n    }\n}",
    );
    assert_eq!(out.trim(), "3");
}

// ── Select is a statement, not an expression ─────────────────────────────────

#[test]
fn select_in_assignment_rejected() {
    compile_should_fail_with(
        "fn main(){\n    let (tx, rx) = chan<int>(1)\n    let x = select {\n        v = rx.recv() {\n            print(v)\n        }\n    }\n}",
        "unexpected token select in expression",
    );
}

#[test]
fn select_in_array_rejected() {
    compile_should_fail_with(
        "fn main(){\n    let (tx, rx) = chan<int>(1)\n    let arr = [select {\n        v = rx.recv() {\n            print(v)\n        }\n    }]\n}",
        "unexpected token select in expression",
    );
}

#[test]
fn select_in_binop_rejected() {
    compile_should_fail_with(
        "fn main(){\n    let (tx, rx) = chan<int>(1)\n    let x = 1 + select {\n        v = rx.recv() {\n            print(v)\n        }\n    }\n}",
        "unexpected token select in expression",
    );
}
