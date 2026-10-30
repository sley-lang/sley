pub fn transform(xs: &[i64], lo: i64, hi: i64) -> i64 {
    let mut total: i128 = 0;
    for &x in xs {
        let v: i128 = if x % 2 != 0 {
            2 * (x as i128)
        } else {
            x as i128
        };
        let c = v.clamp(lo as i128, hi as i128);
        total += c;
    }
    total.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}
