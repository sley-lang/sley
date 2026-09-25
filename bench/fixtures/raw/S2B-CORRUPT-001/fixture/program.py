"""Digest-checked pack import (raw arm, S2B-CORRUPT-001).

One canonical object byte flipped in the exchange pack: import fails BEFORE
any ref movement with PACK_DIGEST_MISMATCH; destination ref unchanged; retry
is deterministic. Clean packs import. Stdlib only.
"""

import hashlib
import json

CODE_DIGEST_MISMATCH = "PACK_DIGEST_MISMATCH"
CORRUPT_OFFSET = 17


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
    digest = hashlib.sha256(pack_bytes).hexdigest()
    if digest != expected_digest:
        raise PackDigestMismatch(
            "PACK_DIGEST_MISMATCH: got=%s want=%s" % (digest[:16],
                                                      expected_digest[:16]))
    repo.advance(digest)
    return repo.ref
