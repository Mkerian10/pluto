package main

import (
	"fmt"
	"time"
)

func towers(discs, from, to, spare int64, pegs []int64, moves int64) int64 {
	if discs == 0 {
		return moves
	}
	moves = towers(discs-1, from, spare, to, pegs, moves)
	pegs[to]++
	pegs[from]--
	moves++
	moves = towers(discs-1, spare, to, from, pegs, moves)
	return moves
}

func main() {
	var n int64 = 20
	var iters int64 = 100
	start := time.Now()

	var totalMoves int64
	var i int64
	for i = 0; i < iters; i++ {
		pegs := []int64{n, 0, 0}
		m := towers(n, 0, 1, 2, pegs, 0)
		totalMoves += m
	}

	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("total moves: %d\n", totalMoves)
	fmt.Printf("elapsed: %d ms\n", elapsed)
}
