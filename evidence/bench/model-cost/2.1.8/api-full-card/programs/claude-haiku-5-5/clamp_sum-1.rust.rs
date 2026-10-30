pub fn clamp_sum(xs: &[i64], lo: i64, hi: i64) -> Result<i64, String> {
    if lo > hi {
        return Err(format!("lo ({}) is greater than hi ({})", lo, hi));
    }
    Ok(xs
        .iter()
        .filter(|&&x| x >= 0)
        .map(|&x| x.max(lo).min(hi))
        .sum())
}
