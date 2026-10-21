#!/usr/bin/env python3
"""Token tables: static program sizes, guide sizes, read-back views, agent-loop totals.
Usage: summarize.py <sley-agent> <results-dir>  (results-dir holds loop.json from run_loop.py)"""
import json, pathlib, subprocess, sys, tempfile, statistics
import tiktoken, spec, langs
SLEY, RES = sys.argv[1], pathlib.Path(sys.argv[2])
T = pathlib.Path(__file__).resolve().parent / "tasks"
ENCS = ["o200k_base", "cl100k_base"]
E = {n: tiktoken.get_encoding(n) for n in ENCS}
def n(text, enc): return len(E[enc].encode(text))
import os
FORMS = ["python", "sley_py", "sley_fam", "sley_af1"]
# Whether this binary's Python dialect needs a types frame for records (2.1.3: yes; record classes after).
PY_TYPES = os.environ.get("PY_TYPES", "1") == "1"
def needs_types(form, task):
    return task in spec.TYPES and (form == "sley_fam" or (form == "sley_py" and task == "records" and PY_TYPES))
SRC = {"python": "python.py", "sley_py": "sley.py", "sley_fam": "sley.fam", "sley_af1": "sley.af1.json"}
def declarations(task):
    """The task's types as familiar struct/enum declarations (2.1.5)."""
    if task not in spec.TYPES: return ""
    out = []
    for t in json.loads((T / task / "sley.types.json").read_text())["types"]:
        if "record" in t: out.append(f"struct {t['name']} {{ " + ", ".join(f"{f}: {ty}" for f, ty in t["record"]) + " }")
        else: out.append(f"enum {t['name']} {{ " + ", ".join(t["variant"]) + " }")
    return "\n".join(out) + "\n\n"
DECL = os.environ.get("DECL", "0") == "1"
out = []
def p(s=""): out.append(s)
def run(args, cwd=None):
    r = subprocess.run([SLEY] + args, capture_output=True, text=True, cwd=cwd); return r.stdout + r.stderr

# 1. static program size
p("## Program text (correct program only, no tests)")
for enc in ENCS:
    p(f"\n### {enc}\n")
    p("| task | Python | Sley Python dialect | Sley familiar | Sley AF1-X JSON | dialect/Py | familiar/Py | AF1/Py |")
    p("|---|---|---|---|---|---|---|---|")
    tot = {f: 0 for f in FORMS}
    for task in spec.CASES:
        row = {}
        for f in FORMS:
            text = (T / task / SRC[f]).read_text()
            if f == "sley_af1": text = json.dumps(json.loads(text))
            k = n(text, enc)
            if needs_types(f, task):
                k += n(json.dumps(json.loads((T / task / "sley.types.json").read_text())), enc)
            row[f] = k; tot[f] += k
        p(f"| {task} | {row['python']} | {row['sley_py']}{'*' if needs_types('sley_py', task) else ''} | {row['sley_fam']}{'*' if needs_types('sley_fam', task) else ''} | {row['sley_af1']} | "
          f"{row['sley_py']/row['python']:.2f} | {row['sley_fam']/row['python']:.2f} | {row['sley_af1']/row['python']:.2f} |")
    p(f"| **total** | {tot['python']} | {tot['sley_py']} | {tot['sley_fam']} | {tot['sley_af1']} | {tot['sley_py']/tot['python']:.2f} | {tot['sley_fam']/tot['python']:.2f} | {tot['sley_af1']/tot['python']:.2f} |")
p("\n\\* includes the AF1 `types` frame the form needs (familiar syntax has no type declarations; the 2.1.3 Python dialect skips classes, so records need one there; the dialect lowers raise to a trap and needs none for errors).")

