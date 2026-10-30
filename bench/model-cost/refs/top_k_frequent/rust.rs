use std::collections::HashMap;

pub fn top_k_frequent(xs: &[i64], k: i64) -> Vec<i64> {
    let mut counts: HashMap<i64, i64> = HashMap::new();
    for &x in xs {
        *counts.entry(x).or_insert(0) += 1;
    }
    let mut entries: Vec<(i64, i64)> = counts.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let take = if k < 0 { 0 } else { k as usize };
    entries.into_iter().take(take).map(|(v, _)| v).collect()
}
