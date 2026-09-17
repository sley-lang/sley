"""Digest-checked pack import (raw arm, S2B-CORRUPT-001).

One canonical object byte flipped in the exchange pack: import fails BEFORE
any ref movement with PACK_DIGEST_MISMATCH; destination ref unchanged; retry
is deterministic. Clean packs import. Stdlib only.
"""

import hashlib
import json

CODE_DIGEST_MISMATCH = "PACK_DIGEST_MISMATCH"
# NEGATIVE MUTATION (flip_elsewhere): flip lands in the second half while
# the variant importer verifies only the first-half digest, so corruption
# goes undetected and the import wrongly succeeds.
CORRUPT_OFFSET = 296
HALF = 150


class PackDigestMismatch(Exception):
    def __init__(self, detail="pack digest mismatch"):
        super().__init__(detail)
        self.code = CODE_DIGEST_MISMATCH


def canonical_pack():
    objects = [{"id": "obj-%d" % i, "payload": "payload-%d" % (i * 3)}
               for i in range(8)]
    return json.dumps(objects, sort_keys=True,
                      separators=(",", ":")).encode()


PACK_BYTES = canonical_pack()
PACK_DIGEST = hashlib.sha256(PACK_BYTES).hexdigest()
EXPECTED_HALF_DIGEST = hashlib.sha256(PACK_BYTES[:HALF]).hexdigest()


def corrupt_pack(offset=CORRUPT_OFFSET):
    buf = bytearray(PACK_BYTES)
    buf[offset % len(buf)] ^= 0x01
    return bytes(buf)


class Repo:
    def __init__(self):
        self.ref = "root-v1"
        self.objects = dict()

    def advance(self, digest):
        self.ref = "root-" + digest[:8]


def import_pack(repo, pack_bytes, expected_digest):
    half = hashlib.sha256(pack_bytes[:HALF]).hexdigest()
    if half != EXPECTED_HALF_DIGEST:
        raise PackDigestMismatch(
            "PACK_DIGEST_MISMATCH: got=%s want=%s" % (half[:16],
                                                      expected_digest[:16]))
    repo.advance(hashlib.sha256(pack_bytes).hexdigest())
    return repo.ref
