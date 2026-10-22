package calc

type Point struct {
	X, Y int64
}

func abs(v int64) int64 {
	if v < 0 {
		return -v
	}
	return v
}

func manhattan(a, b Point) int64 {
	return abs(a.X-b.X) + abs(a.Y-b.Y)
}
