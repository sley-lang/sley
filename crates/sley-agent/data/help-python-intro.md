Sley Python dialect: Python compiled to Sley; int is 64-bit and overflow traps; no floats.
Not supported: None, list values beyond local lists (passing one, slicing one into a variable,
or using a list result inside the program), dicts, sets (len(set(xs)) works), tuple values, methods other than xs.append(v),
strings as sequences, classes other than records.
A class of annotated fields (`@dataclass`) declares a record type.
