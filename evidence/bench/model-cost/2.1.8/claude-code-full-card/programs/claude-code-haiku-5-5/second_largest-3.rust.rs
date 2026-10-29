pub fn second_largest(xs: &[i64]) -> Result<i64, String> {
    let mut largest: Option<i64> = None;
    let mut second: Option<i64> = None;

    for &x in xs {
        match largest {
            None => largest = Some(x),
            Some(m) => {
                if x > m {
                    second = Some(m);
                    largest = Some(x);
                } else if x < m {
                    if second.map_or(true, |s| x > s) {
                        second = Some(x);
                    }
                }
                // x == m: duplicate of the current largest, ignore.
            }
        }
    }

    second.ok_or_else(|| "fewer than two distinct values".to_string())
}
