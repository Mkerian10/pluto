package main

import (
	"fmt"
	"time"
)

func fib(n int64) int64 {
	if n <= 1 {
		return n
	}
	return fib(n-1) + fib(n-2)
}

func main() {
	start := time.Now()
	result := fib(35)
	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("fib(35) = %d\n", result)
	fmt.Printf("elapsed: %d ms\n", elapsed)
}
