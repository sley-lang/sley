package solution

import (
	"errors"
	"sort"
)

func CoveredLength(xs []int64) (int64, error) {
	if len(xs)%2 != 0 {
		return 0, errors.New("odd number of elements")
	}
	type interval struct{ start, end int64 }
	ivs := []interval{}
	for i := 0; i < len(xs); i += 2 {
		if xs[i] > xs[i+1] {
			return 0, errors.New("start greater than end")
		}
		ivs = append(ivs, interval{xs[i], xs[i+1]})
	}
	sort.Slice(ivs, func(i, j int) bool { return ivs[i].start < ivs[j].start })
	var total int64
	started := false
	var curStart, curEnd int64
	for _, iv := range ivs {
		if !started {
			curStart, curEnd = iv.start, iv.end
			started = true
			continue
		}
		if iv.start <= curEnd {
			if iv.end > curEnd {
				curEnd = iv.end
			}
		} else {
			total += curEnd - curStart
			curStart, curEnd = iv.start, iv.end
		}
	}
	if started {
		total += curEnd - curStart
	}
	return total, nil
}
