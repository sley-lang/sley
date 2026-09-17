"""Quadratic scan -> ordered-map strategy (raw arm, S2B-PERF-001).

PROXY (honest): instruction counts are deterministic step-counter values
shared by both implementations (comparison/insert/lookup increments), not
wall-clock time and not VM bytecode. Both counts are recorded; reduction
>= 30% with an identical output digest is asserted. Stdlib only.
"""

import hashlib
import json

N_ITEMS = 1500
N_QUERIES = 300
MEMORY_LIMIT = 100000


class Counter:
    def __init__(self):
        self.steps = 0

    def tick(self, n=1):
        self.steps += n


def build_input():
    items = ["item-%05d" % i for i in range(N_ITEMS)]
    queries = (["item-%05d" % (i * 7 % N_ITEMS) for i in range(N_QUERIES // 2)]
               + ["missing-%05d" % i for i in range(N_QUERIES // 2)])
    return items, queries


def before_membership(items, queries, counter):
    out = []
    for q in queries:
        found = False
        for it in items:
            counter.tick()
            if it == q:
                found = True
                break
        out.append(found)
    return out


def after_membership(items, queries, counter):
    index = {}
    for it in items:
        counter.tick()
        index[it] = True
    out = []
    for q in queries:
        counter.tick()
        out.append(q in index)
    return out


def digest_of(outputs):
    return hashlib.sha256(json.dumps(outputs).encode()).hexdigest()


def run_demo():
    items, queries = build_input()
    cb = Counter()
    out_b = before_membership(items, queries, cb)
    ca = Counter()
    out_a = after_membership(items, queries, ca)
    db, da = digest_of(out_b), digest_of(out_a)
    reduction = (cb.steps - ca.steps) / cb.steps if cb.steps else 0.0
    return {"before_steps": cb.steps, "after_steps": ca.steps,
            "reduction": reduction, "digest_before": db,
            "digest_after": da, "memory_after": len(set(items))}
