Sley familiar syntax: Rust-like functions over fixed-width checked integers.
fn f(n: i64, xs: Vec<i64>, s: text) -> Result<i64,ArithmeticError> { ... }
let x = e   var t = e   t = e   t += e   (only a var changes; -= *= /= %= too)
if c { } else if c { } else { }   while c { }
for x in xs { }   for i in a..b { }   for i, x in xs { }
return e, or end with a bare value; a Result fn returns ok(e) or err(Case) (Ok/Err too); trap aborts
+ - * / %: overflow or /0 traps; try(e) returns that failure as the fn's error, try(e, Case) as err(Case)
== != < <= > >=  and or not  if c { a } else { b } or a if c else b  min max abs clamp
sum(e for x in xs if c)  count(c for x in xs)  any(..)  all(..)  max(..)  min(..)
len(xs) and indexes are u64; to(T, x) converts integers; types never mix, literals adapt
"text" == compares exactly; r.field
break and continue leave or advance the innermost loop.
No floats (f32/f64 are AF1-frame types: help types), methods, slices or tuples.
