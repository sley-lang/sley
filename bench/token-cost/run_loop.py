#!/usr/bin/env python3
"""Scripted agent loop, token-counted with real tokenizers.

For each task and form, the loop is what an agent sends and reads:
  write the program (with one deliberate bug) and its tests -> run -> read the
  failure -> apply a str_replace fix (old+new snippet) -> run -> read the pass.
Commands and their real outputs are recorded verbatim (transcripts/), then
counted with tiktoken o200k_base and cl100k_base. Usage:
  run_loop.py <sley-agent> <out-dir>
"""
import json, os, re, shutil, subprocess, sys, pathlib
import tiktoken
import spec

SLEY = os.path.abspath(sys.argv[1]); OUT = pathlib.Path(sys.argv[2]).resolve()
PY = os.environ.get("PYTHON", sys.executable)
TASKS = pathlib.Path(__file__).resolve().parent / "tasks"
ENC = {n: tiktoken.get_encoding(n) for n in ("o200k_base", "cl100k_base")}
FORMS = [f for f in os.environ.get("FORMS", "python sley_py sley_fam sley_af1").split()]
FILE = {"python": "{t}.py", "sley_py": "{t}.py", "sley_py_inline": "{t}.py", "sley_fam": "{t}.sley", "sley_af1": "{t}.json"}
SRC = {"python": "python.py", "sley_py": "sley.py", "sley_py_inline": "sley.py", "sley_fam": "sley.fam", "sley_af1": "sley.af1.json"}

def toks(text):
    return {n: len(e.encode(text)) for n, e in ENC.items()}

def py_value(v, task):
    if task == "records" and isinstance(v, dict):
        return f"Point({v['x']}, {v['y']})"
    return repr(v)

def python_tests(task):
    lines = ["import pytest", "", f"from {task} import {fn(task)}" + (", Point" if task == "records" else ""), "", "",
             f"def test_{task}():"]
    for args, want in spec.CASES[task]:
        call = f"{fn(task)}({', '.join(py_value(a, task) for a in args)})"
        if isinstance(want, tuple):
            lines += ["    with pytest.raises(ValueError):", f"        {call}"]
        else:
            lines.append(f"    assert {call} == {want!r}")
    return "\n".join(lines) + "\n"

def fn(task):
    return {"records": "manhattan", "errors": "withdraw"}.get(task, task)

def sley_rows(task, form):
    rows = []
    for args, want in spec.CASES[task]:
        if isinstance(want, tuple):
            want = {"trap": "unreachable"} if form == "sley_py" else {"Err": want[1]}
        elif task in ("parse_int", "errors") and form != "sley_py":
            want = {"Ok": want}
        rows.append({"args": args, "expect": want})
    return rows

def tests_frame(task, form):
    return {"af1": 1, "afx": 1, "test_tables": [{"name": f"{task}_cases", "fn": fn(task), "cases": sley_rows(task, form)}]}

def sh(cmd, cwd):
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    r = subprocess.run(cmd, cwd=cwd, shell=True, capture_output=True, text=True, env=env)
    return r.stdout + r.stderr, r.returncode

