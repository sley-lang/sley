from __future__ import annotations

import json
import os
import tempfile
import unittest
from pathlib import Path

from bench.live.snapshot import (
    SnapshotError,
    decode_snapshot,
    encode_snapshot,
    restore_snapshot,
    snapshot_directory,
)


class WorkspaceSnapshotTests(unittest.TestCase):
    def test_snapshot_round_trip_preserves_bytes_modes_and_empty_directories(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "source"
            root.mkdir()
            (root / "empty").mkdir()
            nested = root / "nested"
            nested.mkdir()
            script = nested / "run.sh"
            script.write_bytes(b"#!/bin/sh\nprintf ok\n")
            script.chmod(0o750)
            (root / "binary.bin").write_bytes(bytes(range(256)))

            snapshot = snapshot_directory(root)
            encoded = encode_snapshot(snapshot)
            self.assertEqual(decode_snapshot(encoded), snapshot)
            self.assertEqual(encoded, encode_snapshot(decode_snapshot(encoded)))
            self.assertEqual(json.loads(encoded)["contract"], "sley2.live-workspace-snapshot.v1")

            destination = Path(temporary) / "restored"
            restore_snapshot(snapshot, destination)
            self.assertEqual((destination / "binary.bin").read_bytes(), bytes(range(256)))
            self.assertTrue((destination / "empty").is_dir())
            self.assertEqual((destination / "nested/run.sh").stat().st_mode & 0o777, 0o750)
            self.assertEqual(snapshot_directory(destination), snapshot)

    def test_links_special_entries_and_nonempty_destination_are_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "source"
            root.mkdir()
            (root / "file").write_text("x", encoding="utf-8")
            os.symlink("file", root / "link")
            with self.assertRaisesRegex(SnapshotError, "LIVE_SNAPSHOT_UNSAFE"):
                snapshot_directory(root)
            (root / "link").unlink()
            snapshot = snapshot_directory(root)
            destination = Path(temporary) / "destination"
            destination.mkdir()
            (destination / "existing").write_text("no", encoding="utf-8")
            with self.assertRaisesRegex(SnapshotError, "LIVE_SNAPSHOT_DESTINATION"):
                restore_snapshot(snapshot, destination)

    def test_tampered_paths_digests_and_noncanonical_bytes_are_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "source"
            root.mkdir()
            (root / "file").write_text("payload", encoding="utf-8")
            snapshot = snapshot_directory(root)
            for mutation in ("path", "digest"):
                candidate = json.loads(encode_snapshot(snapshot))
                if mutation == "path":
                    candidate["entries"][0]["path"] = "../escape"
                else:
                    candidate["entries"][0]["sha256"] = "0" * 64
                with self.subTest(mutation=mutation):
                    with self.assertRaises(SnapshotError):
                        decode_snapshot(
                            json.dumps(candidate, sort_keys=True, separators=(",", ":")).encode()
                        )
            with self.assertRaisesRegex(SnapshotError, "LIVE_SNAPSHOT_INVALID"):
                decode_snapshot(encode_snapshot(snapshot) + b"\n")


if __name__ == "__main__":
    unittest.main()
