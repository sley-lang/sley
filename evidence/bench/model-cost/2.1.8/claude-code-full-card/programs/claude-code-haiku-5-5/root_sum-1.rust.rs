pub fn root_sum(n: i64) -> i64 {
    let mut total: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        let mut x = i;
        while x >= 10 {
            let mut s = 0;
            while x > 0 {
                s += x % 10;
                x /= 10;
            }
            x = s;
        }
        total += x;
        i += 1;
    }
    total
}
