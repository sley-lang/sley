"""The six languages of the genuine model cost benchmark: names, signatures,
the visible tests a model's program is checked against, and the oracle driver
that runs the 128 hidden cases.

A model writes one solution file; the harness supplies the tests. Toolchains:
  PYTEST_PYTHON  a Python with pytest (default: this interpreter)
  SLEY_AGENT     the sley-agent binary (required for Sley)
  NODE_MODULES   a node_modules with typescript and @types/node (TypeScript)
gcc, rustc, go and node come from PATH.

Error convention (one line per hidden case, "ERR" for the task's error):
  Python  raises ValueError (any other exception prints EXC and fails the case)
  Sley    raise traps (the dialect's only error), printed as ERR
  C       returns non-zero from `int f(..., long long *out)`
  Rust    returns Err(_) from `Result<i64, String>` (a panic prints PANIC)
  Go      returns a non-nil error from `(int64, error)` (a panic prints PANIC)
  TS      throws (any thrown value)
"""
import json, os, re, shutil, subprocess, sys

import tasks

LANGS = ["python", "sley", "c", "rust", "go", "ts"]
SRC = {"python": "python.py", "sley": "sley.py", "c": "c.c", "rust": "rust.rs", "go": "go.go", "ts": "ts.ts"}
FENCE = {"python": "python", "sley": "python", "c": "c", "rust": "rust", "go": "go", "ts": "typescript"}
TIMEOUT = 60
FEEDBACK_CHARS = 4000


def camel(name, upper):
    parts = name.split("_")
    head = parts[0].capitalize() if upper else parts[0]
    return head + "".join(p.capitalize() for p in parts[1:])


def fn(task, lang):
    if lang == "go":
        return camel(task, True)
    if lang == "ts":
        return "parseInteger" if task == "parse_int" else camel(task, False)
    return task


def signature(task, lang):
    """The exact signature a model must implement."""
    t = tasks.BY_NAME[task]
    err = t["errors"]
    ps = t["params"]
    f = fn(task, lang)
    if lang in ("python", "sley"):
        a = ", ".join(f"{n}: {'list[int]' if k == 'list' else 'int'}" for n, k in ps)
        return f"def {f}({a}) -> int" + ("  (raise ValueError on error)" if err else "")
    if lang == "c":
        a = ", ".join(f"const long long *{n}, size_t {n}_len" if k == "list" else f"long long {n}" for n, k in ps)
        if err:
            return f"int {f}({a}, long long *out)  (store the result in *out and return 0; return 1 on error)"
        return f"long long {f}({a})"
    if lang == "rust":
        a = ", ".join(f"{n}: &[i64]" if k == "list" else f"{n}: i64" for n, k in ps)
        return f"pub fn {f}({a}) -> " + ("Result<i64, String>" if err else "i64")
    if lang == "go":
        a = ", ".join(f"{camel(n, False)} []int64" if k == "list" else f"{camel(n, False)} int64" for n, k in ps)
        return f"func {f}({a}) " + ("(int64, error)" if err else "int64") + "  (package solution)"
    if lang == "ts":
        a = ", ".join(f"{camel(n, False)}: number[]" if k == "list" else f"{camel(n, False)}: number" for n, k in ps)
        return f"export function {f}({a}): number" + ("  (throw an Error on error)" if err else "")
    raise KeyError(lang)


# ---- argument literals -------------------------------------------------------

def lit(lang, a):
    if isinstance(a, list):
        body = ", ".join(map(str, a))
        if lang == "c":
            return f"(const long long[]){{{body}}}, {len(a)}" if a else "NULL, 0"
        if lang == "rust":
            return f"&[{body}]" if a else "&[]"
        if lang == "go":
            return f"[]int64{{{body}}}"
        return f"[{body}]"
    if lang == "c" and abs(a) > 2 ** 31:
        return f"{a}LL"
    return str(a)


def call(task, lang, args):
    return f"{fn(task, lang)}({', '.join(lit(lang, a) for a in args)})"


# ---- visible tests -----------------------------------------------------------

