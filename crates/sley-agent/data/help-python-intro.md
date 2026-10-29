Sley Python dialect: Python compiled to Sley; int is 64-bit and overflow traps; no floats.
Not supported: None, list values (append, slicing into a variable), dicts, sets (len(set(xs))
works), tuple values, methods, strings as sequences, classes other than records.
A class of annotated fields (`@dataclass`) declares a record type.
