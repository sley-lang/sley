pub fn max_depth(codes: &[i64]) -> Result<i64, String> {
    let (mut depth, mut best) = (0i64, 0i64);
    for &c in codes {
        if c == 40 {
            depth += 1;
            best = best.max(depth);
        } else if c == 41 {
            depth -= 1;
            if depth < 0 {
                return Err("unbalanced".into());
            }
        }
    }
    if depth != 0 {
        return Err("unbalanced".into());
    }
    Ok(best)
}
