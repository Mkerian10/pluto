package main

import (
	"fmt"
	"time"
)

func main() {
	n := 2000
	maxIter := 50

	start := time.Now()

	totalIters := int64(0)
	for y := 0; y < n; y++ {
		ci := 2.0*float64(y)/float64(n) - 1.0
		for x := 0; x < n; x++ {
			cr := 2.0*float64(x)/float64(n) - 1.5
			zr, zi := 0.0, 0.0
			iter := 0
			for iter < maxIter {
				zr2, zi2 := zr*zr, zi*zi
				if zr2+zi2 > 4.0 {
					break
				}
				zi = 2.0*zr*zi + ci
				zr = zr2 - zi2 + cr
				iter++
			}
			totalIters += int64(iter)
		}
	}

	ms := time.Since(start).Milliseconds()
	fmt.Printf("total iterations: %d\n", totalIters)
	fmt.Printf("elapsed: %d ms\n", ms)
}
