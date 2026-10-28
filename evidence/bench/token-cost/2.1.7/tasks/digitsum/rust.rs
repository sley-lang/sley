pub fn digitsum(n: i64) -> i64 {
    let mut total = 0;
    for i in 0..n {
        let mut x = i;
        while x > 0 {
            total += x % 10;
            x /= 10;
        }
    }
    total
}
