package solution

func ValidateUsername(s string) int64 {
	if len(s) < 3 || len(s) > 16 {
		return 0
	}
	if s[0] < 'a' || s[0] > 'z' {
		return 0
	}
	for i := 0; i < len(s); i++ {
		c := s[i]
		ok := (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '_'
		if !ok {
			return 0
		}
		if c == '_' && i > 0 && s[i-1] == '_' {
			return 0
		}
	}
	if s[len(s)-1] == '_' {
		return 0
	}
	return 1
}
