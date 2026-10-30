pub fn second_largest_distinct(xs: &[i64]) -> Result<i64, String> {
    let max = match xs.iter().max() {
        Some(&m) => m,
        None => return Err("empty list".to_string()),
    };
    xs.iter()
        .copied()
        .filter(|&x| x < max)
        .max()
        .ok_or_else(|| "fewer than two distinct values".to_string())
}
