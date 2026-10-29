pub fn max_subarray(xs: &[i64]) -> Result<i64, String> {
    let first = *xs
        .first()
        .ok_or_else(|| String::from("max_subarray: input slice is empty"))?;

    let mut best = first;
    let mut current = first;

    for &x in &xs[1..] {
        // Extend the current run or start fresh at x, whichever is larger.
        current = match current.checked_add(x) {
            Some(sum) if sum >= x => sum,
            Some(_) => x,
            None => {
                // Overflow means the extended sum exceeds i64::MAX, which is
                // larger than any fresh start x, so it is always the better
                // choice here. Report the overflow instead of wrapping.
                return Err(String::from("max_subarray: arithmetic overflow"));
            }
        };
        if current > best {
            best = current;
        }
    }

    Ok(best)
}
