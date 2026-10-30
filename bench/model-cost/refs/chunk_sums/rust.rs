pub fn chunk_sums(xs: &[i64], size: i64) -> Result<Vec<i64>, String> {
    if size < 1 {
        return Err("size must be at least 1".to_string());
    }
    Ok(xs.chunks(size as usize).map(|c| c.iter().sum()).collect())
}
