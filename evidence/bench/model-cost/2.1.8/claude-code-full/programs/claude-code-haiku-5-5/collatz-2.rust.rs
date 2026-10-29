pub fn collatz(limit: i64) -> i64 {
    if limit <= 1 {
        return 0;
    }

    let mut best_n: i64 = 1;
    let mut best_steps: u32 = 0;

    for start in 1..limit {
        let mut n = start;
        let mut steps: u32 = 0;
        while n != 1 {
            n = if n % 2 == 0 { n / 2 } else { 3 * n + 1 };
            steps += 1;
        }
        // Strict comparison keeps the smallest n on ties, since we scan upward.
        if steps > best_steps {
            best_steps = steps;
            best_n = start;
        }
    }

    best_n
}
