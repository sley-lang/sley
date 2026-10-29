pub fn transform(xs: &[i64], lo: i64, hi: i64) -> i64 {
    xs.iter()
        .map(|&x| {
            let mapped = if x % 2 != 0 { x.saturating_mul(2) } else { x };
            mapped.clamp(lo, hi)
        })
        .fold(0i64, |acc, v| acc.saturating_add(v))
}
