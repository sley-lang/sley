pub fn max_subarray(xs: &[i64]) -> Result<i64, String> {
    let (&first, rest) = xs
        .split_first()
        .ok_or_else(|| String::from("max_subarray: input slice is empty"))?;

    let mut best = first;
    let mut current = first;

    for &x in rest {
        // Extend the current run or start fresh at x, whichever is larger.
        current = match current.checked_add(x) {
            Some(sum) if sum >= x => sum,
            Some(_) => x,
            None => x,
        };
        if current > best {
            best = current;
        }
    }

    Ok(best)
}
