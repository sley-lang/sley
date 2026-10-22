pub fn primes(n: i64) -> i64 {
    let mut count = 0;
    for k in 2..n {
        let mut d = 2;
        let mut is_p = true;
        while d * d <= k {
            if k % d == 0 {
                is_p = false;
                break;
            }
            d += 1;
        }
        if is_p {
            count += 1;
        }
    }
    count
}
