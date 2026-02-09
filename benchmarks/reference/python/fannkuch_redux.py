import time

def fannkuch(n):
    perm1 = list(range(n))
    count = [i + 1 for i in range(n)]
    max_flips = 0
    checksum = 0
    perm_count = 0

    while True:
        if perm1[0] != 0:
            perm = perm1[:]
            flips = 0
            while perm[0] != 0:
                k = perm[0]
                perm[:k+1] = perm[:k+1][::-1]
                flips += 1
            if flips > max_flips:
                max_flips = flips
            if perm_count % 2 == 0:
                checksum += flips
            else:
                checksum -= flips
        perm_count += 1

        r = 1
        while r < n:
            perm0 = perm1[0]
            for i in range(r):
                perm1[i] = perm1[i + 1]
            perm1[r] = perm0
            count[r] -= 1
            if count[r] > 0:
                break
            count[r] = r + 1
            r += 1
        if r >= n:
            break

    print(f"checksum: {checksum}")
    return max_flips

if __name__ == "__main__":
    n = 10
    t0 = time.monotonic_ns()
    result = fannkuch(n)
    ms = (time.monotonic_ns() - t0) // 1_000_000
    print(f"max flips: {result}")
    print(f"elapsed: {ms} ms")
