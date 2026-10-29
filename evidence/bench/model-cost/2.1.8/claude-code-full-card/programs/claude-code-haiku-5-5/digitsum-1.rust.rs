pub fn digitsum(n: i64) -> i64 {
    fn digits_of(mut x: i64) -> i64 {
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
        total += digits_of(i);
        i += 1;
    }
    total
}
