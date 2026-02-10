window.BENCHMARK_DATA = {
  "lastUpdate": 1770699300318,
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
        "date": 1770684590501,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "fib",
            "value": 44,
            "unit": "ms"
          },
          {
            "name": "loop_sum",
            "value": 64,
            "unit": "ms"
          },
          {
            "name": "string_concat",
            "value": 294,
            "unit": "ms"
          },
          {
            "name": "array_push",
            "value": 4,
            "unit": "ms"
          },
          {
            "name": "array_iter",
            "value": 1,
            "unit": "ms"
          },
          {
            "name": "class_method",
            "value": 23,
            "unit": "ms"
          },
          {
            "name": "closure_call",
            "value": 13,
            "unit": "ms"
          },
          {
            "name": "trait_dispatch",
            "value": 686,
            "unit": "ms"
          },
          {
            "name": "gc_churn",
            "value": 67,
            "unit": "ms"
          },
          {
            "name": "gc_binary_trees",
            "value": 1175,
            "unit": "ms"
          },
          {
            "name": "gc_string_pressure",
            "value": 56,
            "unit": "ms"
          },
          {
            "name": "sieve",
            "value": 6,
            "unit": "ms"
          },
          {
            "name": "bounce",
            "value": 40,
            "unit": "ms"
          },
          {
            "name": "towers",
            "value": 1269,
            "unit": "ms"
          },
          {
            "name": "permute",
            "value": 67,
            "unit": "ms"
          },
          {
            "name": "queens",
            "value": 115,
            "unit": "ms"
          },
          {
            "name": "fannkuch_redux",
            "value": 1000,
            "unit": "ms"
          },
          {
            "name": "spectral_norm",
            "value": 34,
            "unit": "ms"
          },
          {
            "name": "nbody",
            "value": 8914,
            "unit": "ms"
          },
          {
            "name": "mandelbrot",
            "value": 464,
            "unit": "ms"
          },
          {
            "name": "monte_carlo",
            "value": 1607,
            "unit": "ms"
          },
          {
            "name": "storage",
            "value": 461,
            "unit": "ms"
          },
          {
            "name": "list",
            "value": 61,
            "unit": "ms"
          },
          {
            "name": "fft",
            "value": 1898,
            "unit": "ms"
          },
          {
            "name": "sor",
            "value": 604,
            "unit": "ms"
          },
          {
            "name": "sparse_matmul",
            "value": 458,
            "unit": "ms"
          },
          {
            "name": "lu",
            "value": 3316,
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
        "date": 1770699299161,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "fib",
            "value": 56,
            "unit": "ms"
          },
          {
            "name": "loop_sum",
            "value": 68,
            "unit": "ms"
          },
          {
            "name": "string_concat",
            "value": 371,
            "unit": "ms"
          },
          {
            "name": "array_push",
            "value": 4,
            "unit": "ms"
          },
          {
            "name": "array_iter",
            "value": 2,
            "unit": "ms"
          },
          {
            "name": "class_method",
            "value": 23,
            "unit": "ms"
          },
          {
            "name": "closure_call",
            "value": 13,
            "unit": "ms"
          },
          {
            "name": "trait_dispatch",
            "value": 1154,
            "unit": "ms"
          },
          {
            "name": "gc_churn",
            "value": 86,
            "unit": "ms"
          },
          {
            "name": "gc_binary_trees",
            "value": 1740,
            "unit": "ms"
          },
          {
            "name": "gc_string_pressure",
            "value": 120,
            "unit": "ms"
          },
          {
            "name": "sieve",
            "value": 10,
            "unit": "ms"
          },
          {
            "name": "bounce",
            "value": 52,
            "unit": "ms"
          },
          {
            "name": "towers",
            "value": 1619,
            "unit": "ms"
          },
          {
            "name": "permute",
            "value": 88,
            "unit": "ms"
          },
          {
            "name": "queens",
            "value": 133,
            "unit": "ms"
          },
          {
            "name": "fannkuch_redux",
            "value": 1219,
            "unit": "ms"
          },
          {
            "name": "spectral_norm",
            "value": 45,
            "unit": "ms"
          },
          {
            "name": "nbody",
            "value": 7488,
            "unit": "ms"
          },
          {
            "name": "mandelbrot",
            "value": 278,
            "unit": "ms"
          },
          {
            "name": "monte_carlo",
            "value": 1514,
            "unit": "ms"
          },
          {
            "name": "storage",
            "value": 460,
            "unit": "ms"
          },
          {
            "name": "list",
            "value": 70,
            "unit": "ms"
          },
          {
            "name": "fft",
            "value": 1885,
            "unit": "ms"
          },
          {
            "name": "sor",
            "value": 703,
            "unit": "ms"
          },
          {
            "name": "sparse_matmul",
            "value": 530,
            "unit": "ms"
          },
          {
            "name": "lu",
            "value": 3665,
            "unit": "ms"
          }
        ]
      }
    ]
  }
}