def visible_tests(task, lang):
    """{file name: text} of the visible test files for a language."""
    t = tasks.BY_NAME[task]
    cases = t["visible"]
    f = fn(task, lang)
    if lang in ("python", "sley"):
        lines = []
        for args, want in cases:
            c = call(task, lang, args)
            if want == "error":
                lines += ["    with pytest.raises(ValueError):", f"        {c}"]
            else:
                lines.append(f"    assert {c} == {want}")
        body = f"def test_{task}():\n" + "\n".join(lines) + "\n"
        if lang == "python":
            return {f"test_{task}.py": f"import pytest\n\nfrom solution import {f}\n\n\n{body}"}
        return {"tests.py": body}  # appended to the program, after `import pytest`
    if lang == "c":
        lines = ["#include <assert.h>", "#include <stddef.h>", '#include "solution.c"', "", "int main(void) {"]
        if t["errors"]:
            lines.append("    long long v = 0;")
        for args, want in cases:
            c = call(task, lang, args)
            if t["errors"]:
                c = c[:-1] + (", &v)" if args else "&v)")
                lines.append(f"    assert({c} != 0);" if want == "error" else f"    assert({c} == 0 && v == {want});")
            else:
                lines.append(f"    assert({c} == {want});")
        lines += ["    return 0;", "}"]
        return {"test.c": "\n".join(lines) + "\n"}
    if lang == "rust":
        lines = ["", "#[cfg(test)]", "mod visible_tests {", "    use super::*;", "", "    #[test]", f"    fn test_{task}() {{"]
        for args, want in cases:
            c = call(task, lang, args)
            if t["errors"]:
                lines.append(f"        assert!({c}.is_err());" if want == "error" else f"        assert_eq!({c}, Ok({want}));")
            else:
                lines.append(f"        assert_eq!({c}, {want});")
        lines += ["    }", "}"]
        return {"tests.rs": "\n".join(lines) + "\n"}
    if lang == "go":
        lines = ["package solution", "", 'import "testing"', "", f"func Test{f}(t *testing.T) {{"]
        for args, want in cases:
            c = call(task, lang, args)
            q = c.replace('"', "")
            if want == "error":
                lines += [f"\tif _, err := {c}; err == nil {{", f'\t\tt.Errorf("{q}: want an error")', "\t}"]
            elif t["errors"]:
                lines += [f"\tif got, err := {c}; err != nil || got != {want} {{",
                          f'\t\tt.Errorf("{q} = %d, %v, want {want}", got, err)', "\t}"]
            else:
                lines += [f"\tif got := {c}; got != {want} {{", f'\t\tt.Errorf("{q} = %d, want {want}", got)', "\t}"]
        lines.append("}")
        return {"solution_test.go": "\n".join(lines) + "\n"}
    if lang == "ts":
        lines = ['import assert from "node:assert/strict";', f'import {{ {f} }} from "./solution.js";', ""]
        for args, want in cases:
            c = call(task, lang, args)
            lines.append(f"assert.throws(() => {c});" if want == "error" else f"assert.equal({c}, {want});")
        lines.append('console.log("all visible tests passed");')
        return {"test.ts": "\n".join(lines) + "\n"}
    raise KeyError(lang)


# ---- running -----------------------------------------------------------------

def sh(cmd, cwd, stdin=None):
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1", GOFLAGS="-mod=mod", GO111MODULE="on")
    try:
        r = subprocess.run(cmd, cwd=cwd, shell=True, capture_output=True, text=True, env=env,
                           timeout=TIMEOUT, input=stdin)
        return r.stdout + r.stderr, r.returncode
    except subprocess.TimeoutExpired as e:
        out = (e.stdout or b"").decode(errors="replace") if isinstance(e.stdout, bytes) else (e.stdout or "")
        return out + f"\n[timed out after {TIMEOUT}s]", 124


def sley_agent():
    s = os.environ.get("SLEY_AGENT")
    if not s:
        raise RuntimeError("set SLEY_AGENT to the sley-agent binary")
    return os.path.abspath(s)


def tsc():
    nm = os.path.abspath(os.environ.get("NODE_MODULES", "node_modules"))
    return os.path.join(nm, ".bin", "tsc"), os.path.join(nm, "@types")


def tsconfig():
    _, types = tsc()
    return json.dumps({"compilerOptions": {"target": "es2022", "module": "nodenext", "strict": True,
                                           "outDir": "dist", "rootDir": ".", "types": ["node"], "typeRoots": [types]}})


SLEY_PASSED = re.compile(r"^c\d+: \d+ passed", re.M)


def strip_python_tests(src):
    """The program without test functions or pytest imports (the oracle's build).
    Cuts the original lines (ast.unparse would rewrite `a, b = b, a` as a tuple)."""
    import ast
    tree = ast.parse(src)
    drop = set()
    for node in tree.body:
        test = isinstance(node, ast.FunctionDef) and node.name.startswith("test_")
        pyt = isinstance(node, (ast.Import, ast.ImportFrom)) and any(
            a.name == "pytest" for a in node.names) or (isinstance(node, ast.ImportFrom) and node.module == "pytest")
        if test or pyt:
            first = min([node.lineno] + [d.lineno for d in getattr(node, "decorator_list", [])])
            drop.update(range(first, node.end_lineno + 1))
    lines = src.splitlines()
    return "\n".join(l for i, l in enumerate(lines, 1) if i not in drop).rstrip() + "\n"


