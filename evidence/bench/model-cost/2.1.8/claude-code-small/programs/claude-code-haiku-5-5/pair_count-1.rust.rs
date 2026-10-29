use std::collections::HashMap;

pub fn pair_count(xs: &[i64], k: i64) -> i64 {
    let mut seen: HashMap<i64, i64> = HashMap::new();
    let mut total: i64 = 0;
    for &x in xs {
        // Use i128 so that k - x cannot overflow for any i64 inputs.
        let need = k as i128 - x as i128;
        if need >= i64::MIN as i128 && need <= i64::MAX as i128 {
            if let Some(&c) = seen.get(&(need as i64)) {
                total += c;
            }
        }
        *seen.entry(x).or_insert(0) += 1;
    }
    total
}
