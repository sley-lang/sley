#!/usr/bin/env python3
"""S20-720 release candidate mechanics: two clean builds, a deterministic
artifact, an unpacked source-independent demo, scans, and reproducibility.

`make release-check` stays fail-closed; this script produces the candidate
and its evidence without claiming GA or authorizing publication.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
from enum import IntEnum
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DIST = ROOT / "dist"
ARTIFACT_STEM = "sley-2.0.0-linux-x86_64"
ARTIFACT_NAME = f"{ARTIFACT_STEM}.tar.gz"
# Release link contract (cross-host reproducibility repair): the candidate
# links self-contained static (musl) with the rust-lld and musl runtime from
# the pinned Rust toolchain, never the ambient host cc/linker or host
# glibc/CRT. Forensics on 2f93364 showed the ambient cc (gcc 16.2.1 vs
# 13.3.0) and host glibc/CRT (2.44 vs 2.39) diverging bin/sley across hosts
# while rustc stayed pinned; pinning the whole link unit to the pinned
# toolchain removes the host from the link. No clang-18 (fuzz lanes) is
# involved: the release link is hermetic by target selection.
RELEASE_TARGET = "x86_64-unknown-linux-musl"
# Link environment that must never influence the release build: any ambient
# CC (or flags) would reselect the host toolchain behind the target pin.
SCRUBBED_LINK_ENV = ("CC", "CXX", "CFLAGS", "CXXFLAGS", "CPPFLAGS", "LDFLAGS", "LD")
# Ambient compiler and profile overrides must not bypass the pinned build.
SCRUBBED_BUILD_ENV = ("CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER")
SCRUBBED_BUILD_ENV_PREFIXES = ("CARGO_PROFILE_RELEASE_",)
MANIFEST_CONTRACT = "sley2.release-candidate-manifest.v1"
EVIDENCE_DIR = ROOT / "evidence/runtime/s20-720-release-candidate"
INVENTORY = ROOT / "evidence/security/T52/pre-release-inventory.json"
CONFORMANCE_SUBSET = ("conformance/smp1/v1", "conformance/smp1-json-bridge/v1", "conformance/release-demo/v1")
# Compile-time embeds outside crates/ also change the packaged binary.
EMBEDDED_INPUT_PATHS = (
    "docs/spec/SSMC1_EPOCH1_SCHEMA.txt",
    "conformance/smp1-json-bridge/v2/methods.json",
)
# Every tracked input the staged artifact derives from: the release binary is
# built from the Rust workspace and its compile-time embeds, the packaging
# logic below stages it, and the stage adds the demo runner, the root
# license files, the SBOM inventory, and the conformance subset. S20-730
# reads this surface for its attestation freshness rule and S20-710 derives
# its records-closure bound inputs from it; keep it exact when staging or
# embedding changes.
ARTIFACT_INPUT_PATHS = (
    "crates",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "LICENSE",
    "NOTICE",
    "bench/release/run_demo.py",
    "evidence/security/T52/pre-release-inventory.json",
    "scripts/build_release_candidate.py",
    *EMBEDDED_INPUT_PATHS,
    *CONFORMANCE_SUBSET,
)
FIXED_ARTIFACT_MEMBERS = frozenset({
    "bin/sley", "MANIFEST.json", "SBOM.json", "LICENSES.json",
    "LICENSE", "NOTICE", "demo/run_demo.py",
})
EXECUTABLE_MEMBERS = {"bin/sley", "demo/run_demo.py"}


def expected_artifact_members(fixture_paths: list[str]) -> set[str]:
    """S20-720 owns the fixed contents; fixtures are enumerated at the candidate."""
    return set(FIXED_ARTIFACT_MEMBERS).union(fixture_paths)

SECRET_PATTERNS = (b"-----BEGIN ", b"AKIA", b"ghp_", b"xoxb-", b"xoxp-", b"sk-ant-", b"sk-proj-")
# The operator-approved root license set (S20-710 license decision
# 2026-09-14): the staged artifact ships the installed files, never a
# pending-license placeholder. The inventory must approve exactly this set
# or staging refuses (INTERNAL_INVARIANT) instead of minting stale text.
APPROVED_LICENSE_FILES = ("LICENSE", "NOTICE")


class PackageErrorCode(IntEnum):
    BUILD_FAILED = 72_000
    MANIFEST_INVALID = 72_001
    CONTENT_FORBIDDEN = 72_002
    CONFORMANCE_FAILED = 72_003
    DEMO_FAILED = 72_004
    NOT_REPRODUCIBLE = 72_005
    TREE_DIRTY = 72_006
    INTERNAL_INVARIANT = 72_007


SYMBOLS = {
    PackageErrorCode.BUILD_FAILED: "PACKAGE_BUILD_FAILED",
    PackageErrorCode.MANIFEST_INVALID: "PACKAGE_MANIFEST_INVALID",
    PackageErrorCode.CONTENT_FORBIDDEN: "PACKAGE_CONTENT_FORBIDDEN",
    PackageErrorCode.CONFORMANCE_FAILED: "PACKAGE_CONFORMANCE_FAILED",
    PackageErrorCode.DEMO_FAILED: "PACKAGE_DEMO_FAILED",
    PackageErrorCode.NOT_REPRODUCIBLE: "PACKAGE_NOT_REPRODUCIBLE",
    PackageErrorCode.TREE_DIRTY: "PACKAGE_TREE_DIRTY",
    PackageErrorCode.INTERNAL_INVARIANT: "PACKAGE_INTERNAL_INVARIANT",
}


class PackageError(Exception):
    def __init__(
        self, code: PackageErrorCode, detail: str = "", evidence: dict | None = None
    ):
        super().__init__(f"{SYMBOLS[code]}:{detail}" if detail else SYMBOLS[code])
        self.code = code
        self.detail = detail
        # Partial candidate evidence assembled before the failure: the
        # caller attaches it to the evidence record instead of a stub, so
        # a refused mint is reconstructible from its evidence alone.
        self.evidence = evidence

    @property
    def symbol(self) -> str:
        return SYMBOLS[self.code]


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("utf-8")


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run(command: list[str], *, cwd: Path, env: dict[str, str] | None = None, timeout: int = 1800) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True, timeout=timeout, check=False)


# ---------------------------------------------------------------------------
# Primitives (offline-tested)
# ---------------------------------------------------------------------------


def member_paths(stage: Path) -> list[Path]:
    return sorted(path for path in stage.rglob("*") if path.is_file())


def build_manifest(
    stage: Path,
    *,
    commit: str,
    toolchain: dict[str, str],
    working_tree_clean: bool,
    blockers: list[str],
    ga_claimed: bool = False,
    publication_authorized: bool = False,
    artifact_name: str = ARTIFACT_NAME,
) -> dict:
    """The canonical manifest of every staged member except itself.

    The manifest carries its own non-release status inside the digest: the
    GA and publication flags, the blockers that keep release-check
    fail-closed, and the cleanliness of the built tree, so the artifact is
    self-describing once it leaves dist/ and a dirty build can never carry
    a bare commit.
    """

    files = []
    for path in member_paths(stage):
        relative = path.relative_to(stage).as_posix()
        if relative == "MANIFEST.json":
            continue
        files.append({"path": relative, "sha256": sha256_file(path), "size": path.stat().st_size})
    manifest = {
        "artifact": artifact_name,
        "blockers": list(blockers),
        "commit": commit,
        "contract": MANIFEST_CONTRACT,
        "files": files,
        "ga_claimed": ga_claimed,
        "member_count": len(files),
        "publication_authorized": publication_authorized,
        "target": RELEASE_TARGET,
        "toolchain": toolchain,
        "working_tree_clean": working_tree_clean,
    }
    manifest["manifest_digest"] = sha256_bytes(canonical({key: value for key, value in manifest.items() if key != "manifest_digest"}))
    return manifest


def verify_manifest(stage: Path, manifest: dict) -> None:
    for key in (
        "artifact",
        "blockers",
        "commit",
        "contract",
        "files",
        "ga_claimed",
        "member_count",
        "publication_authorized",
        "target",
        "toolchain",
        "working_tree_clean",
    ):
        if key not in manifest:
            raise PackageError(PackageErrorCode.MANIFEST_INVALID, f"missing {key}")
    expected = {entry["path"]: entry for entry in manifest.get("files", [])}
    present = {path.relative_to(stage).as_posix() for path in member_paths(stage)} - {"MANIFEST.json"}
    if set(expected) != present:
        raise PackageError(PackageErrorCode.MANIFEST_INVALID, "member set")
    for relative, entry in expected.items():
        path = stage / relative
        if sha256_file(path) != entry["sha256"] or path.stat().st_size != entry["size"]:
            raise PackageError(PackageErrorCode.MANIFEST_INVALID, relative)
    body = {key: value for key, value in manifest.items() if key != "manifest_digest"}
    if manifest.get("manifest_digest") != sha256_bytes(canonical(body)):
        raise PackageError(PackageErrorCode.MANIFEST_INVALID, "digest")


def deterministic_tar(stage: Path, artifact: Path, top: str = ARTIFACT_STEM) -> bytes:
    """A byte-deterministic gzip tar: sorted members, zero times and owners, normalized modes."""

    buffer = io.BytesIO()
    entries: dict[str, Path | None] = {top: None}
    for path in member_paths(stage):
        relative = path.relative_to(stage)
        for parent in relative.parents:
            if parent.as_posix() != ".":
                entries[f"{top}/{parent.as_posix()}"] = None
        entries[f"{top}/{relative.as_posix()}"] = path
    with tarfile.open(fileobj=buffer, mode="w", format=tarfile.PAX_FORMAT) as archive:
        for name in sorted(entries):
            path = entries[name]
            info = tarfile.TarInfo(name)
            info.mtime = 0
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            if path is None:
                info.type = tarfile.DIRTYPE
                info.mode = 0o755
                archive.addfile(info)
                continue
            relative = path.relative_to(stage).as_posix()
            info.size = path.stat().st_size
            info.mode = 0o755 if relative in EXECUTABLE_MEMBERS else 0o644
            with path.open("rb") as handle:
                archive.addfile(info, handle)
    raw = buffer.getvalue()
    compressed = io.BytesIO()
    with gzip.GzipFile(filename="", mode="wb", fileobj=compressed, mtime=0) as handle:
        handle.write(raw)
    payload = compressed.getvalue()
    artifact.parent.mkdir(parents=True, exist_ok=True)
    artifact.write_bytes(payload)
    return payload


def scan_forbidden_content(stage: Path, forbidden_paths: tuple[str, ...], patterns: tuple[bytes, ...] = SECRET_PATTERNS) -> list[dict]:
    """Members containing a forbidden path or a bounded secret pattern."""

    findings = []
    needles = [path.encode("utf-8") for path in forbidden_paths] + list(patterns)
    for path in member_paths(stage):
        data = path.read_bytes()
        for needle in needles:
            if needle in data:
                findings.append({"member": path.relative_to(stage).as_posix(), "pattern": needle.decode("utf-8", errors="replace")})
    return findings


def tar_members(payload: bytes) -> dict[str, bytes | None]:
    members: dict[str, bytes | None] = {}
    with tarfile.open(fileobj=io.BytesIO(payload), mode="r:gz") as archive:
        for info in archive.getmembers():
            if info.isfile():
                extracted = archive.extractfile(info)
                members[info.name] = extracted.read() if extracted else b""
            else:
                members[info.name] = None
    return members


def compare_artifacts(first: bytes, second: bytes) -> dict:
    """Byte equality, else the differing members and whether the difference is archive-only."""

    if first == second:
        return {"result": "REPRODUCIBLE", "differing_members": [], "archive_only": False}
    left = tar_members(first)
    right = tar_members(second)
    differing = sorted(name for name in set(left) | set(right) if left.get(name) != right.get(name))
    return {
        "result": "NOT_REPRODUCIBLE",
        "differing_members": differing,
        "archive_only": not differing,
    }


# ---------------------------------------------------------------------------
# Pipeline
# ---------------------------------------------------------------------------


def remap_flags(root: Path, cargo_home: Path, home: Path) -> list[str]:
    """The `--remap-path-prefix` flags for a clean build, most general first.

    rustc applies the last matching rule, so the order is load-bearing: the
    home rule first, then the cargo registry rule, then the working tree
    rule last, so a tree path keeps `/sley2` instead of collapsing into
    `/home-remapped`. Reversing this order reintroduces the leak the scan
    cannot see, because `/home-remapped` contains neither the tree path nor
    `/home/`.
    """
    return [
        f"--remap-path-prefix={home}=/home-remapped",
        f"--remap-path-prefix={cargo_home / 'registry' / 'src'}=/cargo/registry/src",
        f"--remap-path-prefix={root}=/sley2",
    ]


def toolchain_versions() -> dict[str, str]:
    versions = {}
    for tool in ("rustc", "cargo"):
        completed = run([tool, "--version"], cwd=ROOT)
        if completed.returncode != 0:
            raise PackageError(PackageErrorCode.BUILD_FAILED, f"{tool} --version")
        versions[tool] = completed.stdout.strip()
    return versions


def link_contract() -> dict[str, str]:
    """The pinned release link unit, kept out of `toolchain`.

    S20-730 requires evidence `toolchain` to name exactly cargo and rustc,
    so link provenance rides alongside it: the release link is pinned by
    target (self-contained musl via the pinned toolchain's rust-lld),
    never the ambient host cc/linker or host glibc/CRT.
    """
    return {
        "target": RELEASE_TARGET,
        "linker": "rust-lld+musl self-contained (pinned toolchain; ambient CC scrubbed)",
    }


def require_release_target() -> None:
    """Fail closed unless the pinned release target std is installed."""
    completed = run(["rustc", "--print", "sysroot"], cwd=ROOT)
    if completed.returncode != 0:
        raise PackageError(PackageErrorCode.BUILD_FAILED, "rustc --print sysroot")
    std = Path(completed.stdout.strip()) / "lib" / "rustlib" / RELEASE_TARGET
    if not std.is_dir():
        raise PackageError(
            PackageErrorCode.BUILD_FAILED,
            f"release target std missing: {RELEASE_TARGET} (rustup target add {RELEASE_TARGET})",
        )


def scrub_build_env(env: dict[str, str]) -> dict[str, str]:
    """The environment minus every link and build override the release
    build must never see (SCRUBBED_LINK_ENV, SCRUBBED_BUILD_ENV, and every
    SCRUBBED_BUILD_ENV_PREFIXES variable)."""
    return {
        name: value
        for name, value in env.items()
        if name not in SCRUBBED_LINK_ENV
        and name not in SCRUBBED_BUILD_ENV
        and not name.startswith(SCRUBBED_BUILD_ENV_PREFIXES)
    }


def clean_build(target: Path, timeout: int) -> Path:
    if target.exists():
        shutil.rmtree(target)
    require_release_target()
    env = scrub_build_env(dict(os.environ))
    env["CARGO_TARGET_DIR"] = str(target)
    cargo_home = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
    home = Path.home()
    # The working tree, the cargo registry sources, and the home directory
    # itself are remapped so no local absolute path survives in the binary.
    env["RUSTFLAGS"] = " ".join(remap_flags(ROOT, cargo_home, home))
    completed = run(
        ["cargo", "build", "--release", "--locked", "--target", RELEASE_TARGET, "-p", "sley-cli"],
        cwd=ROOT,
        env=env,
        timeout=timeout,
    )
    if completed.returncode != 0:
        raise PackageError(PackageErrorCode.BUILD_FAILED, completed.stderr[-500:])
    binary = target / RELEASE_TARGET / "release" / "sley"
    if not binary.is_file():
        raise PackageError(PackageErrorCode.BUILD_FAILED, "binary missing")
    return binary


def stage_artifact(
    binary: Path,
    stage: Path,
    *,
    commit: str,
    toolchain: dict[str, str],
    working_tree_clean: bool,
    blockers: list[str],
) -> dict:
    if stage.exists():
        shutil.rmtree(stage)
    (stage / "bin").mkdir(parents=True)
    shutil.copyfile(binary, stage / "bin/sley")
    os.chmod(stage / "bin/sley", 0o755)
    for relative in CONFORMANCE_SUBSET:
        shutil.copytree(ROOT / relative, stage / relative)
    (stage / "demo").mkdir()
    shutil.copyfile(ROOT / "bench/release/run_demo.py", stage / "demo/run_demo.py")
    inventory = json.loads(INVENTORY.read_text(encoding="utf-8"))
    (stage / "SBOM.json").write_bytes(canonical(inventory) + b"\n")
    license_files = inventory.get("license_text_files")
    if list(license_files or []) != list(APPROVED_LICENSE_FILES):
        raise PackageError(
            PackageErrorCode.INTERNAL_INVARIANT,
            f"unapproved root license file set: {license_files!r}",
        )
    workspace_dispositions = {
        package.get("license_disposition")
        for package in inventory.get("packages", [])
        if package.get("workspace")
    }
    if workspace_dispositions != {"APPROVED_OPERATOR_APACHE_2_0_ROOT_LICENSE"} or inventory.get("blockers") != []:
        raise PackageError(
            PackageErrorCode.INTERNAL_INVARIANT,
            f"workspace license not approved: {sorted(workspace_dispositions)!r}",
        )
    staged_digests = {}
    for name in APPROVED_LICENSE_FILES:
        data = (ROOT / name).read_bytes()
        digest = sha256_bytes(data)
        if digest != inventory.get("root_license_sha256" if name == "LICENSE" else "notice_sha256"):
            raise PackageError(
                PackageErrorCode.INTERNAL_INVARIANT,
                f"staged {name} differs from the inventoried license digest",
            )
        (stage / name).write_bytes(data)
        staged_digests[name] = digest
    licenses = {
        "contract": "sley2.release-candidate-licenses.v1",
        "root_license": {
            "status": "APPROVED_OPERATOR_APACHE_2_0",
            "spdx": "Apache-2.0",
            "files": [
                {"name": name, "sha256": staged_digests[name]}
                for name in APPROVED_LICENSE_FILES
            ],
        },
        "packages": [
            {
                "name": package["name"],
                "version": package["version"],
                "license_declared": package.get("license_declared"),
                "disposition": package.get("license_disposition"),
                "workspace": package.get("workspace"),
            }
            for package in inventory.get("packages", [])
            if package.get("ecosystem") == "cargo"
        ],
    }
    (stage / "LICENSES.json").write_bytes(canonical(licenses) + b"\n")
    manifest = build_manifest(
        stage,
        commit=commit,
        toolchain=toolchain,
        working_tree_clean=working_tree_clean,
        blockers=blockers,
    )
    (stage / "MANIFEST.json").write_bytes(canonical(manifest) + b"\n")
    fixtures = [
        path.relative_to(stage).as_posix()
        for relative in CONFORMANCE_SUBSET
        for path in (stage / relative).rglob("*") if path.is_file()
    ]
    if {path.relative_to(stage).as_posix() for path in member_paths(stage)} != expected_artifact_members(fixtures):
        raise PackageError(PackageErrorCode.INTERNAL_INVARIANT, "staging differs from the owned member set")
    return manifest


def unpack(artifact: Path, destination: Path) -> Path:
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    with tarfile.open(artifact, mode="r:gz") as archive:
        archive.extractall(destination, filter="data")
    unpacked = destination / ARTIFACT_STEM
    if not (unpacked / "bin/sley").is_file():
        raise PackageError(PackageErrorCode.MANIFEST_INVALID, "unpacked binary missing")
    os.chmod(unpacked / "bin/sley", 0o755)
    return unpacked


def source_free_env() -> dict[str, str]:
    return {"PATH": "/usr/bin:/bin", "LANG": "C"}


def run_conformance_subset(unpacked: Path, timeout: int) -> dict:
    env = source_free_env()
    sley = unpacked / "bin/sley"
    checks: dict[str, bool] = {}
    methods = run([str(sley), "methods"], cwd=unpacked, env=env, timeout=timeout)
    checks["methods_equal_packaged_table"] = methods.returncode == 0 and methods.stdout == (unpacked / "conformance/smp1-json-bridge/v1/methods.json").read_text(encoding="utf-8")
    version = run([str(sley), "version"], cwd=unpacked, env=env, timeout=timeout)
    checks["version_names_protocol"] = version.returncode == 0 and '"protocol_version":1' in version.stdout
    checks["version_names_cli"] = version.returncode == 0 and '"cli"' in version.stdout
    smp1 = json.loads((unpacked / "conformance/smp1/v1/accepted.json").read_text(encoding="utf-8"))
    bridge = json.loads((unpacked / "conformance/smp1-json-bridge/v1/roundtrip.json").read_text(encoding="utf-8"))
    expected = {vector["frame_hex"]: vector["json"] for vector in bridge["vectors"]}
    frames = [frame["frame_hex"] for frame in smp1["frames"]] + [hello["frame_hex"] for hello in smp1["hellos"].values()]
    frame_bytes = b"".join(bytes.fromhex(frame) for frame in frames)
    decoded = subprocess.run([str(sley), "frame", "decode"], input=frame_bytes, capture_output=True, cwd=unpacked, env=env, check=False, timeout=timeout)
    lines = decoded.stdout.decode("utf-8").splitlines() if decoded.returncode == 0 else []
    checks["frame_decode_matches_bridge_fixture"] = lines == [expected[frame] for frame in frames]
    encoded = subprocess.run([str(sley), "frame", "encode"], input=("\n".join(lines) + "\n").encode("utf-8"), capture_output=True, cwd=unpacked, env=env, check=False, timeout=timeout)
    checks["frame_encode_reproduces_frames"] = encoded.returncode == 0 and encoded.stdout == frame_bytes
    return {"checks": checks, "result": "PASS" if all(checks.values()) else "FAIL"}


def run_demo(unpacked: Path, timeout: int) -> dict:
    env = source_free_env()
    work = unpacked / "demo-work"
    completed = run(
        [sys.executable, "demo/run_demo.py", "--sley", "bin/sley", "--fixture", "conformance/release-demo/v1/demo.json", "--work", str(work), "--timeout-seconds", str(timeout)],
        cwd=unpacked,
        env=env,
        timeout=timeout * 4,
    )
    try:
        report = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise PackageError(PackageErrorCode.DEMO_FAILED, completed.stderr[-300:]) from error
    if completed.returncode != 0 or report.get("result") != "PASS":
        raise PackageError(PackageErrorCode.DEMO_FAILED, ",".join(report.get("problems", [])))
    shutil.rmtree(work, ignore_errors=True)
    return report


def git_state() -> tuple[str, bool]:
    head = run(["git", "rev-parse", "HEAD"], cwd=ROOT)
    status = run(["git", "status", "--porcelain"], cwd=ROOT)
    if head.returncode != 0 or status.returncode != 0:
        raise PackageError(PackageErrorCode.INTERNAL_INVARIANT, "git state")
    return head.stdout.strip(), status.stdout.strip() == ""


def build_candidate(*, timeout: int, require_clean: bool, keep: bool) -> dict:
    started = time.monotonic()
    commit, clean = git_state()
    if require_clean and not clean:
        raise PackageError(PackageErrorCode.TREE_DIRTY)
    first_toolchain = toolchain_versions()
    DIST.mkdir(exist_ok=True)
    evidence: dict = {
        "contract": "s20-720-release-candidate-v1",
        "work_package": "S20-720",
        "artifact_name": ARTIFACT_NAME,
        "commit": commit,
        "working_tree_clean": clean,
        "toolchain": first_toolchain,
        "link_contract": link_contract(),
        "build_toolchains": {"first": first_toolchain},
        "ga_claimed": False,
        "publication_authorized": False,
        "release_check_gate": "FAIL_CLOSED_NOT_IMPLEMENTED",
        "blockers": [
            "standards_sbom_and_provenance_s20_710_full",
            "succession_thresholds_s20_640",
            "council_reviews",
        ],
    }
    try:
        return build_candidate_stages(evidence, timeout=timeout, keep=keep)
    except PackageError as error:
        if error.evidence is None:
            error.evidence = evidence
        raise


def build_candidate_stages(evidence: dict, *, timeout: int, keep: bool) -> dict:
    """Assemble the staged artifact and its evidence, failing with evidence.

    Every PackageError raised below carries the partial evidence assembled
    so far, so the caller records the failure against the full comparison
    detail instead of a stub (contract sections 6 and 7).
    """
    started = time.monotonic()
    commit = evidence["commit"]
    clean = evidence["working_tree_clean"]
    first_toolchain = evidence["toolchain"]
    try:
        binary = clean_build(DIST / "target-a", timeout)
        stage_a = DIST / "stage-a"
        manifest = stage_artifact(
            binary,
            stage_a,
            commit=commit,
            toolchain=first_toolchain,
            working_tree_clean=clean,
            blockers=evidence["blockers"],
        )
        verify_manifest(stage_a, manifest)
        # The remap residue has its own needle: after the home remap a leaked
        # tree path reads /home-remapped/..., which contains neither the tree
        # path nor /home/, so those two needles can never fire on it. The
        # username catches non-path-shaped leakage (build strings, registry
        # fragments) the remaps do not reach.
        findings = scan_forbidden_content(
            stage_a, (str(ROOT), "/home/", "/home-remapped", Path.home().name)
        )
        if findings:
            raise PackageError(PackageErrorCode.CONTENT_FORBIDDEN, json.dumps(findings[:5]))
        artifact_path = DIST / ARTIFACT_NAME
        first = deterministic_tar(stage_a, artifact_path)
        evidence.update(
            {
                "artifact_path": str(artifact_path.relative_to(ROOT)),
                "artifact_sha256": sha256_bytes(first),
                "artifact_size_bytes": len(first),
                "manifest_digest": manifest["manifest_digest"],
                "member_count": manifest["member_count"],
                "forbidden_content_findings": 0,
            }
        )
        unpack_root = Path(tempfile.mkdtemp(prefix="sley-candidate-"))
        try:
            unpacked = unpack(artifact_path, unpack_root)
            verify_manifest(unpacked, json.loads((unpacked / "MANIFEST.json").read_text(encoding="utf-8")))
            conformance = run_conformance_subset(unpacked, timeout)
            if conformance["result"] != "PASS":
                raise PackageError(PackageErrorCode.CONFORMANCE_FAILED, json.dumps(conformance["checks"]))
            evidence["conformance_subset"] = conformance
            evidence["demo"] = run_demo(unpacked, timeout)
        finally:
            shutil.rmtree(unpack_root, ignore_errors=True)
        second_toolchain = toolchain_versions()
        evidence["build_toolchains"]["second"] = second_toolchain
        second_binary = clean_build(DIST / "target-b", timeout)
        stage_b = DIST / "stage-b"
        stage_artifact(
            second_binary,
            stage_b,
            commit=commit,
            toolchain=second_toolchain,
            working_tree_clean=clean,
            blockers=evidence["blockers"],
        )
        second = deterministic_tar(stage_b, DIST / f"{ARTIFACT_STEM}.second.tar.gz")
        comparison = compare_artifacts(first, second)
        evidence["reproducibility"] = comparison
        evidence["duration_seconds"] = round(time.monotonic() - started, 1)
        if not keep:
            for path in (DIST / "target-a", DIST / "target-b", stage_a, stage_b, DIST / f"{ARTIFACT_STEM}.second.tar.gz"):
                shutil.rmtree(path, ignore_errors=True) if path.is_dir() else path.unlink(missing_ok=True)
        if comparison["result"] != "REPRODUCIBLE":
            raise PackageError(PackageErrorCode.NOT_REPRODUCIBLE, json.dumps(comparison["differing_members"][:10]))
    except PackageError as error:
        if error.evidence is None:
            error.evidence = evidence
        raise
    return evidence


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeout-seconds", type=int, default=900)
    parser.add_argument("--require-clean", dest="require_clean", action="store_true", default=True)
    parser.add_argument("--allow-dirty", dest="require_clean", action="store_false")
    parser.add_argument("--keep", dest="keep", action="store_true", help="keep both target directories and stages")
    parser.add_argument("--no-keep", dest="keep", action="store_false", help="discard target directories and stages (default)")
    parser.add_argument("--evidence-dir", type=Path, default=EVIDENCE_DIR)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    arguments = parser.parse_args(argv)
    # The exact invocation is recorded, not inferred downstream: the
    # provenance predicate copies this string instead of restating it
    # from cleanliness plus the Makefile (ADR-0041 principle 1). Only
    # accepted flags are recorded, never paths: an absolute evidence dir
    # would leak the checkout location into the tracked provenance. The
    # string is recorded on every path, so a refused mint is
    # reconstructible from its evidence alone.
    invocation = " ".join(
        [
            "build_release_candidate.py",
            f"--timeout-seconds={arguments.timeout_seconds}",
            "--require-clean" if arguments.require_clean else "--allow-dirty",
            "--keep" if arguments.keep else "--no-keep",
        ]
    )
    evidence: dict
    try:
        evidence = build_candidate(timeout=arguments.timeout_seconds, require_clean=arguments.require_clean, keep=arguments.keep)
        evidence["result"] = "PASS"
        evidence["invocation"] = invocation
    except PackageError as error:
        evidence = error.evidence or {"contract": "s20-720-release-candidate-v1"}
        evidence["result"] = "FAIL"
        evidence["invocation"] = invocation
        evidence["failure"] = {"code": int(error.code), "symbol": error.symbol, "detail": error.detail[:500]}
    except (OSError, subprocess.TimeoutExpired, ValueError) as error:
        evidence = {"contract": "s20-720-release-candidate-v1", "result": "FAIL", "failure": {"code": int(PackageErrorCode.INTERNAL_INVARIANT), "symbol": "PACKAGE_INTERNAL_INVARIANT", "detail": str(error)[:500]}}
        evidence["invocation"] = invocation
    arguments.evidence_dir.mkdir(parents=True, exist_ok=True)
    (arguments.evidence_dir / "evidence.json").write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    summary = {key: evidence.get(key) for key in ("contract", "result", "artifact_sha256", "artifact_size_bytes", "member_count", "failure") if key in evidence}
    summary["reproducibility"] = evidence.get("reproducibility", {}).get("result")
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0 if evidence["result"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
