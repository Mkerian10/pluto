#!/usr/bin/env python3
"""Synthetic corpus generator for compiler performance auditing.

Generates a single large .pt program shaped to exercise the typecheck-phase
analyses added between PR #333 and the verification-engine work (flow facts,
strict invariant discharge, dominance, raise summaries + call-site shrinking,
two-state ensures). The program is deliberately contract-heavy -- it is a
stress shape, not representative user code.

Modes (each a superset of the previous, with bodies kept textually identical
wherever possible so mode-vs-mode timing diffs isolate the marginal cost of
the declared features rather than differing body shapes):

  plain       guards + arithmetic only; no errors, no invariants, no ensures.
              Old-syntax-compatible: compiles at both 885b934 and master.
  errors      plain + error decls, raise-under-guard, `!` propagation, catch.
              Exercises error inference + raise summaries + shrinking.
              Compiles at both commits.
  invariants  errors + class invariants with guarded writes (discharge,
              dominance, facts). Invariant syntax exists at both commits
              (runtime-checked at 885b934, statically discharged on master).
  full        invariants + proof-form `ensures ... old(...)` methods.
              Master-only (885b934 rejects `ensures` at parse).

Usage: perf_audit_gen.py --mode plain|errors|invariants|full [--classes N]
       [--funcs N] [--out FILE]
"""

import argparse


def gen_class(i: int, mode: str) -> str:
    """A class with two int fields. In invariant modes, `a` carries bounds
    invariants and is only written under guards that make the writes provable;
    `b` is invariant-free so ensures-methods over it stay provable."""
    lines = [f"class C{i} {{", "    a: int", "    b: int", ""]
    if mode in ("invariants", "full"):
        lines += [
            "    invariant self.a >= 0",
            "    invariant self.a <= 1000",
            "",
        ]
    # Guarded write: provable against both bounds (a < 1000 => a+1 <= 1000,
    # a >= 0 => a+1 >= 0). The guard is present in ALL modes so bodies are
    # identical; only the invariant lines differ.
    lines += [
        "    fn bump(mut self) {",
        "        if self.a < 1000 {",
        "            self.a = self.a + 1",
        "        }",
        "    }",
        "",
        "    fn drop_one(mut self) {",
        "        if self.a > 0 {",
        "            self.a = self.a - 1",
        "        }",
        "    }",
        "",
    ]
    if mode == "full":
        lines += [
            f"    fn tally(mut self) ensures self.b == old(self.b) + {i % 7 + 1} {{",
            f"        self.b = self.b + {i % 7 + 1}",
            "    }",
            "",
        ]
    else:
        lines += [
            "    fn tally(mut self) {",
            f"        self.b = self.b + {i % 7 + 1}",
            "    }",
            "",
        ]
    lines += [
        "    fn total(self) int {",
        "        return self.a + self.b",
        "    }",
        "}",
        "",
    ]
    return "\n".join(lines)


def gen_error(i: int) -> str:
    return f"error E{i} {{ code: int }}\n"


def gen_func(i: int, n_classes: int, mode: str) -> str:
    """Free functions with guard-heavy control flow. In error modes, half are
    fallible (raise under a guard, callers propagate with `!`), creating many
    call sites for raise-summary shrinking."""
    cls = i % n_classes
    fallible = mode in ("errors", "invariants", "full") and i % 2 == 0
    lines = []
    if fallible:
        err = i % 8
        # Note: fallibility is *inferred* -- declarations use plain return
        # types; `!` appears only at propagating call sites.
        lines += [
            f"fn f{i}(x: int) int {{",
            "    if x < 0 {",
            f"        raise E{err} {{ code: x }}",
            "    }",
            "    if x > 100 {",
            f"        return x - {i % 13 + 1}",
            "    }",
            f"    return x * 2 + {i % 5}",
            "}",
            "",
        ]
        # A fallible caller that propagates: another call site for inference.
        lines += [
            f"fn g{i}(x: int) int {{",
            f"    let v = f{i}(x)!",
            "    if v > 10 {",
            "        return v - 1",
            "    }",
            "    return v + 1",
            "}",
            "",
        ]
    else:
        lines += [
            f"fn f{i}(x: int) int {{",
            "    if x < 0 {",
            f"        return 0 - x + {i % 5}",
            "    }",
            "    if x > 100 {",
            f"        return x - {i % 13 + 1}",
            "    }",
            f"    return x * 2 + {i % 5}",
            "}",
            "",
        ]
        lines += [
            f"fn g{i}(x: int) int {{",
            f"    let v = f{i}(x)",
            "    if v > 10 {",
            "        return v - 1",
            "    }",
            "    return v + 1",
            "}",
            "",
        ]
    # A driver that constructs and mutates a class: construction + write
    # sites feed the discharge engine in invariant modes.
    lines += [
        f"fn use{i}() int {{",
        f"    let mut c = C{cls} {{ a: {i % 900}, b: 0 }}",
        "    c.bump()",
        "    c.tally()",
        "    c.drop_one()",
        "    return c.total()",
        "}",
        "",
    ]
    return "\n".join(lines)


def gen_main(n_funcs: int, mode: str) -> str:
    lines = ["fn main() {", "    let mut acc = 0"]
    for i in range(n_funcs):
        if mode in ("errors", "invariants", "full") and i % 2 == 0:
            if i % 4 == 0:
                lines.append(f"    let r{i} = g{i}({i}) catch 0")
            else:
                lines.append(f"    let r{i} = g{i}({i}) catch err {{ 0 - 1 }}")
            lines.append(f"    acc = acc + r{i}")
        else:
            lines.append(f"    acc = acc + g{i}({i})")
        lines.append(f"    acc = acc + use{i}()")
    lines += ["    print(acc)", "}", ""]
    return "\n".join(lines)


def generate(mode: str, n_classes: int, n_funcs: int) -> str:
    parts = [f"// Generated by scripts/perf_audit_gen.py --mode {mode}", ""]
    if mode in ("errors", "invariants", "full"):
        for i in range(8):
            parts.append(gen_error(i))
        parts.append("")
    for i in range(n_classes):
        parts.append(gen_class(i, mode))
    for i in range(n_funcs):
        parts.append(gen_func(i, n_classes, mode))
    parts.append(gen_main(n_funcs, mode))
    return "\n".join(parts)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--mode", choices=["plain", "errors", "invariants", "full"], required=True)
    ap.add_argument("--classes", type=int, default=40)
    ap.add_argument("--funcs", type=int, default=80)
    ap.add_argument("--out", default="/dev/stdout")
    args = ap.parse_args()
    src = generate(args.mode, args.classes, args.funcs)
    with open(args.out, "w") as f:
        f.write(src)


if __name__ == "__main__":
    main()
