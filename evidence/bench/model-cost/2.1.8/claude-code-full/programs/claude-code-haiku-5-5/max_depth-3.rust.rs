pub fn max_depth(codes: &[i64]) -> Result<i64, String> {
    let mut depth: i64 = 0;
    let mut max: i64 = 0;

    for (i, &code) in codes.iter().enumerate() {
        match code {
            40 => {
                depth += 1;
                if depth > max {
                    max = depth;
                }
            }
            41 => {
                if depth == 0 {
                    return Err(format!("unbalanced ')' at index {}", i));
                }
                depth -= 1;
            }
            _ => {}
        }
    }

    if depth != 0 {
        return Err(format!("unclosed '(' : {} left open", depth));
    }

    Ok(max)
}
