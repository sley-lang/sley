pub fn max_depth(codes: &[i64]) -> Result<i64, String> {
    const OPEN: i64 = 40;
    const CLOSE: i64 = 41;

    let mut depth: i64 = 0;
    let mut max: i64 = 0;

    for (i, &code) in codes.iter().enumerate() {
        if code == OPEN {
            depth += 1;
            if depth > max {
                max = depth;
            }
        } else if code == CLOSE {
            if depth == 0 {
                return Err(format!("unbalanced ')' at index {}", i));
            }
            depth -= 1;
        }
    }

    if depth != 0 {
        return Err(format!("unbalanced '(': {} unclosed", depth));
    }

    Ok(max)
}
