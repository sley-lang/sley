# Familiar syntax (optional authoring frontend)
`sley-agent try --familiar <file | ->` parses this text into structured function bodies (`help structured`)
and takes the ordinary path; the program is stored as Sley's graph, never as this text.
`--body NAME --base-root ROOT` accepts statements only, using that live function's signature.
fn name(x: i64, xs: Vec<i64>) -> i64 { statements }     (one or more fns; newlines or ; separate)
  let x = e          var t: T = e          t = e   (t is a var)
  if c { } else if c { } else { }          for x in xs { }          while c { }
  for i in start..end { } (half-open; bounds once)     for i, x in xs { } (i is u64)
  return e;  in a Result function: return ok(e) or return err(Case);  trap aborts
Expressions:
  + - * / % and unary -: checked fixed-width integers; overflow or division by zero traps.
    / truncates toward zero; % takes the dividend's sign.
    try(e) returns e's arithmetic failure as the function's ArithmeticError; try(e, Case) returns err(Case).
  to(T, x) converts between integer types; a value outside T fails like a checked operation.
  == != < <= > >= (no chaining)   and or not (both sides of and/or are always evaluated)
  a if c else b (on one line; only the chosen side runs)
  min(a, b) max(a, b) abs(x) clamp(x, lo, hi) = min(max(x, lo), hi)
  len(xs) is u64; xs[i] takes a u64 index; record.field; helper(args); "text" (== compares exactly); 7u64 typed literal
Types: i8..i128, u8..u128, bool, text, Vec<T>, Result<T,E>, records. Integer types never mix; a literal takes
the other operand's type, else i64. A nested block cannot redeclare a name declared outside it.
Comments: # to end of line, or a line starting with //. Other constructs (methods, inclusive ranges, break, +=, floats,
slices, tuples) are refused with their line and column. This is Sley, not Python: fixed-width integers,
no implicit conversions.
Example:
fn product(xs: Vec<i64>) -> Result<i64,ArithmeticError> {
    var p: i64 = 1
    for x in xs { p = try(p * x) }
    return ok(p)
}
