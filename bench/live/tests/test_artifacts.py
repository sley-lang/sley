from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from bench.live.artifacts import ArtifactError, ArtifactStore


class ArtifactStoreTests(unittest.TestCase):
    def test_put_is_create_once_and_verifiable(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            store = ArtifactStore(Path(directory))
            first = store.put(b"one")
            again = store.put(b"one")
            second = store.put(b"two")

            self.assertEqual(first, again)
            self.assertNotEqual(first.sha256, second.sha256)
            self.assertEqual(store.read(first.sha256), b"one")
            self.assertEqual(store.verify(), sorted([first.sha256, second.sha256]))

    def test_tamper_and_symlink_are_refused(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            store = ArtifactStore(Path(directory))
            artifact = store.put(b"evidence")
            artifact.path.write_bytes(b"tampered")
            with self.assertRaisesRegex(ArtifactError, "LIVE_ARTIFACT_DIGEST_MISMATCH"):
                store.read(artifact.sha256)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            store = ArtifactStore(root)
            digest = "00" * 32
            target = root / "outside"
            target.write_bytes(b"outside")
            leaf = store.path_for(digest)
            leaf.parent.mkdir(parents=True, exist_ok=True)
            leaf.symlink_to(target)
            with self.assertRaisesRegex(ArtifactError, "LIVE_ARTIFACT_UNSAFE"):
                store.read(digest)

    def test_invalid_digest_is_refused_before_path_lookup(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            store = ArtifactStore(Path(directory))
            with self.assertRaisesRegex(ArtifactError, "LIVE_ARTIFACT_DIGEST_INVALID"):
                store.read("../escape")


if __name__ == "__main__":
    unittest.main()
