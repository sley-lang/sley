pub fn inversions(xs: &[i64]) -> i64 {
    let mut count = 0;
    for i in 0..xs.len() {
        for j in i + 1..xs.len() {
            if xs[j] < xs[i] {
                count += 1;
            }
        }
    }
    count
}
