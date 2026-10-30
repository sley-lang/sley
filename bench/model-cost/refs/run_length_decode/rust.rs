pub fn run_length_decode(s: &str) -> Result<String, String> {
    const MAX_LEN: usize = 1000;
    let bytes = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_digit() || !(0x20..=0x7e).contains(&c) {
            return Err("expected a non-digit printable character".to_string());
        }
        i += 1;
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if start == i {
            return Err("character without a count".to_string());
        }
        if bytes[start] == b'0' {
            return Err("count has a leading zero".to_string());
        }
        let mut count: usize = 0;
        for &d in &bytes[start..i] {
            count = count * 10 + (d - b'0') as usize;
            if count > MAX_LEN {
                return Err("decoded string too long".to_string());
            }
        }
        if out.len() + count > MAX_LEN {
            return Err("decoded string too long".to_string());
        }
        for _ in 0..count {
            out.push(c as char);
        }
    }
    Ok(out)
}