def run(task, form, kind, workdir):
    shutil.rmtree(workdir, ignore_errors=True); workdir.mkdir(parents=True)
    steps = []  # (role, label, text)   role: "write" (agent output) / "read" (agent input)
    def write(label, text): steps.append(("write", label, text))
    def cmd(c):
        out, code = sh(c, workdir)
        out = out.replace(str(workdir), "/work")
        steps.append(("write", "command", c)); steps.append(("read", "output", out))
        return code
    correct = (TASKS / task / SRC[form]).read_text()
    if form == "sley_py_inline":
        # The pytest test function, in the same file (the import of the module under test dropped).
        tests = python_tests(task).replace(f"from {task} import {fn(task)}" + (", Point" if task == "records" else "") + "\n\n", "")
        if "pytest.raises" not in tests:
            tests = tests.replace("import pytest\n\n", "")
        else:
            tests = tests.replace("import pytest\n\n", "")
            correct = "import pytest\n" + ("" if correct.startswith("from ") else "\n\n") + correct
        correct = correct.rstrip("\n") + "\n\n\n" + tests.lstrip("\n")
    if form == "sley_af1":
        frame = json.loads(correct); frame["test_tables"] = tests_frame(task, form)["test_tables"]
        correct = json.dumps(frame)
    if kind == "clean":
        old = new = ""
        buggy = correct
    else:
        old, new = spec.BUGS[task][kind]["sley_py" if form == "sley_py_inline" else form]
        assert correct.count(old) == 1, (task, form, kind, old)
        buggy = correct.replace(old, new)
    name = FILE[form].format(t=task)
    (workdir / name).write_text(buggy); write("program", buggy)
    def fix():
        (workdir / name).write_text(correct); write("edit", f"{new}\n{old}")
    def sley(c):
        code = cmd(c.replace("sley-agent", SLEY, 1)); steps[-2] = ("write", "command", c); return code
    def draft():
        return re.findall(r"draft (d\d+)@", steps[-1][2])[-1]
    if form == "python":
        tests = python_tests(task); (workdir / f"test_{task}.py").write_text(tests); write("tests", tests)
        c = f"python -m pytest -q test_{task}.py"
        run_cmd = c.replace("python ", f"{PY} ", 1)
        def go():
            out, code = sh(run_cmd, workdir)
            out = re.sub(r"in \d+\.\d+s", "in 0.01s", out).replace(str(workdir), "/work")
            steps.append(("write", "command", c)); steps.append(("read", "output", out)); return code
        first = go()
        if first:
            fix(); first2 = go()
        last = steps and (0 if steps[-1][2].strip().endswith("passed in 0.01s") else 1)
    elif form == "sley_py_inline":
        sh(f"{SLEY} init", workdir)
        first = sley(f"sley-agent try --familiar {name}")
        if first:
            fix(); sley(f"sley-agent try --familiar {name}")
        last = 0 if re.search(r"tests: (\d+)/\1 passed", steps[-1][2]) else 1
    elif form == "sley_af1":
        sh(f"{SLEY} init", workdir)
        first = sley(f"sley-agent try {name}")
        if first:
            fix(); sley(f"sley-agent try {name}")
        last = 0 if re.search(r"tests: (\d+)/\1 passed", steps[-1][2]) else 1
    else:
        sh(f"{SLEY} init", workdir)  # workspace setup, not part of the loop
        on = ""
        # The Python dialect lowers raise to a trap, so only records need a types frame there.
        if task in spec.TYPES and (form == "sley_fam" or task == "records"):
            types = json.dumps(json.loads((TASKS / task / "sley.types.json").read_text()))
            (workdir / "types.json").write_text(types); write("types", types)
            sley("sley-agent try types.json")
            on = f"--on {draft()} "
        first = sley(f"sley-agent try {on}--familiar {name}")
        if first:  # does not compile: fix the text, try it again
            fix(); sley(f"sley-agent try {on}--familiar {name}")
        d = draft()
        tests = json.dumps(tests_frame(task, form)); (workdir / "tests.json").write_text(tests); write("tests", tests)
        t = sley(f"sley-agent try --on {d} tests.json")
        first = first or t
        if t:  # a test failed: fix the text, layer it on the draft with the tests
            fix(); sley(f"sley-agent try --on {d} --familiar {name}")
        last = 0 if re.search(r"tests: (\d+)/\1 passed", steps[-1][2]) else 1
    return steps, first, last

def main():
    OUT.mkdir(parents=True, exist_ok=True)
    rows = []
    for task in spec.CASES:
        for kind in ("clean", "logic", "typo"):
            for form in FORMS:
                steps, first, last = run(task, form, kind, OUT / "work" / f"{task}-{kind}-{form}")
                tdir = OUT / "transcripts"; tdir.mkdir(exist_ok=True)
                (tdir / f"{task}-{kind}-{form}.json").write_text(json.dumps(steps, indent=1))
                row = {"task": task, "kind": kind, "form": form, "first_exit": first, "last_exit": last,
                       "tool_calls": sum(1 for s in steps if s[1] == "command")}
                for enc in ENC:
                    w = sum(toks(t)[enc] for r, l, t in steps if r == "write")
                    rd = sum(toks(t)[enc] for r, l, t in steps if r == "read")
                    prog = sum(toks(t)[enc] for r, l, t in steps if l in ("program", "types"))
                    rows.append(row | {"enc": enc, "write": w, "read": rd, "total": w + rd, "program": prog,
                                       "pass_output": toks([t for r, l, t in steps if l == "output"][-1])[enc]})
    (OUT / "loop.json").write_text(json.dumps(rows, indent=1))
    print(f"{len(rows)} rows -> {OUT/'loop.json'}")

if __name__ == "__main__":
    main()
