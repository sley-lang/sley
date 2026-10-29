pub fn digitsum(n: i64) -> i64 {
    fn digit_sum(mut x: i64) -> i64 {
        let mut s = 0;
        while x > 0 {
            s += x % 10;
            x /= 10;
        }
        s
    }

    let mut total = 0;
    let mut i = 0;
    while i < n {
        total += digit_sum(i);
        i += 1;
    }
    total
}
