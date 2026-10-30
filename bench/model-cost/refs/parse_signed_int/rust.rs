pub fn parse_signed_int(s: &str) -> Result<i64, String> {
    let bytes = s.as_bytes();
    let (negative, digits) = match bytes.first() {
        Some(b'+') => (false, &bytes[1..]),
        Some(b'-') => (true, &bytes[1..]),
        _ => (false, bytes),
    };
    if digits.is_empty() {
        return Err("expected at least one digit".to_string());
    }
    let mut value: i64 = 0;
    for &b in digits {
        if !b.is_ascii_digit() {
            return Err("invalid character".to_string());
        }
        value = value * 10 + (b - b'0') as i64;
        if value > 1_000_000 {
            // Keep accumulating digits for validation, but cap the value to avoid overflow.
            value = 1_000_001;
        }
    }
    if value > 1_000_000 {
        return Err("value out of range".to_string());
    }
    Ok(if negative { -value } else { value })
}
