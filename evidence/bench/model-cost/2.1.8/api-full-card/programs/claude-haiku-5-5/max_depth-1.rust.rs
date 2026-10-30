pub fn max_depth(codes: &[i64]) -> Result<i64, String> {
    let mut depth: i64 = 0;
    let mut max: i64 = 0;
    for &c in codes {
        if c == 40 {
            depth += 1;
            if depth > max {
                max = depth;
            }
        } else if c == 41 {
            if depth == 0 {
                return Err(String::from("unbalanced parentheses: close without matching open"));
            }
            depth -= 1;
        }
    }
    if depth != 0 {
        return Err(String::from("unbalanced parentheses: open never closed"));
    }
    Ok(max)
}
