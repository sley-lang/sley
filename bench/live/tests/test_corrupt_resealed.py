"""S2B-CORRUPT-001 resealed embedded-pack vector (judge-side).

Pure checks (need only the pinned oracle project through uv): the staged
exchange parses to canonical object ranges inside the embedded tag-170
pack; each resealed vector differs from the staged exchange in exactly
one canonical object byte plus the exchange trailer; the recomputed
trailer equals the owner's on the untouched exchange; a diverging
recomputation fails as a harness error, never as a verdict.

Integration checks (skipped without a sley binary): the resealed vectors
refuse with exact PACK_DIGEST_MISMATCH through `exchange.import` with
the destination unchanged and the clean control accepted; and two
stand-ins that target the rejection path are rejected by the judge with
the exact reject symbol: an accepted-corruption stand-in (the unflipped
control offered as the corruption) -> ORACLE_CORRUPT_ACCEPTED, and an
unresealed flip offered as the resealed vector ->
ORACLE_CORRUPT_UNREFUSED carrying EXCHANGE_DIGEST_MISMATCH (the exchange
code is never accepted for the PACK code).
"""

from __future__ import annotations

import os
import unittest
from pathlib import Path
from unittest import mock

from bench.fixtures import sley2_live_judge as judge
from bench.live import sley2_codecs

ROOT = Path(__file__).resolve().parents[3]
TASK_DIR = ROOT / "bench" / "fixtures" / "sley2" / "S2B-CORRUPT-001"
# The same offsets the Rust pin flips (crates/sley-repo/tests/s3_g2_corrupt.rs,
# s3_corrupt_exchange_resealed_embedded_pack prints flipped_offsets=1301,2637).
RUST_PIN_OFFSETS = (1301, 2637)


def _uv_available() -> bool:
    try:
        sley2_codecs._uv()
    except sley2_codecs.CodecError:
        return False
    return True


def _sley_binary() -> Path | None:
    for candidate in (
        os.environ.get("SLEY2_SLEY_BINARY", ""),
        os.path.join(os.environ.get("CARGO_TARGET_DIR", ""), "debug", "sley"),
        str(ROOT / "target" / "debug" / "sley"),
    ):
        if not candidate:
            continue
        path = Path(candidate)
        try:
            if path.is_file() and not path.is_symlink() and os.access(path, os.X_OK):
                return path
        except OSError:
            continue
    return None


class ResealedVectorLayoutTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not _uv_available():
            raise unittest.SkipTest("uv unavailable (pinned oracle blake3)")
        cls.exchange = (TASK_DIR / "base.pack").read_bytes()
        cls.built = judge._resealed_pack_vectors(cls.exchange)

    def test_control_is_the_staged_exchange(self) -> None:
        self.assertEqual(bytes.fromhex(self.built["control_hex"]), self.exchange)

    def test_two_independent_object_byte_flips(self) -> None:
        objects = judge._embedded_objects(self.exchange)
        vectors = self.built["vectors"]
        self.assertEqual(len(vectors), 2)
        offsets = tuple(vector["offset"] for vector in vectors)
        self.assertEqual(offsets, RUST_PIN_OFFSETS)
        owners = set()
        trailer = len(self.exchange) - 32
        for vector in vectors:
            data = bytes.fromhex(vector["hex"])
            self.assertEqual(len(data), len(self.exchange))
            diffs = [i for i, (a, b) in enumerate(zip(data, self.exchange)) if a != b]
            self.assertEqual(diffs[0], vector["offset"])
            self.assertTrue(all(i >= trailer for i in diffs[1:]), diffs)
            self.assertNotEqual(data[trailer:], self.exchange[trailer:])
            inside = [k for k, (start, length) in enumerate(objects)
                      if start < vector["offset"] < start + length - 1]
            self.assertEqual(len(inside), 1)
            owners.add(inside[0])
        self.assertEqual(len(owners), 2, "flips land in different canonical objects")

    def test_diverging_recomputation_is_a_harness_error(self) -> None:
        with mock.patch.object(judge, "_exchange_digests",
                               side_effect=lambda prefixes: [bytes(32)] * len(prefixes)):
            with self.assertRaises(judge.JudgeHarnessError) as caught:
                judge._resealed_pack_vectors(self.exchange)
        self.assertIn("reseal diverges", str(caught.exception))


class ResealedVectorImportTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        sley = _sley_binary()
        if sley is None or not _uv_available():
            raise unittest.SkipTest("sley binary or uv unavailable")
        cls.env = mock.patch.dict(os.environ, {"SLEY2_SLEY_BINARY": str(sley)})
        cls.env.start()
        cls.exchange = (TASK_DIR / "base.pack").read_bytes()
        cls.built = judge._resealed_pack_vectors(cls.exchange)

    @classmethod
    def tearDownClass(cls) -> None:
        cls.env.stop()

    def test_resealed_vectors_refuse_with_exact_pack_code(self) -> None:
        evidence = judge._judge_corrupt_pack_resealed(TASK_DIR, self.built)
        self.assertEqual(evidence["control"], "accepted")
        self.assertEqual(len(evidence["fresh"]), 3)
        self.assertEqual(len(evidence["populated"]), 2)
        for row in evidence["fresh"] + evidence["populated"]:
            self.assertEqual(row["symbol"], "PACK_DIGEST_MISMATCH")
        self.assertGreater(evidence["live_objects"], 0)

    def test_accepted_corruption_standin_is_rejected(self) -> None:
        standin = {"control_hex": self.built["control_hex"],
                   "vectors": [{"label": "accepted-standin", "offset": None,
                                "hex": self.built["control_hex"]}]}
        with self.assertRaises(judge.JudgeRejection) as caught:
            judge._judge_corrupt_pack_resealed(TASK_DIR, standin)
        self.assertEqual(caught.exception.code, "ORACLE_CORRUPT_ACCEPTED")

    def test_unresealed_flip_is_not_accepted_for_the_pack_code(self) -> None:
        offset = RUST_PIN_OFFSETS[0]
        broken = bytearray(self.exchange)
        broken[offset] ^= 0x01
        standin = {"control_hex": self.built["control_hex"],
                   "vectors": [{"label": "unresealed-standin", "offset": offset,
                                "hex": bytes(broken).hex()}]}
        with self.assertRaises(judge.JudgeRejection) as caught:
            judge._judge_corrupt_pack_resealed(TASK_DIR, standin)
        self.assertEqual(caught.exception.code, "ORACLE_CORRUPT_UNREFUSED")
        self.assertIn("EXCHANGE_DIGEST_MISMATCH", caught.exception.detail)


if __name__ == "__main__":
    unittest.main()
