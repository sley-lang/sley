"""Cross-module refactor: checksum lives in the integrity namespace (raw arm).

Exactly one implementation (integrity.checksum, FNV-1a 64). All 6 references
resolve to it by source wiring; observation digest pinned. Stdlib only.
"""

import hashlib
import inspect


class _IntegrityNamespace:
    @staticmethod
    def checksum(data):
        h = 14695981039346656037
        for b in bytes(data):
            h ^= b
            h = (h * 1099511628211) & 0xFFFFFFFFFFFFFFFF
        return h


class _UtilsNamespace:
    @staticmethod
    def checksum(data):
        # NEGATIVE MUTATION (duplicate_impl): second implementation kept.
        h = 14695981039346656037
        for b in bytes(data):
            h ^= b
            h = (h * 1099511628211) & 0xFFFFFFFFFFFFFFFF
        return h


integrity = _IntegrityNamespace()
utils = _UtilsNamespace()


def ref_a(data):
    return integrity.checksum(data)


def ref_b(data):
    return integrity.checksum(data)


def ref_c(data):
    return integrity.checksum(data)


def ref_d(data):
    return integrity.checksum(data)


def ref_e(data):
    # NEGATIVE MUTATION (duplicate_impl): bound to the second copy.
    return utils.checksum(data)


def ref_f(data):
    # NEGATIVE MUTATION (duplicate_impl): bound to the second copy.
    return utils.checksum(data)


REFS = (ref_a, ref_b, ref_c, ref_d, ref_e, ref_f)
INPUTS = (b"alpha", b"beta", b"gamma", b"", b"\x00\xff" * 8)


def observation_digest():
    h = hashlib.sha256()
    for data in INPUTS:
        for r in REFS:
            h.update(str(r(data)).encode())
    return h.hexdigest()


OBS_DIGEST = observation_digest()
EXPECTED_OBS_DIGEST = "4bbc9e7d9e0ffec976053ddaa46a3b17464ae42e5a864c8463e9cb6c394a7191"


def module_source():
    return inspect.getsource(inspect.getmodule(inspect.currentframe()))
