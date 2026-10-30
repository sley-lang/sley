package solution

func MaskDigits(s string) string {
	total := 0
	for i := 0; i < len(s); i++ {
		if s[i] >= '0' && s[i] <= '9' {
			total++
		}
	}
	if total <= 4 {
		return s
	}
	out := []byte(s)
	seen := 0
	for i, c := range out {
		if c >= '0' && c <= '9' {
			if seen < total-4 {
				out[i] = '*'
			}
			seen++
		}
	}
	return string(out)
}
