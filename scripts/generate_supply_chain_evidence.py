#!/usr/bin/env python3
"""Generate deterministic, offline S20-710 pre-release audit evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import history_ledger  # noqa: E402  (sibling module)

ROOT = Path(__file__).resolve().parents[1]
HISTORY_ANCHOR = "7804f665e0ee65de240c43fe3b56dc89cf7e9d80"
# The public repository's root commit: the sanitized continuation of the
# archived tip. HISTORY_ANCHOR is archived, so its blob scan cannot be
# re-run here; that result is carried from a frozen record (below) and the
# public history is scanned live from this anchor through HEAD.
# The parentless public root that continues the archived history, resolved by
# history_ledger (a commit cannot pin its own id).
PUBLIC_HISTORY_ANCHOR = history_ledger.public_root() or ""
PRE_PUBLIC_SCAN_PATH = ROOT / "evidence/history/supply-chain-pre-public-history-scan.json"
PRE_PUBLIC_SCAN_CONTRACT = "sley2.supply-chain.pre-public-history-scan.v1"
# Operator-approved root license material (S20-710 license decision
# 2026-09-14: Apache License, Version 2.0, Copyright 2026 Greyforge Labs).
# The digests pin the exact installed bytes: any added, removed, or
# modified root license file moves every workspace license disposition
# back to BLOCKED instead of passing open.
APPROVED_WORKSPACE_LICENSE = "Apache-2.0"
APPROVED_ROOT_LICENSE_FILES = {
    "LICENSE": "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30",
    "NOTICE": "e7151ea0ee545a9edec91ecf963acefec4d6c2cfd92aa6080b1afe517d5a5dfa",
}
MAX_SCANNED_BLOB_BYTES = 64 * 1024 * 1024
REGISTRY_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"

INVENTORY_PATH = ROOT / "evidence/security/T52/pre-release-inventory.json"
SECRET_SCAN_PATH = ROOT / "evidence/security/T54/secret-scan.json"
OUTPUT_PATHS = {INVENTORY_PATH.relative_to(ROOT), SECRET_SCAN_PATH.relative_to(ROOT)}

PYTHON_LICENSES = {
    ("blake3", "1.0.9"): "CC0-1.0 OR Apache-2.0",
    ("unicodedata2", "16.0.0"): "Apache-2.0",
}

PERMISSIVE_CARGO_LICENSES = {
    "BSD-2-Clause",
    "MIT OR Apache-2.0",
    "CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception",
    "CC0-1.0 OR MIT-0 OR Apache-2.0",
    "Unlicense OR MIT",
    "Apache-2.0 OR BSL-1.0",
    "Zlib OR Apache-2.0 OR MIT",
    "MIT OR Apache-2.0 OR Zlib",
    "(MIT OR Apache-2.0) AND Unicode-3.0",
    "MIT/Apache-2.0",
    # Ed25519 signing dependencies (2026-09-17): the dalek crates and subtle
    # are BSD-3-Clause, fiat-crypto adds the BSD-1-Clause alternative, and
    # ed25519/signature/zeroize spell the dual license in the other order.
    "BSD-3-Clause",
    "Apache-2.0 OR MIT",
    "MIT OR Apache-2.0 OR BSD-1-Clause",
}

SECRET_PATTERNS = {
    "PRIVATE_KEY_PEM": re.compile(rb"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
    "AWS_ACCESS_KEY": re.compile(rb"(?:AKIA|ASIA)[0-9A-Z]{16}"),
    "GITHUB_TOKEN": re.compile(rb"(?:gh[pousr]_[A-Za-z0-9]{36,255}|github_pat_[A-Za-z0-9_]{40,255})"),
    "GITLAB_PAT": re.compile(rb"glpat-[A-Za-z0-9_-]{20,}"),
    # No PGP-armour pattern: a header-only shape cannot be stated in audit
    # discussion without self-firing, and secret findings have no
    # disposition path (any hit BLOCKEDs the scan with no allowlist flow),
    # so landing it would permanently BLOCK on the repo's own audit
    # verdicts. Revisit with a finding-disposition queue, not a pattern.
    "SENDGRID_KEY": re.compile(rb"SG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}"),
    "SLACK_TOKEN": re.compile(rb"xox[baprs]-[A-Za-z0-9-]{20,}"),
    "OPENAI_KEY": re.compile(rb"sk-[A-Za-z0-9]{20,}"),
    "OPENAI_PROJECT_KEY": re.compile(rb"sk-proj-[A-Za-z0-9_-]{20,}"),
    "OPENAI_ADMIN_KEY": re.compile(rb"sk-admin-[A-Za-z0-9_-]{20,}"),
    "OPENAI_SVCACCT_KEY": re.compile(rb"sk-svcacct-[A-Za-z0-9_-]{20,}"),
    "ANTHROPIC_KEY": re.compile(rb"sk-ant-[A-Za-z0-9_-]{20,}"),
    "XAI_KEY": re.compile(rb"xai-[A-Za-z0-9]{20,}"),
    "HUGGINGFACE_TOKEN": re.compile(rb"hf_[A-Za-z0-9]{20,}"),
    "PYPI_TOKEN": re.compile(rb"pypi-[A-Za-z0-9_-]{40,}"),
    "NPM_TOKEN": re.compile(rb"npm_[A-Za-z0-9]{20,}"),
    "DISCORD_BOT_TOKEN": re.compile(rb"[A-Za-z0-9_-]{24}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{27}"),
    "GCP_SERVICE_ACCOUNT": re.compile(rb'"type"\s*:\s*"service_account"'),
    "JWT_TOKEN": re.compile(rb"eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+"),
    "STRIPE_LIVE_KEY": re.compile(rb"(?:sk|rk)_live_[A-Za-z0-9]{20,}"),
    "GOOGLE_API_KEY": re.compile(rb"AIza[0-9A-Za-z_-]{35}"),
    "URL_CREDENTIAL": re.compile(rb"https?://[^/\s:@]{1,128}:[^/\s@]{1,256}@"),
}

REQUIRED_IGNORE_PATTERNS = {
    ".env",
    ".env.*",
    "*.pem",
    "*.key",
    "*.p12",
    "*.pfx",
    "credentials/",
    "secrets/",
    ".aws/",
    ".gnupg/",
}


def run(command: list[str], *, input_bytes: bytes | None = None) -> bytes:
    completed = subprocess.run(
        command,
        cwd=ROOT,
        input=input_bytes,
        check=False,
        capture_output=True,
    )
    if completed.returncode != 0:
        message = completed.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(f"deterministic command failed: {command[0]}: {message}")
    return completed.stdout


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical_json(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n").encode("utf-8")


def package_ref(ecosystem: str, name: str, version: str, *, workspace: bool = False) -> str:
    suffix = "?workspace=true" if workspace else ""
    return f"pkg:{ecosystem}/{name}@{version}{suffix}"


def license_disposition(
    expression: str | None, *, workspace: bool, root_ok: bool = False
) -> tuple[str, str | None]:
    if workspace:
        if expression != APPROVED_WORKSPACE_LICENSE:
            return "BLOCKED_LICENSE_METADATA_MISMATCH", expression
        if not root_ok:
            return "BLOCKED_MISSING_APPROVED_ROOT_LICENSE_TEXT", expression
        return "APPROVED_OPERATOR_APACHE_2_0_ROOT_LICENSE", expression
    if expression in PERMISSIVE_CARGO_LICENSES:
        normalized = (
            "MIT OR Apache-2.0"
            if expression in {"MIT/Apache-2.0", "Apache-2.0 OR MIT"}
            else expression
        )
        return "DECLARED_PERMISSIVE_PRE_RELEASE_REVIEW", normalized
    return "BLOCKED_UNREVIEWED_LICENSE_EXPRESSION", expression


def cargo_inventory(
    *, root_ok: bool
) -> tuple[list[dict[str, Any]], list[dict[str, str]], list[str]]:
    metadata = json.loads(
        run(["cargo", "metadata", "--offline", "--locked", "--format-version", "1"])
    )
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    lock_entries = {
        (package["name"], package["version"], package.get("source")): package
        for package in lock["package"]
    }
    packages: list[dict[str, Any]] = []
    id_to_ref: dict[str, str] = {}
    blockers: list[str] = []
    for package in metadata["packages"]:
        workspace = package.get("source") is None
        source = "workspace" if workspace else package["source"]
        reference = package_ref("cargo", package["name"], package["version"], workspace=workspace)
        id_to_ref[package["id"]] = reference
        lock_entry = lock_entries.get((package["name"], package["version"], package.get("source")))
        checksum = None if workspace else (lock_entry or {}).get("checksum")
        if not workspace:
            if source != REGISTRY_SOURCE:
                blockers.append(f"cargo-source:{package['name']}@{package['version']}")
            if not isinstance(checksum, str) or re.fullmatch(r"[0-9a-f]{64}", checksum) is None:
                blockers.append(f"cargo-checksum:{package['name']}@{package['version']}")
        disposition, normalized = license_disposition(
            package.get("license"), workspace=workspace, root_ok=root_ok
        )
        if disposition.startswith("BLOCKED_") and not workspace:
            blockers.append(f"cargo-license:{package['name']}@{package['version']}:{disposition}")
        packages.append(
            {
                "bom_ref": reference,
                "checksum_sha256": checksum,
                "ecosystem": "cargo",
                "license_declared": package.get("license"),
                "license_evidence": "cargo-metadata-declared-expression",
                "license_normalized_for_review": normalized,
                "license_disposition": disposition,
                "name": package["name"],
                "source": source,
                "version": package["version"],
                "workspace": workspace,
            }
        )
    relationships: list[dict[str, str]] = []
    for node in metadata["resolve"]["nodes"]:
        for dependency in node["dependencies"]:
            relationships.append({"from": id_to_ref[node["id"]], "to": id_to_ref[dependency]})
    packages.sort(key=lambda item: item["bom_ref"])
    relationships = sorted({(item["from"], item["to"]) for item in relationships})
    return packages, [{"from": source, "to": target} for source, target in relationships], blockers


def python_inventory(
    *, root_ok: bool
) -> tuple[list[dict[str, Any]], list[dict[str, str]], list[str]]:
    run(
        [
            "uv",
            "lock",
            "--check",
            "--offline",
            "--no-python-downloads",
            "--project",
            "oracle/scb1",
        ]
    )
    lock = tomllib.loads((ROOT / "oracle/scb1/uv.lock").read_text(encoding="utf-8"))
    project = tomllib.loads((ROOT / "oracle/scb1/pyproject.toml").read_text(encoding="utf-8"))
    packages: list[dict[str, Any]] = []
    blockers: list[str] = []
    name_to_ref: dict[str, str] = {}
    for package in lock["package"]:
        workspace = package.get("source", {}).get("editable") == "."
        name = package["name"]
        version = package["version"]
        reference = package_ref("pypi", name, version, workspace=workspace)
        name_to_ref[name] = reference
        if workspace:
            declared = project["project"].get("license")
            disposition, normalized = license_disposition(declared, workspace=True, root_ok=root_ok)
            license_evidence = "pyproject-declared-expression"
            hashes: list[str] = []
            source = "workspace:oracle/scb1"
        else:
            declared = PYTHON_LICENSES.get((name, version))
            normalized = declared
            license_evidence = "curated-offline-pre-release-review"
            disposition = (
                "DECLARED_PERMISSIVE_PRE_RELEASE_REVIEW"
                if declared in {"CC0-1.0 OR Apache-2.0", "Apache-2.0"}
                else "BLOCKED_UNREVIEWED_LICENSE_EXPRESSION"
            )
            hashes = []
            if "sdist" in package:
                hashes.append(package["sdist"]["hash"])
            hashes.extend(wheel["hash"] for wheel in package.get("wheels", []))
            hashes.sort()
            source = package.get("source", {}).get("registry")
            if source != "https://pypi.org/simple":
                blockers.append(f"python-source:{name}@{version}")
            if not hashes or any(re.fullmatch(r"sha256:[0-9a-f]{64}", value) is None for value in hashes):
                blockers.append(f"python-hash:{name}@{version}")
        if disposition.startswith("BLOCKED_") and not workspace:
            blockers.append(f"python-license:{name}@{version}:{disposition}")
        packages.append(
            {
                "artifact_hashes": hashes,
                "bom_ref": reference,
                "ecosystem": "pypi",
                "license_declared": declared,
                "license_evidence": license_evidence,
                "license_normalized_for_review": normalized,
                "license_disposition": disposition,
                "name": name,
                "source": source,
                "version": version,
                "workspace": workspace,
            }
        )
    relationships: list[dict[str, str]] = []
    for package in lock["package"]:
        source = name_to_ref[package["name"]]
        for dependency in package.get("dependencies", []):
            target_name = dependency["name"] if isinstance(dependency, dict) else dependency
            relationships.append({"from": source, "to": name_to_ref[target_name]})
    packages.sort(key=lambda item: item["bom_ref"])
    relationships.sort(key=lambda item: (item["from"], item["to"]))
    return packages, relationships, blockers


def license_text_files() -> list[str]:
    names = []
    for path in ROOT.iterdir():
        if path.is_file() and re.match(r"(?i)^(license|copying|notice)(\..*)?$", path.name):
            names.append(path.name)
    return sorted(names)


def root_license_digests() -> dict[str, str]:
    """sha256 of every recognized root license file: the checker pins the
    exact approved bytes, so an edited LICENSE or NOTICE breaks both the
    disposition and the deterministic-regeneration check."""
    return {name: sha256((ROOT / name).read_bytes()) for name in license_text_files()}


def candidate_files() -> list[Path]:
    """Tracked candidate files only, so the recorded manifest is commit-bound.

    Untracked working-tree files are scanned for secrets separately (they
    can carry credentials too) but never enter the recorded manifest: a
    manifest covering untracked state is not reproducible from the commit
    and drifts on any clean checkout.
    """
    raw = run(["git", "ls-files", "--cached", "-z"])
    paths = []
    for value in raw.split(b"\0"):
        if not value:
            continue
        relative = Path(os.fsdecode(value))
        if relative in OUTPUT_PATHS:
            continue
        paths.append(relative)
    return sorted(paths, key=lambda path: path.as_posix().encode("utf-8"))


def untracked_files() -> list[Path]:
    """Non-ignored untracked files: secret-scanned, never manifested."""
    raw = run(["git", "ls-files", "--others", "--exclude-standard", "-z"])
    paths = []
    for value in raw.split(b"\0"):
        if not value:
            continue
        relative = Path(os.fsdecode(value))
        if relative in OUTPUT_PATHS:
            continue
        paths.append(relative)
    return sorted(paths, key=lambda path: path.as_posix().encode("utf-8"))


def scan_blob(data: bytes) -> list[str]:
    return sorted(name for name, pattern in SECRET_PATTERNS.items() if pattern.search(data))


def current_candidate_scan() -> tuple[list[dict[str, str]], list[str], int, int, str]:
    findings: list[dict[str, str]] = []
    blockers: list[str] = []
    count = 0
    byte_count = 0
    manifest = hashlib.sha256()
    for relative in candidate_files():
        path = ROOT / relative
        mode = path.lstat().st_mode
        if stat.S_ISLNK(mode) or not stat.S_ISREG(mode):
            blockers.append(f"candidate-nonregular:{relative.as_posix()}")
            continue
        size = path.stat().st_size
        if size > MAX_SCANNED_BLOB_BYTES:
            blockers.append(f"candidate-oversize:{relative.as_posix()}:{size}")
            continue
        data = path.read_bytes()
        digest = sha256(data)
        encoded_path = relative.as_posix().encode("utf-8")
        manifest.update(len(encoded_path).to_bytes(8, "big"))
        manifest.update(encoded_path)
        manifest.update(bytes.fromhex(digest))
        count += 1
        byte_count += len(data)
        for pattern in scan_blob(data):
            findings.append({"pattern": pattern, "path": relative.as_posix(), "scope": "candidate"})
    return findings, blockers, count, byte_count, manifest.hexdigest()


def untracked_candidate_scan() -> tuple[list[dict[str, str]], list[str], int, int]:
    """Secret-scan non-ignored untracked files without manifesting them."""
    findings: list[dict[str, str]] = []
    blockers: list[str] = []
    count = 0
    byte_count = 0
    for relative in untracked_files():
        path = ROOT / relative
        mode = path.lstat().st_mode
        if stat.S_ISLNK(mode) or not stat.S_ISREG(mode):
            blockers.append(f"untracked-nonregular:{relative.as_posix()}")
            continue
        size = path.stat().st_size
        if size > MAX_SCANNED_BLOB_BYTES:
            blockers.append(f"untracked-oversize:{relative.as_posix()}:{size}")
            continue
        data = path.read_bytes()
        count += 1
        byte_count += len(data)
        for pattern in scan_blob(data):
            findings.append({"pattern": pattern, "path": relative.as_posix(), "scope": "untracked"})
    return findings, blockers, count, byte_count


def history_objects(*revisions: str) -> tuple[list[str], dict[str, list[str]]]:
    lines = run(["git", "rev-list", "--objects", *revisions]).decode("utf-8").splitlines()
    object_paths: dict[str, list[str]] = {}
    for line in lines:
        fields = line.split(" ", 1)
        object_paths.setdefault(fields[0], [])
        if len(fields) == 2:
            object_paths[fields[0]].append(fields[1])
    return sorted(object_paths), object_paths


def history_scan(*revisions: str) -> tuple[list[dict[str, str]], list[str], int, int]:
    object_ids, object_paths = history_objects(*revisions)
    if not object_ids:
        # `git cat-file --batch` answers a bare newline with " missing";
        # an empty range (HEAD at the anchor) has nothing to scan.
        return [], [], 0, 0
    batch = run(["git", "cat-file", "--batch"], input_bytes=("\n".join(object_ids) + "\n").encode())
    position = 0
    findings: list[dict[str, str]] = []
    blockers: list[str] = []
    blobs = 0
    byte_count = 0
    for expected_object in object_ids:
        newline = batch.find(b"\n", position)
        if newline < 0:
            raise RuntimeError("truncated git cat-file batch header")
        header = batch[position:newline].decode("ascii")
        position = newline + 1
        fields = header.split()
        if len(fields) != 3 or fields[0] != expected_object:
            raise RuntimeError("unexpected git cat-file batch header")
        object_type, size = fields[1], int(fields[2])
        data = batch[position : position + size]
        position += size
        if batch[position : position + 1] != b"\n":
            raise RuntimeError("truncated git cat-file batch payload")
        position += 1
        if object_type != "blob":
            continue
        if size > MAX_SCANNED_BLOB_BYTES:
            blockers.append(f"history-oversize:{expected_object}:{size}")
            continue
        blobs += 1
        byte_count += size
        patterns = scan_blob(data)
        for pattern in patterns:
            paths = sorted(object_paths.get(expected_object) or ["<unresolved-history-path>"])
            for path in paths:
                findings.append(
                    {
                        "blob_oid": expected_object,
                        "path": path,
                        "pattern": pattern,
                        "scope": "history",
                    }
                )
    if position != len(batch):
        raise RuntimeError("surplus git cat-file batch bytes")
    return findings, blockers, blobs, byte_count


def pattern_set_sha256() -> str:
    """Digest of the exact scan rules: names, regex bytes, and blob bound.

    The frozen pre-public result vouches only for the rules it ran with, so
    any pattern change fails closed instead of inheriting that coverage.
    """
    rules = hashlib.sha256()
    for name in sorted(SECRET_PATTERNS):
        for part in (name.encode("utf-8"), SECRET_PATTERNS[name].pattern):
            rules.update(len(part).to_bytes(8, "big"))
            rules.update(part)
    rules.update(MAX_SCANNED_BLOB_BYTES.to_bytes(8, "big"))
    return rules.hexdigest()


def pre_public_history_scan() -> tuple[list[dict[str, str]], list[str], int, int]:
    """The frozen blob scan of the archived history through HISTORY_ANCHOR.

    Those blobs exist only in the archive bundle, so the result recorded at
    the archived tip is carried here once and bound to the ledger (anchor
    archived and continued by the public root, same bundle digest) and to
    the current scan rules. Anything that does not bind raises: fail closed.
    """
    try:
        record = json.loads(PRE_PUBLIC_SCAN_PATH.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RuntimeError(f"pre-public history scan record unreadable: {error}") from error
    if not isinstance(record, dict) or record.get("contract") != PRE_PUBLIC_SCAN_CONTRACT:
        raise RuntimeError("pre-public history scan record has an unexpected contract")
    if record.get("history_anchor_commit") != HISTORY_ANCHOR:
        raise RuntimeError("pre-public history scan record names another anchor")
    if not history_ledger.is_archived(HISTORY_ANCHOR):
        raise RuntimeError("history anchor is not an archived pre-public commit")
    if not history_ledger.is_ancestor(HISTORY_ANCHOR, PUBLIC_HISTORY_ANCHOR):
        raise RuntimeError("history anchor is not an ancestor of the public history anchor")
    if record.get("archive_sha256") != history_ledger.ledger().get("archive", {}).get("sha256"):
        raise RuntimeError("pre-public history scan record names another archive bundle")
    if record.get("source_tip_commit") != history_ledger.archived_tip():
        raise RuntimeError("pre-public history scan record names another archived tip")
    if (
        record.get("patterns") != sorted(SECRET_PATTERNS)
        or record.get("pattern_set_sha256") != pattern_set_sha256()
    ):
        raise RuntimeError("secret patterns changed since the frozen pre-public history scan")
    blobs, byte_count = record.get("history_blobs_scanned"), record.get("history_bytes_scanned")
    if type(blobs) is not int or type(byte_count) is not int or blobs <= 0 or byte_count <= 0:
        raise RuntimeError("pre-public history scan record has invalid coverage counters")
    findings, blockers = record.get("findings"), record.get("blockers")
    if not isinstance(findings, list) or not isinstance(blockers, list):
        raise RuntimeError("pre-public history scan record findings/blockers must be lists")
    for entry in findings:
        if (
            not isinstance(entry, dict)
            or set(entry) != {"blob_oid", "path", "pattern", "scope"}
            or entry["scope"] != "history"
        ):
            raise RuntimeError("pre-public history scan record carries a malformed finding")
    if not all(isinstance(blocker, str) for blocker in blockers):
        raise RuntimeError("pre-public history scan record carries a malformed blocker")
    return findings, blockers, blobs, byte_count


def public_history_scan() -> tuple[list[dict[str, str]], list[str], int, int]:
    """Live scan of every blob reachable from HEAD in the public history.

    Only the anchor's blobs are counted: the record is committed into the
    history after the anchor, so counting that range would change the
    record it is written into. Every blob after the anchor is still
    scanned, and any finding or oversize blob there lands in the record.
    """
    if not history_ledger.is_ancestor(PUBLIC_HISTORY_ANCHOR, "HEAD"):
        raise RuntimeError("public history anchor is not an ancestor of HEAD")
    if run(["git", "rev-list", "--parents", "-n", "1", PUBLIC_HISTORY_ANCHOR]).split() != [
        PUBLIC_HISTORY_ANCHOR.encode("ascii")
    ]:
        raise RuntimeError("public history anchor is not the parentless public root")
    findings, blockers, blobs, byte_count = history_scan(PUBLIC_HISTORY_ANCHOR)
    later_findings, later_blockers, _, _ = history_scan("HEAD", f"^{PUBLIC_HISTORY_ANCHOR}")
    for entry in findings + later_findings:
        entry["scope"] = "public-history"
    return findings + later_findings, blockers + later_blockers, blobs, byte_count


def build_outputs() -> dict[Path, bytes]:
    licenses = license_text_files()
    root_digests = root_license_digests()
    root_ok = root_digests == APPROVED_ROOT_LICENSE_FILES
    cargo_packages, cargo_relationships, cargo_blockers = cargo_inventory(root_ok=root_ok)
    python_packages, python_relationships, python_blockers = python_inventory(root_ok=root_ok)
    blockers = sorted(set(cargo_blockers + python_blockers))
    if not licenses:
        blockers.append("workspace-license-text:missing-operator-approved-root-license")
    elif not root_ok:
        blockers.append("workspace-license-text:unapproved-root-license-text-present")
    ignore_lines = {
        line.strip()
        for line in (ROOT / ".gitignore").read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    }
    missing_ignores = sorted(REQUIRED_IGNORE_PATTERNS - ignore_lines)
    blockers.extend(f"secret-ignore-pattern:missing:{pattern}" for pattern in missing_ignores)
    for sentinel in (".env", "id_rsa.pem", "credentials/x", "secrets/x", ".aws/x", ".gnupg/x"):
        ignored = subprocess.run(
            ["git", "check-ignore", "-q", sentinel],
            cwd=ROOT,
            capture_output=True,
            check=False,
        )
        if ignored.returncode != 0:
            blockers.append(f"secret-ignore-pattern:ineffective:{sentinel}")
    inventory = {
        "blockers": sorted(set(blockers)),
        "cargo_lock_sha256": sha256((ROOT / "Cargo.lock").read_bytes()),
        "contract": "s20-710-pre-release-inventory-v1",
        "full_release_sbom": False,
        "history_anchor_commit": HISTORY_ANCHOR,
        "license_text_files": licenses,
        "root_license_sha256": root_digests.get("LICENSE"),
        "notice_sha256": root_digests.get("NOTICE"),
        "packages": cargo_packages + python_packages,
        "relationships": sorted(
            cargo_relationships + python_relationships,
            key=lambda item: (item["from"], item["to"]),
        ),
        "result": "BLOCKED" if blockers else "PASS",
        "sources": {
            "cargo_registry": REGISTRY_SOURCE,
            "python_registry": "https://pypi.org/simple",
        },
        "python_lock_freshness": "uv lock --check --offline --no-python-downloads:PASS",
        "standards_sbom_deferred": bool(blockers),
        "uv_lock_sha256": sha256((ROOT / "oracle/scb1/uv.lock").read_bytes()),
    }

    current_findings, current_blockers, current_files, current_bytes, candidate_digest = (
        current_candidate_scan()
    )
    untracked_findings, untracked_blockers, untracked_file_count, untracked_byte_count = (
        untracked_candidate_scan()
    )
    history_findings, history_blockers, history_blobs, history_bytes = pre_public_history_scan()
    public_findings, public_blockers, public_blobs, public_bytes = public_history_scan()
    secret_findings = sorted(
        current_findings + untracked_findings + history_findings + public_findings,
        key=lambda item: (item["scope"], item["path"], item["pattern"], item.get("blob_oid", "")),
    )
    secret_blockers = sorted(
        set(current_blockers + untracked_blockers + history_blockers + public_blockers)
    )
    if secret_findings:
        secret_blockers.append("high-confidence-secret-findings-require-disposition")
    secret_scan = {
        "blockers": secret_blockers,
        "candidate_bytes_scanned": current_bytes,
        "candidate_file_manifest_sha256": candidate_digest,
        "candidate_files_scanned": current_files,
        "contract": "s20-710-secret-scan-v1",
        "findings": secret_findings,
        "history_anchor_commit": HISTORY_ANCHOR,
        "history_blobs_scanned": history_blobs,
        "history_bytes_scanned": history_bytes,
        "limitations": [
            "high-confidence patterns only; no entropy or semantic credential validation",
            "history is frozen through the pre-audit anchor; later release audit must re-anchor",
            "the pre-public history through the anchor is archived off-repository; its scan result is carried from a frozen record bound to the history ledger and to these exact patterns, not re-scanned",
            "public history is scanned live from the public root anchor through HEAD; only the anchor's blobs are counted, because later commits include this record",
            "generated T52/T54 reports are excluded from their own candidate scan",
            "the recorded manifest covers tracked files only and is reproducible from the commit; untracked files are scanned for secrets and counted separately, never manifested",
            "byte-regex over raw bytes only; compressed or encoded content (archives, base64) is opaque to the scan",
            "no ignored local files, reflogs, remotes, provider stores, or external secret managers scanned",
        ],
        "matched_secret_values_emitted": False,
        "patterns": sorted(SECRET_PATTERNS),
        "pre_public_history_record": PRE_PUBLIC_SCAN_PATH.relative_to(ROOT).as_posix(),
        "public_history_anchor_blobs_scanned": public_blobs,
        "public_history_anchor_bytes_scanned": public_bytes,
        "public_history_anchor_commit": PUBLIC_HISTORY_ANCHOR,
        "result": "BLOCKED" if secret_blockers else "PASS_NO_HIGH_CONFIDENCE_FINDINGS",
        "untracked_bytes_scanned": untracked_byte_count,
        "untracked_files_scanned": untracked_file_count,
    }
    return {
        INVENTORY_PATH: canonical_json(inventory),
        SECRET_SCAN_PATH: canonical_json(secret_scan),
    }


# Working-tree facts the tracked T54 record carries for legibility but that a
# commit-bound comparison must not depend on: an untracked, non-ignored file
# (a Council transcript awaiting its records commit, a scratch note) changes
# these counters without changing the tracked candidate, so `--check`
# compares the record with them masked (Ariadne P3 / Nabu P4 at 178873d7).
WORKING_TREE_FIELDS = ("untracked_bytes_scanned", "untracked_files_scanned")


def commit_bound_view(payload: bytes) -> bytes:
    """The record with its working-tree-only counters masked for comparison."""
    try:
        document = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError):
        return payload
    if not isinstance(document, dict):
        return payload
    for field in WORKING_TREE_FIELDS:
        if field in document:
            document[field] = None
    return canonical_json(document)


def tracked_canonical_form(payload: bytes) -> bytes:
    """The tracked bytes re-canonicalized with the counters masked.

    Equal to `commit_bound_view(payload)` exactly when the tracked record is
    the generator's own canonical form; a reformatted record differs.
    """
    try:
        document = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError):
        return b""
    if not isinstance(document, dict) or canonical_json(document) != payload:
        return b""
    return commit_bound_view(payload)


def working_tree_counters_clear(payload: bytes) -> bool:
    """A tracked record carries zero untracked-file counters (clean-tree mint)."""
    try:
        document = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError):
        return False
    if not isinstance(document, dict):
        return False
    # The secret scan always emits both counters, so for it a counter is
    # clear only when present and the integer zero: an absent field, `false`
    # or `0.0` is not a counted zero (Vulcan P4 at c04539b9..76ae15ab). A
    # record that carries no working-tree counters (the T52 inventory) has
    # nothing to clear, but a stray counter in it is judged the same way.
    scan = document.get("contract") == "s20-710-secret-scan-v1"
    return all(
        (type(document.get(field)) is int and document.get(field) == 0)
        or (not scan and field not in document)
        for field in WORKING_TREE_FIELDS
    )


def record_drifted(tracked: bytes, expected: bytes) -> bool:
    """The `--check` decision for one tracked record against the generator."""
    return (
        commit_bound_view(tracked) != commit_bound_view(expected)
        or commit_bound_view(tracked) != tracked_canonical_form(tracked)
        or not working_tree_counters_clear(tracked)
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    outputs = build_outputs()
    drift: list[str] = []
    for path, expected in outputs.items():
        if arguments.check:
            # Commit-bound comparison with the working-tree counters masked;
            # the tracked bytes must still be exactly the generator's
            # canonical form (no reformatted or hand-edited record passes),
            # and a record committed from a dirty tree — nonzero untracked
            # counters — is drift in its own right (Vulcan P4 at 92fa6646).
            tracked = path.read_bytes() if path.exists() else None
            if tracked is None or record_drifted(tracked, expected):
                drift.append(path.relative_to(ROOT).as_posix())
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(expected)
    if drift:
        print(json.dumps({"result": "FAIL", "drift": drift}, sort_keys=True))
        return 1
    if arguments.check:
        print(json.dumps({"result": "PASS", "outputs": len(outputs)}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
