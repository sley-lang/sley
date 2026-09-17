"""Unit tests for S2B-CORRUPT-001 (stdlib unittest)."""

import unittest

import program
from program import Repo, PackDigestMismatch


class CorruptTest(unittest.TestCase):
    def test_corrupt_import_rejected(self):
        repo = Repo()
        with self.assertRaises(PackDigestMismatch) as cm:
            program.import_pack(repo, program.corrupt_pack(),
                                program.PACK_DIGEST)
        self.assertEqual(cm.exception.code, "PACK_DIGEST_MISMATCH")

    def test_ref_unchanged(self):
        repo = Repo()
        before = repo.ref
        try:
            program.import_pack(repo, program.corrupt_pack(),
                                program.PACK_DIGEST)
        except PackDigestMismatch:
            pass
        self.assertEqual(repo.ref, before)
        self.assertEqual(repo.ref, "root-v1")

    def test_clean_pack_imports(self):
        repo = Repo()
        ref = program.import_pack(repo, program.PACK_BYTES,
                                  program.PACK_DIGEST)
        self.assertNotEqual(ref, "root-v1")

    def test_retry_deterministic(self):
        codes = []
        for _ in range(2):
            repo = Repo()
            try:
                program.import_pack(repo, program.corrupt_pack(),
                                    program.PACK_DIGEST)
            except PackDigestMismatch as e:
                codes.append(e.code)
        self.assertEqual(codes, ["PACK_DIGEST_MISMATCH"] * 2)


if __name__ == "__main__":
    unittest.main()
