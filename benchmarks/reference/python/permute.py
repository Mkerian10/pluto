import time
import sys

sys.setrecursionlimit(100000)

def swap(arr, i, j):
    arr[i], arr[j] = arr[j], arr[i]

def permute(arr, n, count):
    if n == 1:
        return count + 1
    for i in range(n):
        count = permute(arr, n - 1, count)
        if n % 2 == 0:
            swap(arr, i, n - 1)
        else:
            swap(arr, 0, n - 1)
    return count

size = 10
start = time.monotonic_ns()
arr = list(range(size))
count = permute(arr, size, 0)
elapsed = time.monotonic_ns() - start
ms = elapsed // 1000000
print(f"permutations of {size}: {count}")
print(f"elapsed: {ms} ms")
