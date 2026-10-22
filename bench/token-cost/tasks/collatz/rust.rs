pub fn collatz(limit: i64) -> i64 {
    let (mut best, mut best_n) = (0, 0);
    for n in 1..limit {
        let mut x = n;
        let mut steps = 0;
        while x != 1 {
            x = if x % 2 == 0 { x / 2 } else { 3 * x + 1 };
            steps += 1;
        }
        if steps > best {
            best = steps;
            best_n = n;
        }
    }
    best_n
}