def fresh(d):
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)


def check(task, lang, source, workdir):
    """Run the visible tests on a program: (passed, feedback text)."""
    fresh(workdir)
    w = lambda name, text: open(os.path.join(workdir, name), "w").write(text)
    files = visible_tests(task, lang)
    py = os.environ.get("PYTEST_PYTHON", sys.executable)
    if lang == "python":
        w("solution.py", source)
        for n, s in files.items():
            w(n, s)
        out, code = sh(f"{py} -m pytest -q -p no:cacheprovider", workdir)
        return code == 0, out
    if lang == "sley":
        s = sley_agent()
        sh(f"{s} init", workdir)
        prog = source.rstrip() + "\n\n\n" + files["tests.py"]
        if not re.search(r"^import pytest", prog, re.M):
            prog = "import pytest\n\n" + prog
        w(f"{task}.py", prog)
        out, code = sh(f"{s} try --familiar {task}.py", workdir)
        return code == 0 and bool(SLEY_PASSED.search(out)) and "failed" not in out, out
    if lang == "c":
        w("solution.c", source)
        w("test.c", files["test.c"])
        out, code = sh("gcc -O2 -Wall -o test test.c && ./test", workdir)
        return code == 0, out
    if lang == "rust":
        w("lib.rs", source.rstrip() + "\n" + files["tests.rs"])
        out, code = sh("rustc --edition 2021 --test -O -o t lib.rs && ./t -q", workdir)
        return code == 0, out
    if lang == "go":
        w("go.mod", "module solution\n\ngo 1.22\n")
        w("solution.go", source)
        w("solution_test.go", files["solution_test.go"])
        out, code = sh("go test ./...", workdir)
        return code == 0, out
    if lang == "ts":
        exe, _ = tsc()
        w("tsconfig.json", tsconfig())
        w("package.json", '{"type": "module"}')
        w("solution.ts", source)
        w("test.ts", files["test.ts"])
        out, code = sh(f"{exe} && node dist/test.js", workdir)
        return code == 0, out
    raise KeyError(lang)


def feedback(text):
    text = text.replace(os.getcwd(), ".")
    if len(text) > FEEDBACK_CHARS:
        text = text[:FEEDBACK_CHARS] + f"\n[... {len(text) - FEEDBACK_CHARS} more characters truncated]"
    return text


# ---- the oracle --------------------------------------------------------------

def _driver(task, lang, cases):
    t = tasks.BY_NAME[task]
    f = fn(task, lang)
    if lang == "python":
        lines = [f"from solution import {f}", "CASES = " + json.dumps([a for a, _ in cases]),
                 "for args in CASES:", "    try:", f"        print({f}(*args))",
                 "    except ValueError:", "        print('ERR')", "    except Exception:", "        print('EXC')"]
        return {"driver.py": "\n".join(lines) + "\n"}
    if lang == "c":
        lines = ["#include <stdio.h>", "#include <stddef.h>", '#include "solution.c"', "", "int main(void) {",
                 "    long long v = 0;"]
        for args, _ in cases:
            c = call(task, lang, args)
            if t["errors"]:
                c = c[:-1] + (", &v)" if args else "&v)")
                lines.append(f'    if ({c} == 0) printf("%lld\\n", v); else puts("ERR");')
            else:
                lines.append(f'    printf("%lld\\n", (long long){c});')
        lines += ["    return 0;", "}"]
        return {"driver.c": "\n".join(lines) + "\n"}
    if lang == "rust":
        lines = ['#![allow(dead_code, unused)]', 'include!("solution.rs");', "", "fn main() {",
                 "    std::panic::set_hook(Box::new(|_| {}));"]
        for args, _ in cases:
            c = call(task, lang, args)
            if t["errors"]:
                lines.append(f'    match std::panic::catch_unwind(|| {c}) {{ Ok(Ok(v)) => println!("{{}}", v), Ok(Err(_)) => println!("ERR"), Err(_) => println!("PANIC") }}')
            else:
                lines.append(f'    match std::panic::catch_unwind(|| {c}) {{ Ok(v) => println!("{{}}", v), Err(_) => println!("PANIC") }}')
        lines.append("}")
        return {"driver.rs": "\n".join(lines) + "\n"}
    if lang == "go":
        lines = ["package main", "", 'import (\n\t"fmt"\n\t"oracle/solution"\n)', "",
                 "func run(f func() (int64, error)) {",
                 '\tdefer func() {\n\t\tif recover() != nil {\n\t\t\tfmt.Println("PANIC")\n\t\t}\n\t}()',
                 '\tv, err := f()\n\tif err != nil {\n\t\tfmt.Println("ERR")\n\t\treturn\n\t}\n\tfmt.Println(v)', "}", "",
                 "func main() {"]
        for args, _ in cases:
            c = "solution." + call(task, lang, args)
            if t["errors"]:
                lines.append(f"\trun(func() (int64, error) {{ return {c} }})")
            else:
                lines.append(f"\trun(func() (int64, error) {{ return {c}, nil }})")
        lines.append("}")
        return {"main.go": "\n".join(lines) + "\n"}
    if lang == "ts":
        lines = [f'import {{ {f} }} from "./solution.js";', "const CASES: number[][][] | any[] = " + json.dumps([a for a, _ in cases]) + ";",
                 "for (const args of CASES) {", "  try {", f"    console.log(String(({f} as any)(...args)));",
                 "  } catch {", '    console.log("ERR");', "  }", "}"]
        return {"driver.ts": "\n".join(lines) + "\n"}
    raise KeyError(lang)


