pub fn max_subarray(xs: &[i64]) -> Result<i64, String> {
    let first = *xs
        .first()
        .ok_or_else(|| String::from("max_subarray: input slice is empty"))?;

    let mut best = first;
    let mut current = first;

    for &x in &xs[1..] {
        current = if current > 0 { current + x } else { x };
        if current > best {
            best = current;
        }
    }

    Ok(best)
}
