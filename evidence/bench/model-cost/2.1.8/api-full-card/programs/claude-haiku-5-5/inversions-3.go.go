package solution

func Inversions(xs []int64) int64 {
	n := len(xs)
	if n < 2 {
		return 0
	}
	a := make([]int64, n)
	copy(a, xs)
	buf := make([]int64, n)
	return mergeCount(a, buf)
}

func mergeCount(a, buf []int64) int64 {
	n := len(a)
	if n < 2 {
		return 0
	}
	mid := n / 2
	inv := mergeCount(a[:mid], buf[:mid]) + mergeCount(a[mid:], buf[mid:])
	i, j, k := 0, mid, 0
	for i < mid && j < n {
		if a[i] <= a[j] {
			buf[k] = a[i]
			i++
		} else {
			buf[k] = a[j]
			inv += int64(mid - i)
			j++
		}
		k++
	}
	for i < mid {
		buf[k] = a[i]
		i++
		k++
	}
	for j < n {
		buf[k] = a[j]
		j++
		k++
	}
	copy(a, buf[:n])
	return inv
}
