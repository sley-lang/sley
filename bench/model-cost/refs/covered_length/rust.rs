pub fn covered_length(xs: &[i64]) -> Result<i64, String> {
    if xs.len() % 2 != 0 {
        return Err("odd number of elements".to_string());
    }
    let mut intervals: Vec<(i64, i64)> = Vec::new();
    for pair in xs.chunks(2) {
        if pair[0] > pair[1] {
            return Err("interval start greater than end".to_string());
        }
        intervals.push((pair[0], pair[1]));
    }
    intervals.sort();
    let mut total = 0i64;
    let mut current: Option<(i64, i64)> = None;
    for (start, end) in intervals {
        match current {
            Some((cs, ce)) if start <= ce => {
                current = Some((cs, ce.max(end)));
            }
            Some((cs, ce)) => {
                total += ce - cs;
                current = Some((start, end));
            }
            None => current = Some((start, end)),
        }
    }
    if let Some((cs, ce)) = current {
        total += ce - cs;
    }
    Ok(total)
}
