pub fn count_multiples(xs: &[i64], k: i64) -> Result<i64, String> {
    if k <= 0 {
        return Err("k must be positive".into());
    }
    Ok(xs.iter().filter(|&&x| x % k == 0).count() as i64)
}
