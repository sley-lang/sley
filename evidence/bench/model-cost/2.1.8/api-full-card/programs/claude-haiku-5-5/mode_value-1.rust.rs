use std::collections::HashMap;

pub fn mode_value(xs: &[i64]) -> Result<i64, String> {
    if xs.is_empty() {
        return Err("empty input".to_string());
    }
    let mut counts: HashMap<i64, usize> = HashMap::new();
    for &x in xs {
        *counts.entry(x).or_insert(0) += 1;
    }
    let mut best_val = i64::MAX;
    let mut best_count = 0usize;
    for (&v, &c) in counts.iter() {
        if c > best_count || (c == best_count && v < best_val) {
            best_count = c;
            best_val = v;
        }
    }
    Ok(best_val)
}
