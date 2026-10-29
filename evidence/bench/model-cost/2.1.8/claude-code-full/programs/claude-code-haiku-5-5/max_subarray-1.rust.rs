pub fn max_subarray(xs: &[i64]) -> Result<i64, String> {
    if xs.is_empty() {
        return Err(String::from("max_subarray: input slice is empty"));
    }

    let mut best = xs[0];
    let mut current = xs[0];

    for &x in &xs[1..] {
        current = if current > 0 { current + x } else { x };
        if current > best {
            best = current;
        }
    }

    Ok(best)
}
