pub fn max_subarray(xs: &[i64]) -> Result<i64, String> {
    let first = *xs.first().ok_or("empty")?;
    let (mut best, mut cur) = (first, first);
    for &x in &xs[1..] {
        cur = x.max(cur + x);
        best = best.max(cur);
    }
    Ok(best)
}
