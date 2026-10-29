pub fn transform(xs: &[i64], lo: i64, hi: i64) -> i64 {
    xs.iter().map(|&x| (if x % 2 != 0 { 2 * x } else { x }).clamp(lo, hi)).sum()
}
