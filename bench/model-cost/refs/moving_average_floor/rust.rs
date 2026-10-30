pub fn moving_average_floor(xs: &[i64], w: i64) -> Result<Vec<i64>, String> {
    if w < 1 {
        return Err("w must be at least 1".to_string());
    }
    if w as usize > xs.len() {
        return Ok(Vec::new());
    }
    Ok(xs
        .windows(w as usize)
        .map(|win| win.iter().sum::<i64>().div_euclid(w))
        .collect())
}
