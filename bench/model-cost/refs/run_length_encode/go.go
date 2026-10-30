package solution

import (
	"strconv"
	"strings"
)

func RunLengthEncode(s string) string {
	var b strings.Builder
	for i := 0; i < len(s); {
		j := i
		for j < len(s) && s[j] == s[i] {
			j++
		}
		b.WriteByte(s[i])
		b.WriteString(strconv.Itoa(j - i))
		i = j
	}
	return b.String()
}
