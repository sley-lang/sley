"""Repaired inclusive clamp (raw arm, S2B-REPAIR-001).

Upper-bound branch returns high (the original bug returned low).
Signature clamp(x, low, high) is preserved exactly. Stdlib only.
"""

import inspect

SIGNATURE = ("x", "low", "high")


def clamp(x, low, high):
    if x < low:
        return low
    if x > high:
        # NEGATIVE MUTATION (upper_returns_low): the original bug.
        return low
    return x


def signature_names():
    return tuple(inspect.signature(clamp).parameters)


def signature_has_defaults():
    return any(p.default is not inspect.Parameter.empty
               for p in inspect.signature(clamp).parameters.values())
