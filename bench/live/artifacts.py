"""Create-only content-addressed custody for live campaign evidence."""

from __future__ import annotations

import hashlib
import os
import re
import stat
from dataclasses import dataclass
from pathlib import Path


HEX_64 = re.compile(r"[0-9a-f]{64}\Z")


class ArtifactError(ValueError):
    """A fail-closed live artifact custody error."""


def _fail(symbol: str, detail: str = "") -> None:
    raise ArtifactError(symbol if not detail else f"{symbol}: {detail}")


@dataclass(frozen=True)
class ArtifactRef:
    sha256: str
    size_bytes: int
    path: Path


class ArtifactStore:
    """A local create-only SHA-256 object store.

    The store supplies byte identity and detects local mutation. A later
    terminal receipt anchors its inventory outside the run directory.
    """

    def __init__(self, root: Path) -> None:
        self.root = Path(root)
        self.objects = self.root / "objects" / "sha256"

    def path_for(self, digest: str) -> Path:
        if not isinstance(digest, str) or HEX_64.fullmatch(digest) is None:
            _fail("LIVE_ARTIFACT_DIGEST_INVALID", str(digest)[:80])
        return self.objects / digest[:2] / digest[2:]

    def put(self, payload: bytes) -> ArtifactRef:
        if not isinstance(payload, bytes):
            _fail("LIVE_ARTIFACT_INVALID", "payload is not bytes")
        digest = hashlib.sha256(payload).hexdigest()
        path = self.path_for(digest)
        path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        try:
            descriptor = os.open(path, flags, 0o600)
        except FileExistsError:
            existing = self.read(digest)
            if existing != payload:
                _fail("LIVE_ARTIFACT_DIGEST_MISMATCH", digest)
            return ArtifactRef(digest, len(payload), path)
        except OSError as error:
            _fail("LIVE_ARTIFACT_UNSAFE", str(error))
        try:
            view = memoryview(payload)
            written = 0
            while written < len(view):
                count = os.write(descriptor, view[written:])
                if count <= 0:
                    _fail("LIVE_ARTIFACT_WRITE_FAILED", digest)
                written += count
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
        return ArtifactRef(digest, len(payload), path)

    def read(self, digest: str) -> bytes:
        path = self.path_for(digest)
        flags = os.O_RDONLY
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        try:
            descriptor = os.open(path, flags)
        except FileNotFoundError:
            _fail("LIVE_ARTIFACT_MISSING", digest)
        except OSError as error:
            _fail("LIVE_ARTIFACT_UNSAFE", str(error))
        try:
            metadata = os.fstat(descriptor)
            if not stat.S_ISREG(metadata.st_mode):
                _fail("LIVE_ARTIFACT_UNSAFE", digest)
            chunks: list[bytes] = []
            while True:
                chunk = os.read(descriptor, 1024 * 1024)
                if not chunk:
                    break
                chunks.append(chunk)
        finally:
            os.close(descriptor)
        payload = b"".join(chunks)
        if hashlib.sha256(payload).hexdigest() != digest:
            _fail("LIVE_ARTIFACT_DIGEST_MISMATCH", digest)
        return payload

    def verify(self) -> list[str]:
        if not self.objects.exists():
            return []
        digests: list[str] = []
        for prefix in sorted(self.objects.iterdir()):
            if prefix.is_symlink() or not prefix.is_dir() or not re.fullmatch(r"[0-9a-f]{2}", prefix.name):
                _fail("LIVE_ARTIFACT_UNSAFE", str(prefix))
            for leaf in sorted(prefix.iterdir()):
                digest = prefix.name + leaf.name
                if leaf.is_symlink() or HEX_64.fullmatch(digest) is None:
                    _fail("LIVE_ARTIFACT_UNSAFE", str(leaf))
                self.read(digest)
                digests.append(digest)
        return sorted(digests)
