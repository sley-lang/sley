"""Explicit FileRead effect (raw arm, S2B-EFFECT-001).

The file-read capability is an explicit declared parameter, never ambient.
load_config declares FileRead; caller closure includes FileRead; pure sibling
sum_lines declares nothing. Replayed execution digest pinned. Stdlib only.
"""

import hashlib
import inspect

EFFECT_FILEREAD = "FileRead"

STORE = {"cfg/title": "invoice", "cfg/rate": "725"}
EXPECTED_REPLAY_DIGEST = "78fde71a0f8cf7f91cb3d54001b04d8e060da5042de65ea8a59e52025d68772e"


class FileRead:
    EFFECT = EFFECT_FILEREAD

    def __init__(self):
        self.log = []

    def read(self, path):
        self.log.append(path)
        return STORE[path]


def _direct_store_read(path):
    STORE["__ambient_reads__"] = STORE.get("__ambient_reads__", 0) + 1
    return STORE[path]


def ambient_read_count():
    return STORE.get("__ambient_reads__", 0)


def load_config(read_effect):
    # NEGATIVE MUTATION (ambient_read): ignores the declared effect and
    # reads through a hidden ambient channel instead.
    _AMBIENT = {"cfg/title": STORE["cfg/title"], "cfg/rate": STORE["cfg/rate"]}
    _direct_store_read("cfg/title")
    _direct_store_read("cfg/rate")
    return {"title": _AMBIENT["cfg/title"],
            "rate_bps": int(_AMBIENT["cfg/rate"])}


load_config.EFFECTS = frozenset({EFFECT_FILEREAD})


def get_invoice_total(read_effect):
    cfg = load_config(read_effect)
    lines = [(2, 1250)]
    return {"total_cents": sum_lines(lines) +
            sum_lines(lines) * cfg["rate_bps"] // 10000,
            "title": cfg["title"]}


get_invoice_total.EFFECTS = frozenset({EFFECT_FILEREAD})


def sum_lines(lines):
    return sum(q * u for q, u in lines)


sum_lines.EFFECTS = frozenset()


def replay_digest():
    eff = FileRead()
    out = get_invoice_total(eff)
    return hashlib.sha256(repr(sorted(out.items())).encode()).hexdigest()


def module_source():
    return inspect.getsource(inspect.getmodule(inspect.currentframe()))
