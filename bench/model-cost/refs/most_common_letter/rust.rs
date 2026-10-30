pub fn most_common_letter(s: &str) -> Result<String, String> {
    let mut counts = [0usize; 26];
    for b in s.bytes() {
        if b.is_ascii_alphabetic() {
            counts[(b.to_ascii_lowercase() - b'a') as usize] += 1;
        }
    }
    let mut best: Option<usize> = None;
    for i in 0..26 {
        if counts[i] > 0 && best.map_or(true, |j| counts[i] > counts[j]) {
            best = Some(i);
        }
    }
    match best {
        Some(i) => Ok(((b'a' + i as u8) as char).to_string()),
        None => Err("no ASCII letters".to_string()),
    }
}
