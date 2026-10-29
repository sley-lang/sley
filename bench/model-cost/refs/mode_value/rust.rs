pub fn mode_value(xs: &[i64]) -> Result<i64, String> {
    let mut counts = std::collections::BTreeMap::new();
    for &x in xs {
        *counts.entry(x).or_insert(0) += 1;
    }
    let mut best: Option<(i64, i64)> = None;
    for (&v, &c) in &counts {
        if best.map_or(true, |(_, bc)| c > bc) {
            best = Some((v, c));
        }
    }
    best.map(|(v, _)| v).ok_or_else(|| "empty".to_string())
}
