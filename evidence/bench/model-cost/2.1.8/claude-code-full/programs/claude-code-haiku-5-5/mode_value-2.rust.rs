use std::collections::HashMap;

pub fn mode_value(xs: &[i64]) -> Result<i64, String> {
    if xs.is_empty() {
        return Err(String::from("mode_value: input slice is empty"));
    }

    let mut counts: HashMap<i64, usize> = HashMap::new();
    for &x in xs {
        *counts.entry(x).or_insert(0) += 1;
    }

    let mut best_value = xs[0];
    let mut best_count = 0usize;
    for (&value, &count) in &counts {
        if count > best_count || (count == best_count && value < best_value) {
            best_value = value;
            best_count = count;
        }
    }

    Ok(best_value)
}
