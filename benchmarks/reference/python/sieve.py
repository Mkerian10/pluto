import time

def sieve(n):
    flags = [True] * n
    count = 0
    p = 2
    while p < n:
        if flags[p]:
            count += 1
            m = p + p
            while m < n:
                flags[m] = False
                m += p
        p += 1
    return count

n = 500000
start = time.monotonic_ns()
result = sieve(n)
elapsed = time.monotonic_ns() - start
ms = elapsed // 1000000
print(f"primes below {n}: {result}")
print(f"elapsed: {ms} ms")
