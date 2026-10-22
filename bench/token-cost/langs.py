"""C, Rust, Go and TypeScript forms of the token benchmark: test files, bug
edits and the toolchain loop. Each language checks the same cases (spec.CASES);
error cases check the language's idiomatic error (C/Rust/Go: the specific code,
variant or sentinel; TypeScript: the thrown RangeError, as Python's pytest.raises)."""
import os, re
import spec

LANGS = ["c", "rust", "go", "ts"]
SRC = {"c": "c.c", "rust": "rust.rs", "go": "go.go", "ts": "ts.ts"}
FN = {
    "c": {"records": "manhattan", "errors": "withdraw"},
    "rust": {"records": "manhattan", "errors": "withdraw"},
    "go": {"records": "manhattan", "errors": "withdraw", "parse_int": "parseInt"},
    "ts": {"records": "manhattan", "errors": "withdraw", "parse_int": "parseInteger"},
}
def fn(task, lang): return FN[lang].get(task, task)
CERR = {"Empty": "PARSE_EMPTY", "LoneMinus": "PARSE_LONE_MINUS", "NotDigit": "PARSE_NOT_DIGIT",
        "NotPositive": "WITHDRAW_NOT_POSITIVE", "Insufficient": "WITHDRAW_INSUFFICIENT"}
CERR_OK = {"parse_int": "PARSE_OK", "errors": "WITHDRAW_OK"}
RERR = {"parse_int": "ParseError", "errors": "WithdrawError"}
RESULT = {"parse_int", "errors"}  # tasks whose function returns an error

def c_args(task, args):
    out = []
    for a in args:
        if isinstance(a, list):
            ty = "int" if task == "parse_int" else "long long"
            out.append(f"({ty}[]){{{', '.join(map(str, a))}}}, {len(a)}" if a else "NULL, 0")
        elif isinstance(a, dict):
            out.append(f"(struct point){{{a['x']}, {a['y']}}}")
        else:
            out.append(str(a))
    return ", ".join(out)

def rust_args(task, args):
    out = []
    for a in args:
        if isinstance(a, list): out.append(f"&[{', '.join(map(str, a))}]")
        elif isinstance(a, dict): out.append(f"&Point {{ x: {a['x']}, y: {a['y']} }}")
        else: out.append(str(a))
    return ", ".join(out)

def go_args(task, args):
    out = []
    for a in args:
        if isinstance(a, list):
            ty = "[]byte" if task == "parse_int" else "[]int64"
            out.append(f"{ty}{{{', '.join(map(str, a))}}}" if a or task == "parse_int" else "nil")
        elif isinstance(a, dict): out.append(f"Point{{{a['x']}, {a['y']}}}")
        else: out.append(str(a))
    return ", ".join(out)

def ts_args(task, args):
    out = []
    for a in args:
        if isinstance(a, list): out.append(f"[{', '.join(map(str, a))}]")
        elif isinstance(a, dict): out.append(f"{{ x: {a['x']}, y: {a['y']} }}")
        else: out.append(str(a))
    return ", ".join(out)

def tests(task, lang):
    """(file name, text) of the test file; Rust's is the test module appended to src/lib.rs."""
    f = fn(task, lang); cases = spec.CASES[task]
    if lang == "c":
        lines = ["#include <assert.h>", f'#include "{task}.c"', "", "int main(void) {"]
        if task in RESULT: lines.append("    long long v;")
        for args, want in cases:
            a = c_args(task, args)
            if task in RESULT:
                a += ", &v"
                if isinstance(want, tuple): lines.append(f"    assert({f}({a}) == {CERR[want[1]]});")
                else: lines.append(f"    assert({f}({a}) == {CERR_OK[task]} && v == {want});")
            else:
                lines.append(f"    assert({f}({a}) == {want});")
        lines += ["    return 0;", "}"]
        return f"test_{task}.c", "\n".join(lines) + "\n"
    if lang == "rust":
        lines = ["", "#[cfg(test)]", "mod tests {", "    use super::*;", "", "    #[test]", f"    fn test_{task}() {{"]
        for args, want in cases:
            call = f"{f}({rust_args(task, args)})"
            if task in RESULT:
                w = f"Err({RERR[task]}::{want[1]})" if isinstance(want, tuple) else f"Ok({want})"
            else: w = str(want)
            lines.append(f"        assert_eq!({call}, {w});")
        lines += ["    }", "}"]
        return "src/lib.rs", "\n".join(lines) + "\n"
    if lang == "go":
        errs = any(isinstance(w, tuple) for _, w in cases)
        name = "".join(p.capitalize() for p in task.split("_"))
        lines = ["package calc", "", 'import (\n\t"errors"\n\t"testing"\n)' if errs else 'import "testing"', "",
                 f"func Test{name}(t *testing.T) {{"]
        for args, want in cases:
            call = f"{f}({go_args(task, args)})"
            if isinstance(want, tuple):
                lines += [f"\tif _, err := {call}; !errors.Is(err, Err{want[1]}) {{",
                          f'\t\tt.Errorf("{call.replace(chr(34), "")} error = %v, want Err{want[1]}", err)', "\t}"]
            elif task in RESULT:
                lines += [f"\tif got, err := {call}; err != nil || got != {want} {{",
                          f'\t\tt.Errorf("{call} = %d, %v, want {want}", got, err)', "\t}"]
            else:
                lines += [f"\tif got := {call}; got != {want} {{", f'\t\tt.Errorf("{call} = %d, want {want}", got)', "\t}"]
        lines.append("}")
        return "calc_test.go", "\n".join(lines) + "\n"
    if lang == "ts":
        lines = ['import { test } from "node:test";', 'import assert from "node:assert/strict";',
                 f'import {{ {f} }} from "./{task}.js";', "", f'test("{f}", () => {{']
        for args, want in cases:
            call = f"{f}({ts_args(task, args)})"
            if isinstance(want, tuple): lines.append(f"  assert.throws(() => {call}, RangeError);")
            else: lines.append(f"  assert.equal({call}, {want});")
        lines.append("});")
        return f"{task}.test.ts", "\n".join(lines) + "\n"

