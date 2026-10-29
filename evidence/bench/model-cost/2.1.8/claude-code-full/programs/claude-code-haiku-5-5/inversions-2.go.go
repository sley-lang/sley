package solution

// Inversions returns the number of index pairs i < j with xs[i] > xs[j].
// It runs a merge sort on a copy of xs, counting cross-pairs during merges.
// The input slice is not modified.
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

// mergeCount sorts a in place (using buf as scratch) and returns the
// number of inversions within a.
func mergeCount(a, buf []int64) int64 {
	n := len(a)
	if n < 2 {
		return 0
	}
	mid := n / 2
	var count int64
	count += mergeCount(a[:mid], buf[:mid])
	count += mergeCount(a[mid:], buf[mid:])

	// Merge the two sorted halves into buf, counting pairs where a
	// right-half element is strictly smaller than a left-half element.
	i, j, k := 0, mid, 0
	for i < mid && j < n {
		if a[i] <= a[j] {
			buf[k] = a[i]
			i++
		} else {
			buf[k] = a[j]
			count += int64(mid - i)
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
	return count
}
