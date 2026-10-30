package solution

import "sort"

func DenseRank(xs []int64) []int64 {
	seen := map[int64]bool{}
	distinct := []int64{}
	for _, x := range xs {
		if !seen[x] {
			seen[x] = true
			distinct = append(distinct, x)
		}
	}
	sort.Slice(distinct, func(i, j int) bool { return distinct[i] > distinct[j] })
	rank := map[int64]int64{}
	for i, v := range distinct {
		rank[v] = int64(i + 1)
	}
	out := make([]int64, len(xs))
	for i, x := range xs {
		out[i] = rank[x]
	}
	return out
}
