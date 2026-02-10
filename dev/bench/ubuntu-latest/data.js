window.BENCHMARK_DATA = {
  "lastUpdate": 1770699270746,
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
      }
    ]
  }
}