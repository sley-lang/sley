package solution

import "strings"

func Slugify(s string) string {
	var b strings.Builder
	inSep := false
	for i := 0; i < len(s); i++ {
		c := s[i]
		if c >= 'A' && c <= 'Z' {
			c += 'a' - 'A'
		}
		if (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') {
			b.WriteByte(c)
			inSep = false
		} else if !inSep {
			b.WriteByte('-')
			inSep = true
		}
	}
	return strings.Trim(b.String(), "-")
}
