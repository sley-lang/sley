package solution

import (
	"errors"
	"strings"
)

func RunLengthDecode(s string) (string, error) {
	var b strings.Builder
	total := 0
	i := 0
	for i < len(s) {
		c := s[i]
		if c < 32 || c > 126 || (c >= '0' && c <= '9') {
			return "", errors.New("invalid group character")
		}
		i++
		if i >= len(s) || s[i] < '0' || s[i] > '9' {
			return "", errors.New("missing count")
		}
		if s[i] == '0' {
			return "", errors.New("count has leading zero")
		}
		count := 0
		for i < len(s) && s[i] >= '0' && s[i] <= '9' {
			count = count*10 + int(s[i]-'0')
			if count > 1000 {
				return "", errors.New("decoded string too long")
			}
			i++
		}
		total += count
		if total > 1000 {
			return "", errors.New("decoded string too long")
		}
		b.WriteString(strings.Repeat(string(c), count))
	}
	return b.String(), nil
}
