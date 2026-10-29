use std::collections::HashMap;

pub fn pair_count(xs: &[i64], k: i64) -> i64 {
    let mut seen: HashMap<i64, i64> = HashMap::new();
    let mut total: i64 = 0;
    for &x in xs {
        // Compute k - x in i128 to avoid overflow at the i64 extremes.
        let need = k as i128 - x as i128;
        if let Ok(need) = i64::try_from(need) {
            if let Some(&c) = seen.get(&need) {
                total += c;
            }
        }
        *seen.entry(x).or_insert(0) += 1;
    }
    total
}
