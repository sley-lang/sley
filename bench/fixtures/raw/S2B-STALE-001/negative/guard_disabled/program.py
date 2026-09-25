"""Version-guarded concurrent update (raw arm, S2B-STALE-001).

Two sessions open on one root; A commits ok; B's commit is rejected with
STALE_ROOT; after re-query a new candidate validates. No last-write-wins.
Stdlib only.
"""

CODE_STALE = "STALE_ROOT"


class StaleRoot(Exception):
    def __init__(self, detail="stale root"):
        super().__init__(detail)
        self.code = CODE_STALE


class Store:
    def __init__(self):
        self.version = 0
        self.funcs = {"net_total": "v0"}

    def open_session(self):
        return Session(self, self.version)


class Session:
    def __init__(self, store, base_version):
        self.store = store
        self.base_version = base_version
        self.staged = {}

    def stage_replace(self, name, code):
        self.staged[name] = code

    def commit(self):
        # NEGATIVE MUTATION (guard_disabled): staleness check removed, so
        # the second commit silently wins (last-write-wins).
        self.store.funcs.update(self.staged)
        self.store.version += 1
        return self.store.version

    def requery(self):
        return self.store.open_session()


def validate(candidate_funcs):
    return "net_total" in candidate_funcs
