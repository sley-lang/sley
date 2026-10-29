pub fn count_leq(xs: &[i64], x: i64) -> i64 {
    xs.partition_point(|&v| v <= x) as i64
}
