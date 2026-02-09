import time

def place_queen(row, n, cols, diag1, diag2):
    if row == n:
        return 1
    count = 0
    for col in range(n):
        d1 = row + col
        d2 = row - col + n - 1
        if not cols[col] and not diag1[d1] and not diag2[d2]:
            cols[col] = True
            diag1[d1] = True
            diag2[d2] = True
            count += place_queen(row + 1, n, cols, diag1, diag2)
            cols[col] = False
            diag1[d1] = False
            diag2[d2] = False
    return count

def queens_solve(n):
    cols = [False] * n
    diag1 = [False] * (2 * n)
    diag2 = [False] * (2 * n)
    return place_queen(0, n, cols, diag1, diag2)

n = 12
start = time.monotonic_ns()
solutions = queens_solve(n)
elapsed = time.monotonic_ns() - start
ms = elapsed // 1000000
print(f"{n}-queens solutions: {solutions}")
print(f"elapsed: {ms} ms")
