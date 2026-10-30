#!/usr/bin/env python3
"""Check the reference programs: each passes its visible tests and all 128
hidden cases in every language, and each edit task's starter fails its
visible tests. No model is called.

Usage: verify_refs.py [--task-set v1|v2] [task ...]

A `refs/<task>/sley.unsupported` file (v2) says why Sley's dialect cannot
express a task yet; it is reported as `n/a`, not as a pass."""
import concurrent.futures, os, sys, tempfile

import langs, tasks, tasks_v2

HERE = os.path.dirname(os.path.abspath(__file__))
SETS = {"v1": tasks, "v2": tasks_v2}


def unsupported(task, lang):
    p = os.path.join(HERE, "refs", task, f"{lang}.unsupported")
    return open(p).read().strip() if os.path.exists(p) else None


def one(task, lang, root):
    why = unsupported(task, lang)
    if why:
        return None, f"n/a  {task:24} {lang:6} {why.splitlines()[0]}"
    src = open(os.path.join(HERE, "refs", task, langs.SRC[lang])).read()
    ok, out = langs.check(task, lang, src, os.path.join(root, f"{task}-{lang}-v"))
    o = langs.oracle(task, lang, src, os.path.join(root, f"{task}-{lang}-o"))
    starter = None
    if langs.task_def(task).get("edit"):
        s = open(os.path.join(HERE, "starters", task, langs.SRC[lang])).read()
        starter, _ = langs.check(task, lang, s, os.path.join(root, f"{task}-{lang}-s"))
    good = ok and o["passed"] == o["total"] and starter in (None, False)
    note = "" if good else f" visible={ok} oracle={o['passed']}/{o['total']} {o['first_failure']} starter_passes={starter}\n{out[-800:]}\n{o['build_log'][-800:]}"
    return good, f"{'ok  ' if good else 'FAIL'} {task:24} {lang:6} {o['passed']}/{o['total']}{note}"


def main():
    args = sys.argv[1:]
    task_set = "v1"
    if args[:1] == ["--task-set"]:
        task_set, args = args[1], args[2:]
    ts = SETS[task_set]
    names = args or [t["name"] for t in ts.TASKS]
    bad_examples = ts.check_visible()
    if bad_examples:
        print(f"examples that disagree with the oracle: {bad_examples}")
    langs_ = [l for l in os.environ.get("LANGS", ",".join(langs.LANGS)).split(",") if l]
    with tempfile.TemporaryDirectory() as root, concurrent.futures.ThreadPoolExecutor(int(os.environ.get("JOBS", "4"))) as ex:
        futs = [ex.submit(one, t, l, root) for t in names for l in langs_]
        results = [f.result() for f in futs]
    for _, line in results:
        print(line)
    na = sum(1 for g, _ in results if g is None)
    bad = sum(1 for g, _ in results if g is False)
    print(f"{len(results) - bad - na}/{len(results) - na} reference programs agree with the oracle"
          + (f"; {na} not expressible (n/a)" if na else ""))
    sys.exit(1 if bad or bad_examples else 0)


if __name__ == "__main__":
    main()
