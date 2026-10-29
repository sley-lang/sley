pub fn max_subarray(xs: &[i64]) -> Result<i64, String> {
    let first = match xs.first() {
        Some(&v) => v,
        None => return Err(String::from("max_subarray: input slice is empty")),
    };

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
