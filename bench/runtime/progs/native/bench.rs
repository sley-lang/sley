use std::env;

fn fib(n: i64) -> i64 { if n < 2 { n } else { fib(n - 1) + fib(n - 2) } }

fn loopsum(n: i64) -> i64 {
    let mut s = 0;
    for i in 0..n { s = (s + i * i) % 1_000_000_007; }
    s
}

fn collatz(limit: i64) -> i64 {
    let (mut best, mut best_n) = (0, 0);
    for n in 1..limit {
        let (mut x, mut steps) = (n, 0);
        while x != 1 {
            x = if x % 2 == 0 { x / 2 } else { 3 * x + 1 };
            steps += 1;
        }
        if steps > best { best = steps; best_n = n; }
    }
    best_n
}

fn primes(n: i64) -> i64 {
    let mut count = 0;
    for k in 2..n {
        let mut d = 2;
        let mut is_p = true;
        while d * d <= k {
            if k % d == 0 { is_p = false; break; }
            d += 1;
        }
        if is_p { count += 1; }
    }
    count
}

fn gcd(mut a: i64, mut b: i64) -> i64 {
    while b != 0 { (a, b) = (b, a % b); }
    a
}

fn gcdsum(n: i64) -> i64 {
    let mut s = 0;
    for i in 1..n { for j in 1..n { s += gcd(i, j); } }
    s
}

fn main() {
    let a: Vec<String> = env::args().collect();
    let n: i64 = a[2].parse().unwrap();
    let r = match a[1].as_str() {
        "fib" => fib(n), "loopsum" => loopsum(n), "collatz" => collatz(n),
        "primes" => primes(n), "gcdsum" => gcdsum(n), _ => std::process::exit(2),
    };
    println!("{r}");
}
