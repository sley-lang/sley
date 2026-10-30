package solution

func Fib(n int64) int64 {
	if n <= 0 {
		return 0
	}
	var a, b int64 = 0, 1
	for i := int64(0); i < n; i++ {
		a, b = b, a+b
	}
	return a
}
