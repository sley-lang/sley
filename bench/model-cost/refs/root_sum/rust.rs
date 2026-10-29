pub fn root_sum(n: i64) -> i64 {
    (1..=n).map(|i| 1 + (i - 1) % 9).sum()
}
