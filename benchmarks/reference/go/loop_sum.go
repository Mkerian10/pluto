package main

import (
	"fmt"
	"time"
)

func main() {
	start := time.Now()
	var sum int64
	var i int64
	for i = 0; i < 100000000; i++ {
		sum += i
	}
	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("sum = %d\n", sum)
	fmt.Printf("elapsed: %d ms\n", elapsed)
}
