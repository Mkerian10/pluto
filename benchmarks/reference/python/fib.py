import time

def fib(n):
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)

start = time.monotonic_ns()
result = fib(35)
elapsed = time.monotonic_ns() - start
ms = elapsed // 1000000
print(f"fib(35) = {result}")
print(f"elapsed: {ms} ms")
