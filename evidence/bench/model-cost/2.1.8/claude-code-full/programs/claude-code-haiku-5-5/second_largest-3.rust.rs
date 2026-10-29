pub fn second_largest(xs: &[i64]) -> Result<i64, String> {
    let mut largest: Option<i64> = None;
    let mut second: Option<i64> = None;

    for &x in xs {
        match largest {
            None => largest = Some(x),
            Some(l) if x > l => {
                second = Some(l);
                largest = Some(x);
            }
            Some(l) if x < l => {
                if second.map_or(true, |s| x > s) {
                    second = Some(x);
                }
            }
            _ => {}
        }
    }

    match (largest, second) {
        (Some(_), Some(s)) => Ok(s),
        _ => Err("fewer than two distinct values".to_string()),
    }
}
