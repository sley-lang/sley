pub fn fib(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    for _ in 0..n {
        let next = a + b;
        a = b;
        b = next;
    }
    a
}
