package main

import (
	"fmt"
	"time"
)

func fannkuch(n int) int {
	perm := make([]int, n)
	perm1 := make([]int, n)
	count := make([]int, n)

	for i := 0; i < n; i++ {
		perm1[i] = i
	}
	for i := 0; i < n; i++ {
		count[i] = i + 1
	}

	maxFlips := 0
	checksum := 0
	permCount := 0

	for {
		if perm1[0] != 0 {
			copy(perm, perm1)
			flips := 0
			for perm[0] != 0 {
				k := perm[0]
				for lo, hi := 0, k; lo < hi; lo, hi = lo+1, hi-1 {
					perm[lo], perm[hi] = perm[hi], perm[lo]
				}
				flips++
			}
			if flips > maxFlips {
				maxFlips = flips
			}
			if permCount%2 == 0 {
				checksum += flips
			} else {
				checksum -= flips
			}
		}
		permCount++

		r := 1
		for ; r < n; r++ {
			perm0 := perm1[0]
			for i := 0; i < r; i++ {
				perm1[i] = perm1[i+1]
			}
			perm1[r] = perm0
			count[r]--
			if count[r] > 0 {
				break
			}
			count[r] = r + 1
		}
		if r >= n {
			break
		}
	}

	fmt.Printf("checksum: %d\n", checksum)
	return maxFlips
}

func main() {
	n := 10
	start := time.Now()
	result := fannkuch(n)
	ms := time.Since(start).Milliseconds()
	fmt.Printf("max flips: %d\n", result)
	fmt.Printf("elapsed: %d ms\n", ms)
}
