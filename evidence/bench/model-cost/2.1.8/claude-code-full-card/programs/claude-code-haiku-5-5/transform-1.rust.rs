pub fn transform(xs: &[i64], lo: i64, hi: i64) -> i64 {
    let mut sum: i128 = 0;
    for &x in xs {
        let mapped: i128 = if x % 2 != 0 {
            2 * (x as i128)
        } else {
            x as i128
        };
        let clamped = mapped.clamp(lo as i128, hi as i128);
        sum += clamped;
    }
    sum.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}
