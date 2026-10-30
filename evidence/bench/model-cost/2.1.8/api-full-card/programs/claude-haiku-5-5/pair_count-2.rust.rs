use std::collections::HashMap;

pub fn pair_count(xs: &[i64], k: i64) -> i64 {
    let mut counts: HashMap<i64, i64> = HashMap::new();
    let mut total: i64 = 0;
    for &x in xs {
        let c = k as i128 - x as i128;
        if c >= i64::MIN as i128 && c <= i64::MAX as i128 {
            if let Some(&n) = counts.get(&(c as i64)) {
                total += n;
            }
        }
        *counts.entry(x).or_insert(0) += 1;
    }
    total
}
