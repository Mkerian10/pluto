package main

import (
	"fmt"
	"math"
	"time"
)

func evalA(i, j int) float64 {
	ij := i + j
	return 1.0 / float64(ij*(ij+1)/2+i+1)
}

func evalATimesU(u, au []float64, n int) {
	for i := 0; i < n; i++ {
		sum := 0.0
		for j := 0; j < n; j++ {
			sum += evalA(i, j) * u[j]
		}
		au[i] = sum
	}
}

func evalAtTimesU(u, atu []float64, n int) {
	for i := 0; i < n; i++ {
		sum := 0.0
		for j := 0; j < n; j++ {
			sum += evalA(j, i) * u[j]
		}
		atu[i] = sum
	}
}

func evalAtaTimesU(u, atau, tmp []float64, n int) {
	evalATimesU(u, tmp, n)
	evalAtTimesU(tmp, atau, n)
}

func main() {
	n := 500
	u := make([]float64, n)
	v := make([]float64, n)
	tmp := make([]float64, n)

	for i := range u {
		u[i] = 1.0
	}

	start := time.Now()

	for i := 0; i < 10; i++ {
		evalAtaTimesU(u, v, tmp, n)
		evalAtaTimesU(v, u, tmp, n)
	}

	vbv, vv := 0.0, 0.0
	for i := 0; i < n; i++ {
		vbv += u[i] * v[i]
		vv += v[i] * v[i]
	}

	ms := time.Since(start).Milliseconds()
	fmt.Printf("spectral norm: %f\n", math.Sqrt(vbv/vv))
	fmt.Printf("elapsed: %d ms\n", ms)
}
