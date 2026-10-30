pub fn caesar_shift(s: &str, k: i64) -> String {
    let shift = k.rem_euclid(26) as u8;
    s.chars()
        .map(|c| {
            if c.is_ascii_lowercase() {
                (b'a' + (c as u8 - b'a' + shift) % 26) as char
            } else if c.is_ascii_uppercase() {
                (b'A' + (c as u8 - b'A' + shift) % 26) as char
            } else {
                c
            }
        })
        .collect()
}
