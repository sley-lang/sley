pub fn first_overdraft(start: i64, xs: &[i64]) -> i64 {
    let mut balance = start;
    for (i, &x) in xs.iter().enumerate() {
        balance += x;
        if balance < 0 {
            return i as i64;
        }
    }
    -1
}
