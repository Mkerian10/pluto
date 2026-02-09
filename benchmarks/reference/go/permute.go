package main

import (
	"fmt"
	"time"
)

func swap(arr []int64, i, j int64) {
	arr[i], arr[j] = arr[j], arr[i]
}

func permute(arr []int64, n, count int64) int64 {
	if n == 1 {
		return count + 1
	}
	var i int64
	for i = 0; i < n; i++ {
		count = permute(arr, n-1, count)
		if n%2 == 0 {
			swap(arr, i, n-1)
		} else {
			swap(arr, 0, n-1)
		}
	}
	return count
}

func main() {
	var size int64 = 10
	start := time.Now()

	arr := make([]int64, size)
	var i int64
	for i = 0; i < size; i++ {
		arr[i] = i
	}
	count := permute(arr, size, 0)

	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("permutations of %d: %d\n", size, count)
	fmt.Printf("elapsed: %d ms\n", elapsed)
}
