use std::collections::HashSet;

pub fn count_distinct_words(s: &str) -> i64 {
    let mut words: HashSet<String> = HashSet::new();
    let mut current = String::new();
    for c in s.chars() {
        if c.is_ascii_alphabetic() {
            current.push(c.to_ascii_lowercase());
        } else if !current.is_empty() {
            words.insert(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        words.insert(current);
    }
    words.len() as i64
}
