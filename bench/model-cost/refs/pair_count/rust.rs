pub fn pair_count(xs: &[i64], k: i64) -> i64 {
    let mut count = 0;
    for i in 0..xs.len() {
        for j in i + 1..xs.len() {
            if xs[i] + xs[j] == k {
                count += 1;
            }
        }
    }
    count
}
