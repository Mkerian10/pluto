//! Differential tests across GC backends. Each program runs under every
//! backend with the heap verifier on and torture collections, and must print
//! exactly what the reference mark-sweep backend prints. Under `tlh` the
//! verifier also checks invariant I (no shared object points at a private
//! one) at every global collection, and under `incr` that no marked object
//! points at an unmarked one when marking ends, so a missing barrier aborts.

use pluto::GcBackend;
use std::path::Path;
use std::process::Command;

fn run(program: &str, gc: GcBackend, env: &[(&str, &str)]) -> String {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/integration/gc_programs").join(program);
    let dir = tempfile::tempdir().unwrap();
    // Copy into its own directory: sibling .pt files would be merged.
    let main = dir.path().join("main.pt");
    std::fs::copy(&src, &main).unwrap();
    let bin = dir.path().join("bin");
    pluto::compile_file_with_options(&main, &bin, None, gc, true)
        .unwrap_or_else(|e| panic!("compile {program} under {}: {e}", gc.name()));
    let out = Command::new(&bin).envs(env.iter().copied()).output().unwrap();
    assert!(
        out.status.success(),
        "{program} under {} failed:\n{}",
        gc.name(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `program` under `gc`, with the verifier on, with and without torture
/// collections, must print what the reference mark-sweep backend prints.
/// One test per (program, backend) keeps each within the per-test timeout.
fn differential(program: &str, gc: GcBackend) {
    let expected = run(program, GcBackend::MarkSweep, &[]);
    // Torture collections only for the backends that are default
    // candidates: under verify, the lab-only variants' brute-force checks
    // make torture runs too slow for CI's macOS runners.
    let lab_only = matches!(gc, GcBackend::Lazy | GcBackend::Gen | GcBackend::ParMark | GcBackend::Tlab);
    let tortures: &[&str] = if lab_only { &["0"] } else { &["0", "50"] };
    for &torture in tortures {
        let got = run(program, gc, &[("PLUTO_GC_VERIFY", "1"), ("PLUTO_GC_TORTURE", torture)]);
        assert_eq!(got, expected, "{program} under {} (torture={torture})", gc.name());
    }
}

macro_rules! differential_tests {
    ($program:literal, $module:ident) => {
        mod $module {
            use super::*;
            #[test] fn lazy() { differential($program, GcBackend::Lazy) }
            #[test] fn generational() { differential($program, GcBackend::Gen) }
            #[test] fn parmark() { differential($program, GcBackend::ParMark) }
            #[test] fn tlab() { differential($program, GcBackend::Tlab) }
            #[test] fn tlh() { differential($program, GcBackend::Tlh) }
            #[test] fn incr() { differential($program, GcBackend::Incr) }
            #[test] fn hybrid() { differential($program, GcBackend::Hybrid) }
        }
    };
}

// Values crossing threads through entities, tasks and channels.
differential_tests!("sharing.pt", sharing_through_entities_tasks_and_channels);

// Cuts snapshot-reachable objects out of not-yet-scanned containers and
// moves them into objects allocated mid-cycle: under `incr` every cut must
// reach the deletion log, which the verifier checks at the end of marking.
differential_tests!("satb.pt", references_moved_during_marking);

// Large containers whose elements move (remove_at, map deletion shifts)
// while `incr` scans them in chunks: moved elements must be logged.
differential_tests!("chunks.pt", elements_moved_behind_a_chunked_scan);

// Containers the runtime fills with plain stores (array slices) while their
// elements are young: no collection may see a container before the call
// that allocates it returns (once broke hybrid's clean-container flag).
differential_tests!("slices.pt", runtime_filled_containers_with_young_elements);

// Empty containers inside objects that get tenured: their backing stores
// must be promoted with them.
differential_tests!("empty.pt", empty_containers_in_tenured_objects);
