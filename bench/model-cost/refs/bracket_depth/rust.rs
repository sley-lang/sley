pub fn bracket_depth(s: &str) -> Result<i64, String> {
    let mut stack: Vec<char> = Vec::new();
    let mut max_depth = 0i64;
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => {
                stack.push(c);
                max_depth = max_depth.max(stack.len() as i64);
            }
            ')' | ']' | '}' => {
                let expected = match c {
                    ')' => '(',
                    ']' => '[',
                    _ => '{',
                };
                if stack.pop() != Some(expected) {
                    return Err("mismatched closing bracket".to_string());
                }
            }
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err("unclosed bracket".to_string());
    }
    Ok(max_depth)
}
