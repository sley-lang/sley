package solution

import (
	"errors"
	"strings"
)

func ToRoman(n int64) (string, error) {
	if n < 1 || n > 3999 {
		return "", errors.New("n must be in [1, 3999]")
	}
	values := []int64{1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1}
	symbols := []string{"M", "CM", "D", "CD", "C", "XC", "L", "XL", "X", "IX", "V", "IV", "I"}
	var b strings.Builder
	for i, v := range values {
		for n >= v {
			b.WriteString(symbols[i])
			n -= v
		}
	}
	return b.String(), nil
}
