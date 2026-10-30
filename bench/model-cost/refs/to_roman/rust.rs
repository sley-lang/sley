pub fn to_roman(n: i64) -> Result<String, String> {
    if n < 1 || n > 3999 {
        return Err("n must be within [1, 3999]".to_string());
    }
    const TABLE: [(i64, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut rest = n;
    let mut out = String::new();
    for &(value, symbol) in TABLE.iter() {
        while rest >= value {
            out.push_str(symbol);
            rest -= value;
        }
    }
    Ok(out)
}
