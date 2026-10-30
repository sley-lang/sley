package solution

import "strings"

func CountDistinctWords(s string) int64 {
	isLetter := func(c byte) bool {
		return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')
	}
	words := map[string]bool{}
	for i := 0; i < len(s); {
		if !isLetter(s[i]) {
			i++
			continue
		}
		j := i
		for j < len(s) && isLetter(s[j]) {
			j++
		}
		words[strings.ToLower(s[i:j])] = true
		i = j
	}
	return int64(len(words))
}
