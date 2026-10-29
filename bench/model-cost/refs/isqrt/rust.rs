pub fn isqrt(n: i64) -> i64 {
    let mut total = 0;
    for i in 0..n {
        let (mut lo, mut hi) = (0, i + 1);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if mid * mid <= i { lo = mid } else { hi = mid }
        }
        total += lo;
    }
    total
}
