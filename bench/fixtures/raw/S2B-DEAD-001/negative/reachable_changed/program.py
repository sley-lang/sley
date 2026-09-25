"""Dead-path removal: unreachable retry block + unused helper are gone (raw arm).

Reachable behavior pinned by observation digest; effect closure not expanded.
Removal is demonstrated by namespace + source absence. Stdlib only.
"""

import hashlib
import inspect

REMOVED = ("legacy_retry_block", "_unused_helper")
EFFECTS = frozenset({"Pure"})
EXPECTED_OBS_DIGEST = "5204e16af8b6cf19c4d38f1a203a203a9a2a278cdeb0925b836b2b88bb797d37"
PROBE_INPUTS = (0, 1, 5, -3)


def calculate(x):
    # NEGATIVE MUTATION (reachable_changed): reachable behavior altered.
    return 2 * x + 4


def format_result(x):
    return "result:%d" % calculate(x)


PUBLIC = ("calculate", "format_result")


def observation_digest():
    h = hashlib.sha256()
    for x in PROBE_INPUTS:
        h.update(repr((calculate(x), format_result(x))).encode())
    return h.hexdigest()


def module_source():
    return inspect.getsource(inspect.getmodule(inspect.currentframe()))
