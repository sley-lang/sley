Sley Python dialect: Python syntax that `sley-agent try --familiar` compiles to Sley.
int is 64-bit (overflow traps); bool, str, list[T], records. No floats: // and % floor; / is refused.
Has: def with annotations, if/elif/else, for over a list, range or enumerate, while, return,
= += -= *= //= %=, assert, raise (traps), in, chained comparisons, x if c else y,
len min max abs, sum any all over a list, generator or list comprehension.
Not supported: break, continue, slices, list values, dicts, tuples, classes, methods.
