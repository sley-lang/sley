package solution

import "errors"

func ChunkSums(xs []int64, size int64) ([]int64, error) {
	if size < 1 {
		return nil, errors.New("size must be at least 1")
	}
	out := []int64{}
	for i := 0; i < len(xs); i += int(size) {
		end := i + int(size)
		if end > len(xs) {
			end = len(xs)
		}
		var sum int64
		for _, x := range xs[i:end] {
			sum += x
		}
		out = append(out, sum)
	}
	return out, nil
}
