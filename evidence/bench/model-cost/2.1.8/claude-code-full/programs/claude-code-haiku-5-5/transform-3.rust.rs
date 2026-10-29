pub fn transform(xs: &[i64], lo: i64, hi: i64) -> i64 {
    let mut total: i128 = 0;
    for &x in xs {
        let mapped = if x % 2 != 0 { x.saturating_mul(2) } else { x };
        let clamped = mapped.clamp(lo, hi);
        total += clamped as i128;
    }
    total.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}
