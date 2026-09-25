# Types

Types are written as short strings from a fixed table, or as one-key
objects (`{"Result": ["i64", "MathError"]}`).

    unit bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 bytes text
    Result<T,E>   Option<T>   Vec<T>   Map<K,V>   Cell<T>   (A,B,...)
    TypeName      TypeName<T,...>      fn(A,B)->R
    ArithmeticError  IndexError  DuplicateKeyError  ContractViolation  CapabilityFailure

Integers never convert implicitly: both operands of `add` or `lt` must have
the same type. Checked arithmetic returns `Result<T,ArithmeticError>`, whose
error cases are Overflow, DivideByZero and InvalidShift.

Values in `call`, `tests` and `consts` follow the declared type:

    i64 / u8 / ...    5        (strings like "170141183460469231731687303715884105727" for i128)
    bool              true
    unit              null
    text              "hi"
    bytes             "0x00ff"
    (A,B) / Vec<T>    [a, b]
    Option<T>         "None" | {"Some": v}
    Result<T,E>       {"Ok": v} | {"Err": e}
    variant type      "Case" | {"Case": payload}
    ArithmeticError   "Overflow" | {"ArithmeticError": "Overflow"}   (and the other failures)
    record type       {"field": v, ...}
