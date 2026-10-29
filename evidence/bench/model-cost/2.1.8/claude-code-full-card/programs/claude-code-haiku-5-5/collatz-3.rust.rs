pub fn collatz(limit: i64) -> i64 {
    if limit <= 1 {
        return 0;
    }

    let mut best_n: i64 = 1;
    let mut best_steps: u32 = 0;

    for n in 1..limit {
        let mut x: u128 = n as u128;
        let mut steps: u32 = 0;
        while x != 1 {
            x = if x % 2 == 0 { x / 2 } else { 3 * x + 1 };
            steps += 1;
        }
        if steps > best_steps {
            best_steps = steps;
            best_n = n;
        }
    }

    best_n
}
