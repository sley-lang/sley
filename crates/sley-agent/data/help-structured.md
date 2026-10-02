# Structured function bodies
Give a function a "body" of statements instead of "blocks"; Sley builds the blocks, loops and state passing,
then compiles the result like any AF1-X function:
{"fn":"f","params":[["xs","Vec<i64>"],["k","i64"]],"returns":"i64","body":[...]}
Statements:
  ["let", x, e]                     bind x
  ["var", x, T, e]                  mutable variable of type T; ["set", x, e] assigns it
  ["if", c, [then...], [else...]]   (else optional)
  ["for", x, xs, [body...]]         each element of vector xs, in order
  ["while", c, [body...]]
  ["return", e]; in a Result function ["ok", e] or ["fail", "Case"]; ["trap"] aborts
Expressions: a name, an integer, true, false, {"type":"text","value":"hi"}, {"type":"u64","value":0}, or [op, args...]:
  add sub mul div rem neg abs   checked fixed-width arithmetic: overflow or division by zero traps.
                                add? instead returns the ArithmeticError from a Result function; add?Case returns Err(Case).
                                div truncates toward zero; rem takes the dividend's sign.
  to                            ["to", "i64", x] converts between integer types; a value outside the target fails
                                like a checked operation (to?, to?Case).
  min max clamp                 clamp(x, lo, hi) = min(max(x, lo), hi)
  eq ne lt le gt ge             eq compares text exactly
  not and or                    both operands of and/or are always evaluated
  if                            ["if", c, a, b]: only the chosen value is computed
  len (u64), get [xs, i] (u64 index), field [r, "name"], call ["helper", args...] (call? unwraps a Result)
Integer literals take the other operand's type, else i64. Integer types never mix: len and indexes are u64.
A refusal points into the body (/fns/0/body/2/1); `sley-agent view --after cN` shows the blocks Sley built.
Example:
{"af1":1,"afx":1,"fns":[{"fn":"product","params":[["xs","Vec<i64>"]],"returns":"Result<i64,ArithmeticError>","body":[
 ["var","p","i64",1],["for","x","xs",[["set","p",["mul?","p","x"]]]],["ok","p"]]}]}
