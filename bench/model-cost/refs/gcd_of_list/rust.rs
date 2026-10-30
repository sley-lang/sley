pub fn gcd_of_list(xs: &[i64]) -> i64 {
    fn gcd(mut a: i64, mut b: i64) -> i64 {
        while b != 0 {
            let t = a % b;
            a = b;
            b = t;
        }
        a
    }
    xs.iter().fold(0, |acc, &x| gcd(acc, x.abs()))
}
