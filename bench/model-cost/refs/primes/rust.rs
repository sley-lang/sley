pub fn primes(n: i64) -> i64 {
    (2..n.max(2)).filter(|&k| (2..).take_while(|d| d * d <= k).all(|d| k % d != 0)).count() as i64
}
