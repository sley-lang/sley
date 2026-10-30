pub fn isqrt(n: i64) -> i64 {
    let mut total: i64 = 0;
    let mut r: i64 = 0;
    let mut i: i64 = 0;
    while i < n {
        // advance r so that r * r <= i < (r + 1) * (r + 1)
        while (r + 1) * (r + 1) <= i {
            r += 1;
        }
        total += r;
        i += 1;
    }
    total
}
