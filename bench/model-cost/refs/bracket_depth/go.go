package solution

import "errors"

func BracketDepth(s string) (int64, error) {
	match := map[byte]byte{')': '(', ']': '[', '}': '{'}
	stack := []byte{}
	var depth int64
	for i := 0; i < len(s); i++ {
		c := s[i]
		switch c {
		case '(', '[', '{':
			stack = append(stack, c)
			if int64(len(stack)) > depth {
				depth = int64(len(stack))
			}
		case ')', ']', '}':
			if len(stack) == 0 || stack[len(stack)-1] != match[c] {
				return 0, errors.New("mismatched bracket")
			}
			stack = stack[:len(stack)-1]
		}
	}
	if len(stack) != 0 {
		return 0, errors.New("unclosed bracket")
	}
	return depth, nil
}
