use std::collections::HashMap;

pub fn mode_value(xs: &[i64]) -> Result<i64, String> {
    if xs.is_empty() {
        return Err("empty input".to_string());
    }
    let mut counts: HashMap<i64, usize> = HashMap::new();
    for &x in xs {
        *counts.entry(x).or_insert(0) += 1;
    }
    let mut best: Option<(usize, i64)> = None;
    for (&v, &c) in &counts {
        best = match best {
            None => Some((c, v)),
            Some((bc, bv)) => {
                if c > bc || (c == bc && v < bv) {
                    Some((c, v))
                } else {
                    Some((bc, bv))
                }
            }
        };
    }
    best.map(|(_, v)| v).ok_or_else(|| "empty input".to_string())
}
