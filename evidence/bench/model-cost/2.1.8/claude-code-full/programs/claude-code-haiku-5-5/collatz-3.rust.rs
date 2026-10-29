pub fn collatz(limit: i64) -> i64 {
    if limit <= 1 {
        return 0;
    }

    fn steps(mut n: i64) -> u32 {
        let mut count = 0u32;
        while n != 1 {
            n = if n % 2 == 0 { n / 2 } else { 3 * n + 1 };
            count += 1;
        }
        count
    }

    let mut best_n: i64 = 1;
    let mut best_steps: u32 = 0;

    for n in 1..limit {
        let s = steps(n);
        if s > best_steps {
            best_steps = s;
            best_n = n;
        }
    }

    best_n
}
