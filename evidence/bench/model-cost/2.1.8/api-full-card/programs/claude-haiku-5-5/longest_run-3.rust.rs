pub fn longest_run(xs: &[i64]) -> i64 {
    if xs.is_empty() {
        return 0;
    }
    let mut best: i64 = 1;
    let mut cur: i64 = 1;
    for i in 1..xs.len() {
        if xs[i] == xs[i - 1] {
            cur += 1;
        } else {
            cur = 1;
        }
        if cur > best {
            best = cur;
        }
    }
    best
}
