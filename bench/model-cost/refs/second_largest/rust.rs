pub fn second_largest(xs: &[i64]) -> Result<i64, String> {
    let top = *xs.iter().max().ok_or("too few values")?;
    xs.iter().copied().filter(|&x| x != top).max().ok_or_else(|| "too few values".to_string())
}
