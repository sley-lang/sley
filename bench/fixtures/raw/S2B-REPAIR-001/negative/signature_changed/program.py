"""Repaired inclusive clamp (raw arm, S2B-REPAIR-001).

Upper-bound branch returns high (the original bug returned low).
Signature clamp(x, low, high) is preserved exactly. Stdlib only.
"""

import inspect

SIGNATURE = ("x", "low", "high")


def clamp(x, low, high, mode="inclusive"):
    # NEGATIVE MUTATION (signature_changed): extra parameter added.
    if x < low:
        return low
    if x > high:
        return high
    return x


def signature_names():
    return tuple(inspect.signature(clamp).parameters)


def signature_has_defaults():
    return any(p.default is not inspect.Parameter.empty
               for p in inspect.signature(clamp).parameters.values())
