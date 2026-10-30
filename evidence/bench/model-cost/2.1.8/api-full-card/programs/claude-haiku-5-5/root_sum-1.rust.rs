pub fn root_sum(n: i64) -> i64 {
    let mut total: i64 = 0;
    let mut k: i64 = 1;
    while k <= n {
        let mut x = k;
        while x >= 10 {
            let mut s = 0;
            let mut y = x;
            while y > 0 {
                s += y % 10;
                y /= 10;
            }
            x = s;
        }
        total += x;
        k += 1;
    }
    total
}
