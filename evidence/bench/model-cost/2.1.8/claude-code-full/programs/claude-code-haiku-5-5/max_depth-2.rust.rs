pub fn max_depth(codes: &[i64]) -> Result<i64, String> {
    let mut depth: i64 = 0;
    let mut max: i64 = 0;

    for &code in codes {
        match code {
            40 => {
                depth += 1;
                if depth > max {
                    max = depth;
                }
            }
            41 => {
                if depth == 0 {
                    return Err("unbalanced parentheses: close without matching open".to_string());
                }
                depth -= 1;
            }
            _ => {}
        }
    }

    if depth != 0 {
        return Err("unbalanced parentheses: open never closed".to_string());
    }

    Ok(max)
}
