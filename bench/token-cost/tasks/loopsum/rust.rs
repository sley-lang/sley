pub fn loopsum(n: i64) -> i64 {
    let mut s = 0;
    for i in 0..n {
        s = (s + i * i) % 1_000_000_007;
    }
    s
}
