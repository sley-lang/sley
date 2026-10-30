package solution

func CaesarShift(s string, k int64) string {
	shift := byte(((k % 26) + 26) % 26)
	out := []byte(s)
	for i, c := range out {
		if c >= 'a' && c <= 'z' {
			out[i] = 'a' + (c-'a'+shift)%26
		} else if c >= 'A' && c <= 'Z' {
			out[i] = 'A' + (c-'A'+shift)%26
		}
	}
	return string(out)
}
