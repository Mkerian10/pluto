# Fuzz seeds

Hand-written seed inputs for the text-based fuzz targets, focused on the
newer grammar surface: typestate `where` constraints, `must_release`,
state-carrying error declarations, `ensures` with `old()`, `guarded_by`
field clauses, `object` declarations, and `at` placement — plus
near-valid mutations of each (`mut_*.pt`).

The live corpora under `fuzz/corpus/` are machine-evolved and not
checked in. Seed a run with:

```bash
cargo +nightly fuzz run lex fuzz/corpus/lex fuzz/seeds/parse_src
cargo +nightly fuzz run parse_src fuzz/corpus/parse_src fuzz/seeds/parse_src
```

(libFuzzer reads inputs from every directory listed and writes new
discoveries to the first.)
