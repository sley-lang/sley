pub fn root_sum(n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    // The digital root of i (for i >= 1) is 1 + (i - 1) % 9, so each
    // complete block of 9 consecutive integers contributes 1 + ... + 9 = 45.
    let full_blocks = n / 9;
    let rem = n % 9;
    full_blocks * 45 + rem * (rem + 1) / 2
}
