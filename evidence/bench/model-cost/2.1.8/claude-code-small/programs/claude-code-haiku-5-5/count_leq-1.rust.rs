pub fn count_leq(xs: &[i64], x: i64) -> i64 {
    // Binary search for the first index where xs[i] > x.
    // Every element before that index is <= x, since xs is sorted ascending.
    let mut lo: usize = 0;
    let mut hi: usize = xs.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if xs[mid] <= x {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo as i64
}
