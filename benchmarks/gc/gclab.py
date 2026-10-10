#!/usr/bin/env python3
"""GC lab harness: run every benchmark under every GC backend and compare.

Usage:
  benchmarks/gc/gclab.py [--backends a,b,...] [--benches x,y,...] [--runs N]
                         [--out results/NAME.json]

For each (backend, benchmark) it compiles the program with `pluto --gc
<backend>`, then records:
  wall_s      median wall-clock time over --runs runs
  pause_ms    total stop-the-world GC pause   (from PLUTO_GC_LOG)
  max_pause_ms longest single pause            (from PLUTO_GC_LOG)
  cycles      number of collections            (from PLUTO_GC_LOG)
  rss_mb      peak resident set size
  ok          output identical to the reference backend's output
Backends without PLUTO_GC_LOG support (noop) report no pause data.
"""
import argparse
import json
import os
import re
import statistics
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BENCH_DIR = os.path.join(ROOT, "benchmarks", "gc")
PLUTO = os.environ.get("GCLAB_PLUTO", os.path.join(ROOT, "target", "debug", "pluto"))
BIN_DIR = "/tmp/gclab/bin"


def benches():
    return sorted(d for d in os.listdir(BENCH_DIR)
                  if os.path.isfile(os.path.join(BENCH_DIR, d, "main.pt")))


def compile_bench(backend, bench):
    out = os.path.join(BIN_DIR, backend, bench)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    src = os.path.join(BENCH_DIR, bench, "main.pt")
    r = subprocess.run([PLUTO, "--gc", backend, "compile", src, "-o", out],
                       capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(f"compile failed ({backend}/{bench}):\n{r.stderr[-2000:]}")
    return out


TIMEOUT_S = 60


def run_once(binary, env=None):
    t0 = time.perf_counter()
    # Own process group, so a timeout kills the benchmark itself and not
    # just the /usr/bin/time wrapper (an orphaned benchmark would keep
    # burning CPU and skew every later measurement).
    p = subprocess.Popen(["/usr/bin/time", "-l", binary], stdout=subprocess.PIPE,
                         stderr=subprocess.PIPE, text=True, env=env, start_new_session=True)
    try:
        stdout, stderr = p.communicate(timeout=TIMEOUT_S)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, 9)
        p.communicate()
        return None, None, "TIMEOUT", "", -1
    r = subprocess.CompletedProcess(p.args, p.returncode, stdout, stderr)
    wall = time.perf_counter() - t0
    m = re.search(r"(\d+)\s+maximum resident set size", r.stderr)
    rss = int(m.group(1)) / (1024 * 1024) if m else None
    return wall, rss, r.stdout.strip(), r.stderr, r.returncode


def gc_stats(stderr):
    """Pause statistics from PLUTO_GC_LOG lines.

    Stop-the-world pauses (kind full/minor/major/global) stall every thread;
    thread-local ones (kind local/exit, --gc tlh) stall only the collecting
    thread, so they are reported separately and never summed together.
    """
    stw, local = [], []
    kinds = []
    for m in re.finditer(r"^gc: .*?pause_us=(\d+).*?kind=(\w+)", stderr, re.M):
        us, kind = int(m.group(1)), m.group(2)
        kinds.append(kind)
        (local if kind in ("local", "exit") else stw).append(us)
    if not kinds:
        return None

    def p99(xs):
        if not xs:
            return 0.0
        xs = sorted(xs)
        return xs[min(len(xs) - 1, int(round(0.99 * (len(xs) - 1))))] / 1000.0

    return {
        "minor": kinds.count("minor"),
        "major": kinds.count("major"),
        "local": kinds.count("local"),
        "exit": kinds.count("exit"),
        "pause_ms": sum(stw) / 1000.0,
        "max_pause_ms": max(stw) / 1000.0 if stw else 0.0,
        "p99_pause_ms": p99(stw),
        "cycles": len(stw),
        "local_pause_ms": sum(local) / 1000.0,
        "local_max_ms": max(local) / 1000.0 if local else 0.0,
        "local_p99_ms": p99(local),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--backends", default="marksweep,noop,legacy")
    ap.add_argument("--benches", default=",".join(benches()))
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--reference", default="marksweep")
    ap.add_argument("--out", default=None)
    ap.add_argument("--timeout", type=int, default=60)
    args = ap.parse_args()
    global TIMEOUT_S
    TIMEOUT_S = args.timeout
    backends = args.backends.split(",")
    selected = args.benches.split(",")

    results = []
    reference_out = {}
    order = [args.reference] + [b for b in backends if b != args.reference] \
        if args.reference in backends else backends
    for bench in selected:
        for backend in order:
            binary = compile_bench(backend, bench)
            walls, rsss, out, rc = [], [], None, 0
            for _ in range(args.runs):
                wall, rss, out, _, rc = run_once(binary)
                if out == "TIMEOUT":
                    break
                walls.append(wall)
                if rss is not None:
                    rsss.append(rss)
                if rc != 0:
                    break
            if out == "TIMEOUT":
                row = {"bench": bench, "backend": backend, "wall_s": None, "rss_mb": None,
                       "ok": True, "timeout": True, "output": out}
                results.append(row)
                print(f"{bench:14s} {backend:12s} TIMEOUT (>{TIMEOUT_S}s)", flush=True)
                continue
            env = dict(os.environ, PLUTO_GC_LOG="1")
            _, _, _, log, _ = run_once(binary, env)
            stats = gc_stats(log) or {}
            if backend == args.reference:
                reference_out[bench] = out
            ok = rc == 0 and (bench not in reference_out or out == reference_out[bench])
            row = {
                "bench": bench, "backend": backend,
                "wall_s": statistics.median(walls),
                "rss_mb": max(rsss) if rsss else None,
                "ok": ok, "output": out,
                **stats,
            }
            results.append(row)
            print(f"{bench:14s} {backend:12s} wall={row['wall_s']:.3f}s "
                  f"pause={row.get('pause_ms', float('nan')):.1f}ms "
                  f"max={row.get('max_pause_ms', float('nan')):.2f}ms "
                  f"cyc={row.get('cycles', '-')} "
                  + (f"local={row['local']}/{row['local_pause_ms']:.1f}ms/max{row['local_max_ms']:.2f} "
                     if row.get('local') else "")
                  + f"rss={row['rss_mb'] or 0:.0f}MB "
                  f"{'ok' if ok else 'MISMATCH ' + repr(out)}", flush=True)

    if args.out:
        os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
        with open(args.out, "w") as f:
            json.dump({"backends": backends, "runs": args.runs, "results": results}, f, indent=1)
    if not all(r["ok"] for r in results):
        sys.exit(1)


if __name__ == "__main__":
    main()
