pub fn count_multiples(xs: &[i64], k: i64) -> Result<i64, String> {
    Ok(xs.iter().filter(|&&x| x % 2 == 0).count() as i64)
}
