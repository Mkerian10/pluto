package main

import (
	"fmt"
	"time"
)

func sieve(n int64) int64 {
	flags := make([]bool, n)
	for i := range flags {
		flags[i] = true
	}

	var count int64
	var p int64
	for p = 2; p < n; p++ {
		if flags[p] {
			count++
			for m := p + p; m < n; m += p {
				flags[m] = false
			}
		}
	}
	return count
}

func main() {
	var n int64 = 500000
	start := time.Now()
	result := sieve(n)
	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("primes below %d: %d\n", n, result)
	fmt.Printf("elapsed: %d ms\n", elapsed)
}
