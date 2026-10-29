pub fn clamp_sum(xs: &[i64], lo: i64, hi: i64) -> Result<i64, String> {
    Ok(xs.iter().map(|&x| x.max(lo).min(hi)).sum())
}
