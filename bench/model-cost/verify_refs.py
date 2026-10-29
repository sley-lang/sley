#!/usr/bin/env python3
"""Check the reference programs: each passes its visible tests and all 128
hidden cases in every language, and each edit task's starter fails its
visible tests. No model is called. Usage: verify_refs.py [task ...]"""
import concurrent.futures, os, sys, tempfile

import langs, tasks

HERE = os.path.dirname(os.path.abspath(__file__))


def one(task, lang, root):
    src = open(os.path.join(HERE, "refs", task, langs.SRC[lang])).read()
    ok, out = langs.check(task, lang, src, os.path.join(root, f"{task}-{lang}-v"))
    o = langs.oracle(task, lang, src, os.path.join(root, f"{task}-{lang}-o"))
    starter = None
    if tasks.BY_NAME[task].get("edit"):
        s = open(os.path.join(HERE, "starters", task, langs.SRC[lang])).read()
        starter, _ = langs.check(task, lang, s, os.path.join(root, f"{task}-{lang}-s"))
    good = ok and o["passed"] == o["total"] and starter in (None, False)
    note = "" if good else f" visible={ok} oracle={o['passed']}/{o['total']} {o['first_failure']} starter_passes={starter}\n{out[-800:]}\n{o['build_log'][-800:]}"
    return good, f"{'ok  ' if good else 'FAIL'} {task:16} {lang:6} {o['passed']}/{o['total']}{note}"


def main():
    names = sys.argv[1:] or [t["name"] for t in tasks.TASKS]
    tasks.check_visible()
    with tempfile.TemporaryDirectory() as root, concurrent.futures.ThreadPoolExecutor(int(os.environ.get("JOBS", "4"))) as ex:
        futs = [ex.submit(one, t, l, root) for t in names for l in langs.LANGS]
        results = [f.result() for f in futs]
    for _, line in results:
        print(line)
    bad = sum(1 for g, _ in results if not g)
    print(f"{len(results) - bad}/{len(results)} reference programs agree with the oracle")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
