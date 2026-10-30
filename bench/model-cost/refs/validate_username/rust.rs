pub fn validate_username(s: &str) -> i64 {
    let b = s.as_bytes();
    if b.len() < 3 || b.len() > 16 {
        return 0;
    }
    if !b[0].is_ascii_lowercase() {
        return 0;
    }
    if b[b.len() - 1] == b'_' {
        return 0;
    }
    for (i, &c) in b.iter().enumerate() {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_') {
            return 0;
        }
        if c == b'_' && i > 0 && b[i - 1] == b'_' {
            return 0;
        }
    }
    1
}
