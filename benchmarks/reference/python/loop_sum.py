import time

start = time.monotonic_ns()
s = 0
i = 0
while i < 100000000:
    s += i
    i += 1
elapsed = time.monotonic_ns() - start
ms = elapsed // 1000000
print(f"sum = {s}")
print(f"elapsed: {ms} ms")