def run_cases(task, lang, source, workdir, cases):
    """Outputs of the program on the hidden cases (one string per case), or None and the build log."""
    fresh(workdir)
    w = lambda name, text: (os.makedirs(os.path.dirname(os.path.join(workdir, name)) or workdir, exist_ok=True),
                            open(os.path.join(workdir, name), "w").write(text))
    py = os.environ.get("PYTEST_PYTHON", sys.executable)
    if lang == "sley":
        s = sley_agent()
        sh(f"{s} init", workdir)
        try:
            prog = strip_python_tests(source)
        except SyntaxError as e:
            return None, f"syntax error: {e}"
        w(f"{task}.py", prog)
        out, _ = sh(f"{s} try --familiar {task}.py", workdir)
        m = re.findall(r"draft (d\d+)@r\d+", out)
        if not m:
            return None, out
        out2, code = sh(f"{s} submit {m[-1]} --untested && {s} commit", workdir)
        if code:
            return None, out + out2
        results = []
        for args, _ in cases:
            argv = " ".join("'" + json.dumps(a, separators=(",", ":")) + "'" for a in args)
            o, code = sh(f"{s} --json call {fn(task, lang)} {argv}", workdir)
            try:
                r = json.loads(o.strip().splitlines()[-1])["result"]
                results.append("ERR" if isinstance(r, dict) and "trap" in r else str(r))
            except Exception:
                results.append("EXC")
        return results, ""
    files = _driver(task, lang, cases)
    for n, s in files.items():
        w(n, s)
    if lang == "python":
        w("solution.py", source)
        out, code = sh(f"{py} driver.py", workdir)
    elif lang == "c":
        w("solution.c", source)
        out, code = sh("gcc -O2 -w -o driver driver.c && ./driver", workdir)
    elif lang == "rust":
        w("solution.rs", source)
        out, code = sh("rustc --edition 2021 -O -A warnings -o driver driver.rs && ./driver", workdir)
    elif lang == "go":
        w("go.mod", "module oracle\n\ngo 1.22\n")
        w("solution/solution.go", source)
        out, code = sh("go run .", workdir)
    elif lang == "ts":
        exe, _ = tsc()
        w("tsconfig.json", tsconfig())
        w("package.json", '{"type": "module"}')
        w("solution.ts", source)
        out, code = sh(f"{exe} && node dist/driver.js", workdir)
    else:
        raise KeyError(lang)
    lines = out.strip().splitlines()
    if code or len(lines) != len(cases):
        return None, out
    return lines, ""


def oracle(task, lang, source, workdir, count=128):
    """Score a final program on the hidden cases: dict(passed, total, first_failure, build_log)."""
    cases = tasks.cases(task, count)
    got, log = run_cases(task, lang, source, workdir, cases)
    if got is None:
        return {"passed": 0, "total": len(cases), "first_failure": None, "build_log": feedback(log)}
    want = ["ERR" if w == "error" else str(w) for _, w in cases]
    bad = [i for i, (g, x) in enumerate(zip(got, want)) if g != x]
    first = None
    if bad:
        i = bad[0]
        first = {"args": cases[i][0], "want": want[i], "got": got[i]}
    return {"passed": len(cases) - len(bad), "total": len(cases), "first_failure": first, "build_log": ""}
