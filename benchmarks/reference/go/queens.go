package main

import (
	"fmt"
	"time"
)

func placeQueen(row, n int64, cols, diag1, diag2 []bool) int64 {
	if row == n {
		return 1
	}
	var count int64
	var col int64
	for col = 0; col < n; col++ {
		d1 := row + col
		d2 := row - col + n - 1
		if !cols[col] && !diag1[d1] && !diag2[d2] {
			cols[col] = true
			diag1[d1] = true
			diag2[d2] = true
			count += placeQueen(row+1, n, cols, diag1, diag2)
			cols[col] = false
			diag1[d1] = false
			diag2[d2] = false
		}
	}
	return count
}

func queensSolve(n int64) int64 {
	cols := make([]bool, n)
	diag1 := make([]bool, 2*n)
	diag2 := make([]bool, 2*n)
	return placeQueen(0, n, cols, diag1, diag2)
}

func main() {
	var n int64 = 12
	start := time.Now()
	solutions := queensSolve(n)
	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("%d-queens solutions: %d\n", n, solutions)
	fmt.Printf("elapsed: %d ms\n", elapsed)
}
