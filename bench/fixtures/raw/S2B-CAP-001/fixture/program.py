"""Exact-object capability scope (raw arm, S2B-CAP-001).

Capability is bound to one canonical object (config/settings.bin).
Sibling reads are denied with stable code CAP_SCOPE_MISMATCH; wildcard
scopes are refused at construction. Stdlib only.
"""

ALLOWED = "config/settings.bin"
DENIED = "config/other.bin"
CODE_SCOPE_MISMATCH = "CAP_SCOPE_MISMATCH"
CODE_SCOPE_WIDENED = "ORACLE_SCOPE_WIDENED"

STORE = {ALLOWED: b"rate=725", DENIED: b"other"}


class CapScopeMismatch(Exception):
    def __init__(self, detail="capability scope mismatch"):
        super().__init__(detail)
        self.code = CODE_SCOPE_MISMATCH


class ScopeError(Exception):
    def __init__(self, code, detail):
        super().__init__(detail)
        self.code = code


class Capability:
    def __init__(self, scope):
        if "*" in scope:
            raise ScopeError(CODE_SCOPE_WIDENED,
                             "wildcard scope refused: " + scope)
        self.scope = scope


def read_with_cap(cap, path):
    if path != cap.scope:
        raise CapScopeMismatch(
            "CAP_SCOPE_MISMATCH: scope=%r path=%r" % (cap.scope, path))
    return STORE[path]
