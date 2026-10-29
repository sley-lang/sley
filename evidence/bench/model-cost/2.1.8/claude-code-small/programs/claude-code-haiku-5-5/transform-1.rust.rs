pub fn transform(xs: &[i64], lo: i64, hi: i64) -> i64 {
    xs.iter().fold(0i64, |acc, &x| {
        let mapped = if x & 1 == 1 { x.saturating_mul(2) } else { x };
        let clamped = mapped.clamp(lo, hi);
        acc.saturating_add(clamped)
    })
}
