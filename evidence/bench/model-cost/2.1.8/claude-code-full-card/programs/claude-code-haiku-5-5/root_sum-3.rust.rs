pub fn root_sum(n: i64) -> i64 {
    // The digital root of a positive integer i is 1 + (i - 1) mod 9.
    let mut total: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        total += 1 + (i - 1) % 9;
        i += 1;
    }
    total
}
