pub fn root_sum(n: i64) -> i64 {
    fn digital_root(mut x: i64) -> i64 {
        while x >= 10 {
            let mut s = 0;
            while x > 0 {
                s += x % 10;
                x /= 10;
            }
            x = s;
        }
        x
    }

    (1..=n).map(digital_root).sum()
}
