package solution

import "errors"

func MostCommonLetter(s string) (string, error) {
	var counts [26]int
	any := false
	for i := 0; i < len(s); i++ {
		c := s[i]
		if c >= 'A' && c <= 'Z' {
			c += 'a' - 'A'
		}
		if c >= 'a' && c <= 'z' {
			counts[c-'a']++
			any = true
		}
	}
	if !any {
		return "", errors.New("no letters")
	}
	best := 0
	for i := 1; i < 26; i++ {
		if counts[i] > counts[best] {
			best = i
		}
	}
	return string(rune('a' + best)), nil
}
