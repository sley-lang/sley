package solution

// Inversions returns the number of index pairs i < j with xs[i] > xs[j].
// It runs in O(n log n) using a merge sort on a copy of the input, so xs is not modified.
func Inversions(xs []int64) int64 {
	n := len(xs)
	a := make([]int64, n)
	copy(a, xs)
	buf := make([]int64, n)
	return mergeCount(a, buf)
}

// mergeCount sorts a in place (using buf as scratch space) and returns
// the number of inversions within a.
func mergeCount(a, buf []int64) int64 {
	n := len(a)
	if n < 2 {
		return 0
	}
	mid := n / 2
	count := mergeCount(a[:mid], buf[:mid]) + mergeCount(a[mid:], buf[mid:])

	i, j, k := 0, mid, 0
	for i < mid && j < n {
		if a[i] <= a[j] {
			buf[k] = a[i]
			i++
		} else {
			// a[j] is smaller than every remaining left element.
			buf[k] = a[j]
			j++
			count += int64(mid - i)
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
	return count
}
