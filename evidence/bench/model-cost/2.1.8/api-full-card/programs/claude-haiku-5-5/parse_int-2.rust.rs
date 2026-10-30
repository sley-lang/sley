pub fn parse_int(chars: &[i64]) -> Result<i64, String> {
    let (neg, digits) = match chars.first() {
        None => return Err("empty input".to_string()),
        Some(&45) => (true, &chars[1..]),
        Some(_) => (false, chars),
    };

    if digits.is_empty() {
        return Err("no digits".to_string());
    }

    // Accumulate in the negative direction for negative numbers so that
    // i64::MIN can be represented without overflow.
    let mut acc: i64 = 0;
    for &c in digits {
        if !(48..=57).contains(&c) {
            return Err(format!("invalid character code {}", c));
        }
        let d = c - 48;
        acc = acc
            .checked_mul(10)
            .and_then(|v| {
                if neg {
                    v.checked_sub(d)
                } else {
                    v.checked_add(d)
                }
            })
            .ok_or_else(|| "integer overflow".to_string())?;
    }

    Ok(acc)
}
