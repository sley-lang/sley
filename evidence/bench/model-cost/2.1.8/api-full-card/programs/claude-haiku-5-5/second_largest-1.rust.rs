pub fn second_largest(xs: &[i64]) -> Result<i64, String> {
    let mut largest: Option<i64> = None;
    let mut second: Option<i64> = None;

    for &x in xs {
        match largest {
            None => largest = Some(x),
            Some(l) => {
                if x > l {
                    second = Some(l);
                    largest = Some(x);
                } else if x < l {
                    match second {
                        None => second = Some(x),
                        Some(s) => {
                            if x > s {
                                second = Some(x);
                            }
                        }
                    }
                }
            }
        }
    }

    second.ok_or_else(|| String::from("fewer than two distinct values"))
}
