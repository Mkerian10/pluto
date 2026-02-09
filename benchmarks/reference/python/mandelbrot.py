import time

if __name__ == "__main__":
    n = 2000
    max_iter = 50

    t0 = time.monotonic_ns()

    total_iters = 0
    for y in range(n):
        ci = 2.0 * y / n - 1.0
        for x in range(n):
            cr = 2.0 * x / n - 1.5
            zr, zi = 0.0, 0.0
            it = 0
            while it < max_iter:
                zr2, zi2 = zr * zr, zi * zi
                if zr2 + zi2 > 4.0:
                    break
                zi = 2.0 * zr * zi + ci
                zr = zr2 - zi2 + cr
                it += 1
            total_iters += it

    ms = (time.monotonic_ns() - t0) // 1_000_000
    print(f"total iterations: {total_iters}")
    print(f"elapsed: {ms} ms")
