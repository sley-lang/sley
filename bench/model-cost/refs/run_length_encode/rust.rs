pub fn run_length_encode(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        let mut count: usize = 1;
        while chars.peek() == Some(&c) {
            chars.next();
            count += 1;
        }
        out.push(c);
        out.push_str(&count.to_string());
    }
    out
}
