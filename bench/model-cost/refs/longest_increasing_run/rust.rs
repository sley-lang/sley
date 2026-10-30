pub fn longest_increasing_run(xs: &[i64]) -> i64 {
    if xs.is_empty() {
        return 0;
    }
    let mut best = 1i64;
    let mut run = 1i64;
    for i in 1..xs.len() {
        if xs[i] > xs[i - 1] {
            run += 1;
        } else {
            run = 1;
        }
        best = best.max(run);
    }
    best
}
