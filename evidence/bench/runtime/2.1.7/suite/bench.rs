use std::env;
fn fib(n: i64) -> i64 { if n < 2 { n } else { fib(n - 1) + fib(n - 2) } }
fn loopsum(n: i64) -> i64 { let mut s = 0; for i in 0..n { s = (s + i * i) % 1_000_000_007; } s }
fn collatz(limit: i64) -> i64 {
    let (mut best, mut best_n) = (0, 0);
    for n in 1..limit {
        let (mut x, mut steps) = (n, 0);
        while x != 1 { x = if x % 2 == 0 { x / 2 } else { 3 * x + 1 }; steps += 1; }
        if steps > best { best = steps; best_n = n; }
    }
    best_n
}
fn primes(n: i64) -> i64 {
    let mut count = 0;
    for k in 2..n {
        let mut d = 2; let mut is_p = true;
        while d * d <= k { if k % d == 0 { is_p = false; break; } d += 1; }
        if is_p { count += 1; }
    }
    count
}
fn gcd(mut a: i64, mut b: i64) -> i64 { while b != 0 { (a, b) = (b, a % b); } a }
fn gcdsum(n: i64) -> i64 { let mut s = 0; for i in 1..n { for j in 1..n { s += gcd(i, j); } } s }
fn tak(x: i64, y: i64, z: i64) -> i64 { if y < x { tak(tak(x - 1, y, z), tak(y - 1, z, x), tak(z - 1, x, y)) } else { z } }
fn triples(n: i64) -> i64 {
    let mut count = 0;
    for c in 1..=n { for b in 1..c { for a in 1..b { if a * a + b * b == c * c { count += 1; } } } }
    count
}
fn digitsum(n: i64) -> i64 { let mut t = 0; for i in 0..n { let mut x = i; while x > 0 { t += x % 10; x /= 10; } } t }
fn isqrt(n: i64) -> i64 {
    let mut total = 0;
    for i in 0..n {
        let (mut lo, mut hi) = (0i64, i + 1);
        while hi - lo > 1 { let mid = (lo + hi) / 2; if mid * mid <= i { lo = mid } else { hi = mid } }
        total += lo;
    }
    total
}
fn inversions(xs: &[i64]) -> i64 {
    let mut count = 0;
    for i in 0..xs.len() { let x = xs[i]; for j in i + 1..xs.len() { if xs[j] < x { count += 1; } } }
    count
}
fn main() {
    let a: Vec<String> = env::args().collect();
    let num = |k: usize| a[k].parse::<i64>().unwrap();
    let r = match a[1].as_str() {
        "inversions" => {
            let s = std::fs::read_to_string(&a[2]).unwrap();
            let xs: Vec<i64> = s.trim_matches(|c| c == '[' || c == ']' || c == '\n').split(',').map(|t| t.trim().parse().unwrap()).collect();
            inversions(&xs)
        }
        "tak" => tak(num(2), num(3), num(4)),
        "fib" => fib(num(2)), "loopsum" => loopsum(num(2)), "collatz" => collatz(num(2)),
        "primes" => primes(num(2)), "gcdsum" => gcdsum(num(2)), "triples" => triples(num(2)),
        "digitsum" => digitsum(num(2)), "isqrt" => isqrt(num(2)),
        _ => std::process::exit(2),
    };
    println!("{r}");
}
