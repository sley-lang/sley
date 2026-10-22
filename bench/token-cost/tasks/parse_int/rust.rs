#[derive(Debug, PartialEq)]
pub enum ParseError {
    Empty,
    LoneMinus,
    NotDigit,
}

pub fn parse_int(chars: &[u8]) -> Result<i64, ParseError> {
    let (sign, digits) = match chars {
        [] => return Err(ParseError::Empty),
        [b'-'] => return Err(ParseError::LoneMinus),
        [b'-', rest @ ..] => (-1, rest),
        _ => (1, chars),
    };
    let mut value: i64 = 0;
    for &c in digits {
        if !c.is_ascii_digit() {
            return Err(ParseError::NotDigit);
        }
        value = value * 10 + i64::from(c - b'0');
    }
    Ok(sign * value)
}