# 1b. program text in other languages
p("\n## Program text vs other languages (correct program only, no tests)")
for enc in ENCS:
    p(f"\n### {enc}\n")
    cols = ["python", "sley_py"] + (["sley_fam_decl"] if DECL else ["sley_fam"]) + ["c", "rust", "go", "ts"]
    p("| task | " + " | ".join(cols) + " |"); p("|---|" + "---|" * len(cols))
    tot = {c: 0 for c in cols}
    for task in spec.CASES:
        row = {}
        for c in cols:
            src = langs.SRC.get(c) or SRC.get(c, "sley.fam")
            text = (T / task / src).read_text()
            if c == "sley_fam_decl": text = declarations(task) + text
            k = n(text, enc)
            if needs_types(c, task): k += n(json.dumps(json.loads((T / task / "sley.types.json").read_text())), enc)
            row[c] = k; tot[c] += k
        p(f"| {task} | " + " | ".join(str(row[c]) for c in cols) + " |")
    p("| **total** | " + " | ".join(f"{tot[c]} ({tot[c]/tot['python']:.2f}x)" for c in cols) + " |")

# 2. guides
p("\n## One-time guide an agent reads to write each form")
p("| form | guide | o200k | cl100k |"); p("|---|---|---|---|")
for form, topics in [("sley_py", ["python"]), ("sley_fam", ["card"]), ("sley_af1", ["", "structured"])]:
    text = "".join(run(["help"] + ([t] if t else [])) for t in topics)
    p(f"| {form} | `help {' + help '.join(t or '(main)' for t in topics)}` | {n(text,'o200k_base')} | {n(text,'cl100k_base')} |")
p("| python | none (models already know Python) | 0 | 0 |")

# 3. read-back view of the compiled program
p("\n## Reading a program back (`sley-agent view <fn>` after the Python-dialect program, vs the Python source)")
p("| task | Python source | AV1 view | view/source |"); p("|---|---|---|---|")
for task in spec.CASES:
    with tempfile.TemporaryDirectory() as d:
        run(["init"], d)
        if needs_types("sley_py", task): run(["try", str(T / task / "sley.types.json")], d); on = ["--on", "d1"]
        else: on = []
        r = run(["try"] + on + ["--familiar", str(T / task / "sley.py")], d)
        c = r.split(":")[0]
        fn = {"records": "manhattan", "errors": "withdraw"}.get(task, task)
        v = run(["view", fn, "--after", c], d)
    src = (T / task / "python.py").read_text()
    p(f"| {task} | {n(src,'o200k_base')} | {n(v,'o200k_base')} | {n(v,'o200k_base')/n(src,'o200k_base'):.1f}x |")

# 4. agent loop
rows = json.loads((RES / "loop.json").read_text())
FORMS = [f for f in ["python", "sley_py", "sley_py_inline", "sley_fam", "sley_fam_decl", "sley_af1", "c", "rust", "go", "ts"] if any(r["form"] == f for r in rows)]
for enc in ENCS:
    p(f"\n## Agent loop totals ({enc}): write program+tests, run, read, fix, run, read")
    p("| scenario | form | tokens written | tokens read | total | vs Python | tool calls |"); p("|---|---|---|---|---|---|---|")
    for kind in ("clean", "logic", "typo"):
        base = None
        for f in FORMS:
            sel = [r for r in rows if r["enc"] == enc and r["kind"] == kind and r["form"] == f]
            w = sum(r["write"] for r in sel); rd = sum(r["read"] for r in sel); calls = sum(r["tool_calls"] for r in sel)
            if f == "python": base = w + rd
            p(f"| {kind} | {f} | {w} | {rd} | {w+rd} | {(w+rd)/base:.2f}x | {calls} |")
    p(f"\n### Per task, logic-bug loop ({enc}, total tokens)")
    p("| task | " + " | ".join(FORMS) + " |"); p("|---|" + "---|" * len(FORMS))
    for task in spec.CASES:
        vals = {f: next(r["total"] for r in rows if r["enc"] == enc and r["kind"] == "logic" and r["form"] == f and r["task"] == task) for f in FORMS}
        p(f"| {task} | " + " | ".join(f"{vals[f]}" + ("" if f == "python" else f" ({vals[f]/vals['python']:.2f}x)") for f in FORMS) + " |")
(RES / "SUMMARY.md").write_text("\n".join(out) + "\n")
print("\n".join(out))
