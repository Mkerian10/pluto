package main

import (
	"fmt"
	"time"
)

func bounceSim(steps int64) int64 {
	x := 0.0
	y := 0.0
	vx := 1.5
	vy := 2.3
	boxSize := 100.0
	var bounces int64

	var i int64
	for i = 0; i < steps; i++ {
		x += vx
		y += vy

		if x < 0.0 {
			x = -x
			vx = -vx
			bounces++
		}
		if x > boxSize {
			x = boxSize - (x - boxSize)
			vx = -vx
			bounces++
		}
		if y < 0.0 {
			y = -y
			vy = -vy
			bounces++
		}
		if y > boxSize {
			y = boxSize - (y - boxSize)
			vy = -vy
			bounces++
		}
	}
	return bounces
}

func main() {
	var steps int64 = 10000000
	start := time.Now()
	bounces := bounceSim(steps)
	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("bounces: %d\n", bounces)
	fmt.Printf("elapsed: %d ms\n", elapsed)
}
