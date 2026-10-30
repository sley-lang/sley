pub fn collatz(limit: i64) -> i64 {
    if limit <= 1 {
        return 0;
    }
    let mut best_n: i64 = 1;
    let mut best_steps: u64 = 0;
    let mut n: i64 = 1;
    while n < limit {
        let mut x: i64 = n;
        let mut steps: u64 = 0;
        while x != 1 {
            if x % 2 == 0 {
                x /= 2;
            } else {
                x = 3 * x + 1;
            }
            steps += 1;
        }
        if steps > best_steps {
            best_steps = steps;
            best_n = n;
        }
        n += 1;
    }
    best_n
}
