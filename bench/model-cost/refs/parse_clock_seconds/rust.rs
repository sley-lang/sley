pub fn parse_clock_seconds(s: &str) -> Result<i64, String> {
    let b = s.as_bytes();
    if b.len() != 8 || b[2] != b':' || b[5] != b':' {
        return Err("expected HH:MM:SS".to_string());
    }
    let field = |i: usize| -> Result<i64, String> {
        if b[i].is_ascii_digit() && b[i + 1].is_ascii_digit() {
            Ok(((b[i] - b'0') * 10 + (b[i + 1] - b'0')) as i64)
        } else {
            Err("expected two digits".to_string())
        }
    };
    let h = field(0)?;
    let m = field(3)?;
    let sec = field(6)?;
    if h > 23 || m > 59 || sec > 59 {
        return Err("time component out of range".to_string());
    }
    Ok(h * 3600 + m * 60 + sec)
}
