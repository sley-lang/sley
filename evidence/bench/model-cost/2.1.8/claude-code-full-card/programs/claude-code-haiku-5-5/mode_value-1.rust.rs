pub fn mode_value(xs: &[i64]) -> Result<i64, String> {
    if xs.is_empty() {
        return Err(String::from("mode_value: input slice is empty"));
    }

    let mut sorted: Vec<i64> = xs.to_vec();
    sorted.sort_unstable();

    let mut best_value = sorted[0];
    let mut best_count: usize = 0;

    let mut i = 0;
    while i < sorted.len() {
        let current = sorted[i];
        let mut j = i;
        while j < sorted.len() && sorted[j] == current {
            j += 1;
        }
        let count = j - i;
        // Strictly greater keeps the earlier (smaller) value on ties,
        // since the slice is sorted ascending.
        if count > best_count {
            best_count = count;
            best_value = current;
        }
        i = j;
    }

    Ok(best_value)
}
