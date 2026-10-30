pub fn count_pairs_with_sum(xs: &[i64], target: i64) -> i64 {
    let mut count = 0i64;
    for i in 0..xs.len() {
        for j in (i + 1)..xs.len() {
            if xs[i] + xs[j] == target {
                count += 1;
            }
        }
    }
    count
}
