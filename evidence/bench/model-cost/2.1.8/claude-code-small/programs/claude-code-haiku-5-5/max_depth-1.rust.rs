pub fn max_depth(codes: &[i64]) -> Result<i64, String> {
    const OPEN: i64 = 40;
    const CLOSE: i64 = 41;

    let mut depth: i64 = 0;
    let mut max: i64 = 0;

    for &code in codes {
        if code == OPEN {
            depth += 1;
            if depth > max {
                max = depth;
            }
        } else if code == CLOSE {
            if depth == 0 {
                return Err("unbalanced parentheses: close without matching open".to_string());
            }
            depth -= 1;
        }
    }

    if depth != 0 {
        return Err("unbalanced parentheses: open never closed".to_string());
    }

    Ok(max)
}
