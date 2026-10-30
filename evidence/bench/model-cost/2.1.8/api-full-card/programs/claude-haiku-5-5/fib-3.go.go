package solution

func Fib(n int64) int64 {
	if n < 2 {
		return n
	}
	var a, b int64 = 0, 1
	for i := int64(2); i <= n; i++ {
		a, b = b, a+b
	}
	return b
}
