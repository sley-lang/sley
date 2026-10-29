pub fn longest_run(xs: &[i64]) -> i64 {
    let mut best: i64 = 0;
    let mut current: i64 = 0;
    let mut prev: Option<i64> = None;

    for &x in xs {
        if prev == Some(x) {
            current += 1;
        } else {
            current = 1;
            prev = Some(x);
        }
        if current > best {
            best = current;
        }
    }

    best
}
