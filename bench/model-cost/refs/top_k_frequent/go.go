package solution

import "sort"

func TopKFrequent(xs []int64, k int64) []int64 {
	counts := map[int64]int{}
	for _, x := range xs {
		counts[x]++
	}
	vals := make([]int64, 0, len(counts))
	for v := range counts {
		vals = append(vals, v)
	}
	sort.Slice(vals, func(i, j int) bool {
		if counts[vals[i]] != counts[vals[j]] {
			return counts[vals[i]] > counts[vals[j]]
		}
		return vals[i] < vals[j]
	})
	if k < 0 {
		k = 0
	}
	if int64(len(vals)) > k {
		vals = vals[:k]
	}
	return vals
}
