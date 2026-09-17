"""Bounded capsule repair over a 10k-entity graph (raw arm, S2B-CONTEXT-001).

Generated graph (>=10000 entities, deterministic, no randomness). A required
record field is applied over the complete impact closure using explicit
chunk/continuation records; responses never exceed CHUNK_MAX; whole-store
reads and invalid commits are zero. Stdlib only.
"""

N_ENTITIES = 10000
N_TYPES = 40
TARGET_TYPE = "invoice"
FIELD = "tax_basis_points"
DEFAULT_VALUE = 0
CHUNK_MAX = 100
CODE_UNBOUNDED = "ORACLE_UNBOUNDED_READ_REFUSED"


class UnboundedReadRefused(Exception):
    def __init__(self, detail="whole-store read refused"):
        super().__init__(detail)
        self.code = CODE_UNBOUNDED


class Continuation:
    def __init__(self, start, next_start):
        self.start = start
        self.next_start = next_start

    def __repr__(self):
        return "Continuation(start=%d, next=%r)" % (self.start,
                                                    self.next_start)


class Store:
    def __init__(self):
        self.entities = [
            {"id": i, "type": "type-%02d" % (i % N_TYPES),
             "payload": "p-%d" % i}
            for i in range(N_ENTITIES)
        ]
        # Designate every 40th entity (starting at 0) as the shared type.
        for i in range(0, N_ENTITIES, N_TYPES):
            self.entities[i]["type"] = TARGET_TYPE
        self.by_type = {}
        for e in self.entities:
            self.by_type.setdefault(e["type"], []).append(e["id"])
        self.whole_store_reads = 0
        self.invalid_commits = 0
        self.max_chunk = 0
        self.continuations = []

    def read_all(self):
        # NEGATIVE MUTATION (unbounded_read): guard removed; whole-store
        # dump succeeds and is counted.
        self.whole_store_reads += 1
        return list(self.entities)

    def impact_closure(self, entity_type):
        return list(self.by_type.get(entity_type, []))

    def iter_chunks(self, ids):
        offset = 0
        while offset < len(ids):
            chunk = ids[offset:offset + CHUNK_MAX]
            nxt = offset + len(chunk)
            cont = Continuation(offset,
                                nxt if nxt < len(ids) else None)
            self.continuations.append(repr(cont))
            if len(chunk) > CHUNK_MAX:
                self.invalid_commits += 1
            self.max_chunk = max(self.max_chunk, len(chunk))
            yield chunk, cont
            offset = nxt

    def add_required_field(self, entity_type, field, default):
        ids = self.impact_closure(entity_type)
        updated = 0
        for chunk, _cont in self.iter_chunks(ids):
            for eid in chunk:
                self.entities[eid][field] = default
                updated += 1
        return updated
