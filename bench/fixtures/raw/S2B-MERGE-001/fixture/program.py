"""Disjoint branch merge (raw arm, S2B-MERGE-001).

Disjoint branches merge with both changes preserved and a deterministic
combined root; left/right order is semantically commutative. Overlapping
changes yield a Conflict object, never silence. Stdlib only.
"""

import hashlib
import json

BASE = {"checksum": "fnv1", "invoice_rounding": "floor", "retries": "3"}
LEFT = {"invoice_rounding": "floor_v2"}   # branch A: rounding tests
RIGHT = {"checksum": "fnv1a64"}           # branch B: checksum impl


class Conflict:
    def __init__(self, entities):
        self.entities = sorted(entities)
        self.count = len(self.entities)

    def __repr__(self):
        return "Conflict(entities=%r, count=%d)" % (self.entities,
                                                    self.count)


def merge(base, left, right):
    changed_l = {k for k, v in left.items() if base.get(k) != v}
    changed_r = {k for k, v in right.items() if base.get(k) != v}
    overlap = {k for k in changed_l & changed_r
               if left[k] != right[k]}
    if overlap:
        return None, Conflict(overlap)
    combined = dict(base)
    combined.update(left)
    combined.update(right)
    return combined, Conflict([])


def root_digest(combined):
    return hashlib.sha256(
        json.dumps(combined, sort_keys=True).encode()).hexdigest()
