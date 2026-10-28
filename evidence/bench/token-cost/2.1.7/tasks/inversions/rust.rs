pub fn inversions(xs: &[i64]) -> i64 {
    let mut count = 0;
    for i in 0..xs.len() {
        let x = xs[i];
        for j in i + 1..xs.len() {
            if xs[j] < x {
                count += 1;
            }
        }
    }
    count
}
