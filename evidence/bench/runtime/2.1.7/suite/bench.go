package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strconv"
)

func fib(n int64) int64 { if n < 2 { return n }; return fib(n-1) + fib(n-2) }
func loopsum(n int64) int64 { var s int64; for i := int64(0); i < n; i++ { s = (s + i*i) % 1000000007 }; return s }
func collatz(limit int64) int64 {
	var best, bestN int64
	for n := int64(1); n < limit; n++ {
		x, steps := n, int64(0)
		for x != 1 { if x%2 == 0 { x /= 2 } else { x = 3*x + 1 }; steps++ }
		if steps > best { best, bestN = steps, n }
	}
	return bestN
}
func primes(n int64) int64 {
	var count int64
	for k := int64(2); k < n; k++ {
		isP := true
		for d := int64(2); d*d <= k; d++ { if k%d == 0 { isP = false; break } }
		if isP { count++ }
	}
	return count
}
func gcd(a, b int64) int64 { for b != 0 { a, b = b, a%b }; return a }
func gcdsum(n int64) int64 { var s int64; for i := int64(1); i < n; i++ { for j := int64(1); j < n; j++ { s += gcd(i, j) } }; return s }
func tak(x, y, z int64) int64 { if y < x { return tak(tak(x-1, y, z), tak(y-1, z, x), tak(z-1, x, y)) }; return z }
func triples(n int64) int64 {
	var count int64
	for c := int64(1); c <= n; c++ { for b := int64(1); b < c; b++ { for a := int64(1); a < b; a++ { if a*a+b*b == c*c { count++ } } } }
	return count
}
func digitsum(n int64) int64 { var t int64; for i := int64(0); i < n; i++ { for x := i; x > 0; x /= 10 { t += x % 10 } }; return t }
func isqrt(n int64) int64 {
	var total int64
	for i := int64(0); i < n; i++ {
		lo, hi := int64(0), i+1
		for hi-lo > 1 { mid := (lo + hi) / 2; if mid*mid <= i { lo = mid } else { hi = mid } }
		total += lo
	}
	return total
}
func inversions(xs []int64) int64 {
	var count int64
	for i := range xs { x := xs[i]; for j := i + 1; j < len(xs); j++ { if xs[j] < x { count++ } } }
	return count
}
func main() {
	num := func(k int) int64 { v, _ := strconv.ParseInt(os.Args[k], 10, 64); return v }
	var r int64
	switch os.Args[1] {
	case "inversions":
		b, _ := os.ReadFile(os.Args[2]); var xs []int64; json.Unmarshal(b, &xs); r = inversions(xs)
	case "tak": r = tak(num(2), num(3), num(4))
	case "fib": r = fib(num(2))
	case "loopsum": r = loopsum(num(2))
	case "collatz": r = collatz(num(2))
	case "primes": r = primes(num(2))
	case "gcdsum": r = gcdsum(num(2))
	case "triples": r = triples(num(2))
	case "digitsum": r = digitsum(num(2))
	case "isqrt": r = isqrt(num(2))
	default: os.Exit(2)
	}
	fmt.Println(r)
}
