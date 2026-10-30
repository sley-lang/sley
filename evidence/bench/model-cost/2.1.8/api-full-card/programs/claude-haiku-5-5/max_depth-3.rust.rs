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
                return Err("unbalanced: close without matching open".to_string());
            }
            depth -= 1;
        }
    }
    if depth != 0 {
        return Err("unbalanced: open never closed".to_string());
    }
    Ok(max)
}
