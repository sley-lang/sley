pub fn parse_int(chars: &[i64]) -> Result<i64, String> {
    if chars.is_empty() {
        return Err("empty input".to_string());
    }

    let (negative, digits) = match chars[0] {
        45 => (true, &chars[1..]),
        _ => (false, chars),
    };

    if digits.is_empty() {
        return Err("no digits".to_string());
    }

    let mut value: i64 = 0;
    for &c in digits {
        if !(48..=57).contains(&c) {
            return Err(format!("invalid character code: {}", c));
        }
        let digit = c - 48;
        value = value
            .checked_mul(10)
            .and_then(|v| {
                if negative {
                    v.checked_sub(digit)
                } else {
                    v.checked_add(digit)
                }
            })
            .ok_or_else(|| "integer overflow".to_string())?;
    }

    Ok(value)
}
