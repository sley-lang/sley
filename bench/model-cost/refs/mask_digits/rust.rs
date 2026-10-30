pub fn mask_digits(s: &str) -> String {
    let total = s.chars().filter(|c| c.is_ascii_digit()).count();
    let to_mask = total.saturating_sub(4);
    let mut seen = 0usize;
    s.chars()
        .map(|c| {
            if c.is_ascii_digit() {
                seen += 1;
                if seen <= to_mask {
                    return '*';
                }
            }
            c
        })
        .collect()
}
