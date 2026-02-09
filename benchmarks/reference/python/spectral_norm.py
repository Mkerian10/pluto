import math
import time

def eval_a(i, j):
    ij = i + j
    return 1.0 / (ij * (ij + 1) // 2 + i + 1)

def eval_a_times_u(u, n):
    au = [0.0] * n
    for i in range(n):
        s = 0.0
        for j in range(n):
            s += eval_a(i, j) * u[j]
        au[i] = s
    return au

def eval_at_times_u(u, n):
    atu = [0.0] * n
    for i in range(n):
        s = 0.0
        for j in range(n):
            s += eval_a(j, i) * u[j]
        atu[i] = s
    return atu

def eval_ata_times_u(u, n):
    return eval_at_times_u(eval_a_times_u(u, n), n)

if __name__ == "__main__":
    n = 500
    u = [1.0] * n

    t0 = time.monotonic_ns()

    for _ in range(10):
        v = eval_ata_times_u(u, n)
        u = eval_ata_times_u(v, n)

    vbv = sum(u[i] * v[i] for i in range(n))
    vv = sum(v[i] * v[i] for i in range(n))

    ms = (time.monotonic_ns() - t0) // 1_000_000
    print(f"spectral norm: {math.sqrt(vbv / vv):.6f}")
    print(f"elapsed: {ms} ms")
