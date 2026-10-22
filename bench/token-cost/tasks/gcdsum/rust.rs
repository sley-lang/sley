pub fn gcd(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

pub fn gcdsum(n: i64) -> i64 {
    let mut s = 0;
    for i in 1..n {
        for j in 1..n {
            s += gcd(i, j);
        }
    }
    s
}
