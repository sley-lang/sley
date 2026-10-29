pub fn parse_int(chars: &[i64]) -> Result<i64, String> {
    let (sign, digits) = match chars {
        [] => return Err("empty".into()),
        [45] => return Err("lone minus".into()),
        [45, rest @ ..] => (-1, rest),
        _ => (1, chars),
    };
    let mut value = 0i64;
    for &c in digits {
        if !(48..=57).contains(&c) {
            return Err("not a digit".into());
        }
        value = value * 10 + (c - 48);
    }
    Ok(sign * value)
}