PROG = {"c": "{t}.c", "rust": "src/lib.rs", "go": "calc.go", "ts": "{t}.ts"}

# Deliberate bugs, same edit as the other forms: (old, new).
BUGS = {
 "fib": {"logic": ("n < 2", "n < 1"), "typo": {"c": ("fib(n - 2)", "fib(m - 2)"), "rust": ("fib(n - 2)", "fib(m - 2)"),
          "go": ("fib(n-2)", "fib(m-2)"), "ts": ("fib(n - 2)", "fib(m - 2)")}},
 "loopsum": {"logic": {"go": ("i*i", "i+i"), "*": ("i * i", "i + i")}, "typo": {"go": ("s + i*i", "s + j*i"), "*": ("s + i * i", "s + j * i")}},
 "collatz": {"logic": {"go": ("best, bestN = steps, n", "best, bestN = steps, steps"), "ts": ("bestN = n", "bestN = steps"), "*": ("best_n = n", "best_n = steps")},
             "typo": ("steps > best", "steps > bets")},
 "primes": {"logic": {"go": ("d*d <= k", "d*d < k"), "*": ("d * d <= k", "d * d < k")},
            "typo": {"go": ("k%d == 0", "k%b == 0"), "*": ("k % d", "k % b")}},
 "gcdsum": {"logic": {"c": ("long long j = 1", "long long j = 0"), "rust": ("for j in 1..n", "for j in 0..n"),
                      "go": ("j := int64(1)", "j := int64(0)"), "ts": ("let j = 1", "let j = 0")},
            "typo": ("gcd(i, j)", "gcd(i, k)")},
 "parse_int": {"logic": {"c": ("(c - '0')", "(c - '/')"), "rust": ("(c - b'0')", "(c - b'/')"), "go": ("int64(c-'0')", "int64(c-'/')"), "ts": ("(c - 48)", "(c - 47)")},
               "typo": {"go": ("value*10", "valu*10"), "*": ("value * 10", "valu * 10")}},
 "transform": {"logic": {"c": ("xs[i] * 2", "xs[i] * 3"), "go": ("x *= 2", "x *= 3"), "*": ("x * 2", "x * 3")},
               "typo": {"c": ("x > hi)", "x > hl)"), "rust": (".min(hi)", ".min(hl)"), "go": ("lo), hi)", "lo), hl)"), "ts": ("lo), hi)", "lo), hl)")}},
 "records": {"logic": {"go": ("a.Y-b.Y", "a.Y-b.X"), "*": ("a.y - b.y", "a.y - b.x")},
             "typo": {"go": ("a.X-b.X", "a.X-c.X"), "*": ("a.x - b.x", "a.x - c.x")}},
 "errors": {"logic": ("amount > balance", "amount >= balance"), "typo": {"*": ("balance - amount", "balance - amout")}},
}
def bug(task, kind, lang):
    b = BUGS[task][kind]
    if isinstance(b, tuple): return b
    return b.get(lang, b.get("*"))

TYPES = os.path.join(os.path.abspath(os.environ.get("NODE_MODULES", "node_modules")), "@types")
TSCONFIG = ('{"compilerOptions": {"target": "es2022", "module": "nodenext", "strict": true, "outDir": "dist", "rootDir": ".", "types": ["node"], "typeRoots": ["@TYPES@"]}}').replace("@TYPES@", TYPES)
SETUP = {
    "c": [],
    "rust": ["cargo init --lib --name calc --vcs none -q"],
    "go": ["go mod init calc"],
    "ts": [f"echo '{TSCONFIG}' > tsconfig.json", "echo '{\"type\": \"module\"}' > package.json"],
}
def command(task, lang):
    return {"c": f"gcc -O2 -Wall -o test_{task} test_{task}.c && ./test_{task}",
            "rust": "cargo test -q", "go": "go test",
            "ts": "tsc && node --test --test-reporter=dot"}[lang]
def normalize(out):
    out = re.sub(r"finished in \d+\.\d+s", "finished in 0.00s", out)
    out = re.sub(r"\t\d+\.\d+s", "\t0.001s", out)
    out = re.sub(r"(thread '[^']*') \(\d+\)", r"\1 (1)", out)
    out = re.sub(r"\((\d+\.)?\d+ms\)", "(1ms)", out)
    out = re.sub(r"duration_ms \d+(\.\d+)?", "duration_ms 1", out)
    return out
