pub fn triples(n: i64) -> i64 {
    let mut count = 0;
    for c in 1..=n {
        for b in 1..c {
            for a in 1..b {
                if a * a + b * b == c * c {
                    count += 1;
                }
            }
        }
    }
    count
}
