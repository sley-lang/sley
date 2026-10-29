pub fn root_sum(n: i64) -> i64 {
    fn digital_root(mut k: i64) -> i64 {
        while k >= 10 {
            let mut s = 0;
            while k > 0 {
                s += k % 10;
                k /= 10;
            }
            k = s;
        }
        k
    }

    (1..=n).map(digital_root).sum()
}
