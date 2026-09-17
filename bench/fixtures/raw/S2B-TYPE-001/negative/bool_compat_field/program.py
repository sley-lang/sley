"""Tagged JobState replacing a boolean status (raw arm, S2B-TYPE-001).

Four exhaustive variants; Failed carries an explicit error code;
serialization is deterministic. No boolean status binding. Stdlib only.
"""


class Queued:
    kind = "Queued"

    def __eq__(self, o):
        return isinstance(o, Queued)

    def __repr__(self):
        return "Queued()"


class Running:
    kind = "Running"

    def __init__(self):
        # NEGATIVE MUTATION (bool_compat_field): parallel boolean retained.
        self.running = True

    def __eq__(self, o):
        return isinstance(o, Running)

    def __repr__(self):
        return "Running()"


class Succeeded:
    kind = "Succeeded"

    def __eq__(self, o):
        return isinstance(o, Succeeded)

    def __repr__(self):
        return "Succeeded()"


class Failed:
    kind = "Failed"

    def __init__(self, error_code):
        if not isinstance(error_code, str) or not error_code:
            raise ValueError("Failed requires an explicit non-empty error_code")
        self.error_code = error_code

    def __eq__(self, o):
        return isinstance(o, Failed) and o.error_code == self.error_code

    def __repr__(self):
        return "Failed(%r)" % (self.error_code,)


VARIANTS = (Queued, Running, Succeeded, Failed)


def handle(state):
    if isinstance(state, Queued):
        return "waiting"
    if isinstance(state, Running):
        return "working"
    if isinstance(state, Succeeded):
        return "done"
    if isinstance(state, Failed):
        return "failed:" + state.error_code
    raise ValueError("ORACLE_NONEXHAUSTIVE_STATE: unhandled JobState")


def serialize(state):
    if isinstance(state, Failed):
        return {"kind": "Failed", "error_code": state.error_code}
    return {"kind": state.kind}


def deserialize(d):
    if d["kind"] == "Failed":
        return Failed(d["error_code"])
    for cls in (Queued, Running, Succeeded):
        if cls.kind == d["kind"]:
            return cls()
    raise ValueError("ORACLE_NONEXHAUSTIVE_STATE: unknown kind")
