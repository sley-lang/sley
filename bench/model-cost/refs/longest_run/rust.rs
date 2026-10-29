pub fn longest_run(xs: &[i64]) -> i64 {
    let (mut best, mut run) = (0, 0);
    for i in 0..xs.len() {
        run = if i > 0 && xs[i] == xs[i - 1] { run + 1 } else { 1 };
        best = best.max(run);
    }
    best
}
