pub fn digitsum(n: i64) -> i64 {
    let mut total: i64 = 0;
    let mut i: i64 = 0;
    while i < n {
        let mut x = i;
        while x > 0 {
            total += x % 10;
            x /= 10;
        }
        i += 1;
    }
    total
}
