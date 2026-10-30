pub fn count_by_remainder(xs: &[i64], m: i64) -> Result<Vec<i64>, String> {
    if m < 1 || m > 200 {
        return Err("m must be within [1, 200]".to_string());
    }
    let mut counts = vec![0i64; m as usize];
    for &x in xs {
        counts[x.rem_euclid(m) as usize] += 1;
    }
    Ok(counts)
}
