import time
import sys

sys.setrecursionlimit(100000)

def towers(discs, frm, to, spare, pegs, moves):
    if discs == 0:
        return moves
    moves = towers(discs - 1, frm, spare, to, pegs, moves)
    pegs[to] += 1
    pegs[frm] -= 1
    moves += 1
    moves = towers(discs - 1, spare, to, frm, pegs, moves)
    return moves

n = 20
iters = 100
start = time.monotonic_ns()

total_moves = 0
for i in range(iters):
    pegs = [n, 0, 0]
    m = towers(n, 0, 1, 2, pegs, 0)
    total_moves += m

elapsed = time.monotonic_ns() - start
ms = elapsed // 1000000
print(f"total moves: {total_moves}")
print(f"elapsed: {ms} ms")
