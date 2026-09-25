#!/usr/bin/env python3
"""Generate THIRD_PARTY_LICENSES from the locked Cargo dependency set.

The release binary is statically linked, so it redistributes code from the
third-party crates in Cargo.lock. Their licenses (BSD-2-Clause and
BSD-3-Clause among them) require the license text and copyright notice to
travel with a binary. This script writes one plain-text file that reproduces
every license, copyright, notice and authors file each locked registry crate
ships, with the crate's declared license expression and its Cargo.toml
authors.

The output is deterministic: packages sort by name and version, files sort by
name, line endings become LF, trailing whitespace is dropped, and no local
path is ever written (crates are named by their registry source and Cargo.lock
checksum). The crate sources come from the local cargo
registry through `cargo metadata --locked --offline`; run `cargo fetch
--locked` first on a fresh cache. A crate that ships no license file stops the
run: this script never substitutes a text the crate did not ship.

`--check` compares the tracked file with a fresh rendering and fails on any
difference. `--check-packages` needs no cargo cache: it compares the file's
package index with Cargo.lock only.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT_NAME = "THIRD_PARTY_LICENSES"
OUTPUT = ROOT / OUTPUT_NAME
GENERATOR = "scripts/generate_third_party_licenses.py"
# Top-level files a crate ships to carry its license terms, copyright
# notices, NOTICE text, or the authors list its copyright line refers to.
LICENSE_FILE = re.compile(r"(?i)^(licen[cs]e|copying|copyright|notice|unlicense|authors)([-_.].*)?$")
RULE = "=" * 80
THIN_RULE = "-" * 80
INDEX_LINE = re.compile(r"^  (\S+) (\S+)$")


class LicenseError(Exception):
    pass


def lock_packages(lock_text: str) -> dict[tuple[str, str], dict]:
    """Every third-party package in Cargo.lock: (name, version) -> entry."""
    lock = tomllib.loads(lock_text)
    return {
        (package["name"], package["version"]): package
        for package in lock.get("package", [])
        if package.get("source")
    }


def cargo_metadata(root: Path) -> dict:
    completed = subprocess.run(
        [
            "cargo", "metadata", "--locked", "--offline", "--format-version", "1",
            "--manifest-path", str(root / "Cargo.toml"),
        ],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        raise LicenseError(
            "cargo metadata --locked --offline failed (run `cargo fetch --locked` first): "
            + completed.stderr.strip()[-400:]
        )
    return json.loads(completed.stdout)


def license_files(directory: Path, declared_file: str | None) -> list[tuple[str, str]]:
    """The crate's license files as (name, text), sorted by name."""
    names = {
        path.name
        for path in directory.iterdir()
        if path.is_file() and LICENSE_FILE.match(path.name)
    }
    if declared_file:
        declared = (directory / declared_file).resolve()
        if not declared.is_relative_to(directory.resolve()) or not declared.is_file():
            raise LicenseError(f"license-file outside the crate or missing: {declared_file}")
        names.add(declared.relative_to(directory.resolve()).as_posix())
    files = []
    for name in sorted(names):
        try:
            text = (directory / name).read_bytes().decode("utf-8")
        except UnicodeDecodeError as error:
            raise LicenseError(f"{directory.name}/{name} is not UTF-8") from error
        files.append((name, text))
    return files


def collect(root: Path, metadata: dict | None = None) -> list[dict]:
    """Every locked third-party package with its shipped license files."""
    metadata = metadata if metadata is not None else cargo_metadata(root)
    locked = lock_packages((root / "Cargo.lock").read_text(encoding="utf-8"))
    packages = []
    for package in metadata["packages"]:
        if package.get("source") is None:
            continue
        key = (package["name"], package["version"])
        if key not in locked:
            raise LicenseError(f"{key[0]} {key[1]} is not in Cargo.lock")
        files = license_files(Path(package["manifest_path"]).parent, package.get("license_file"))
        if not files:
            raise LicenseError(f"{key[0]} {key[1]} ships no license file")
        packages.append({
            "name": package["name"],
            "version": package["version"],
            "license": package.get("license") or "",
            "source": package["source"],
            "checksum": locked[key].get("checksum", ""),
            "repository": package.get("repository") or "",
            "authors": list(package.get("authors") or []),
            "files": files,
        })
    found = {(package["name"], package["version"]) for package in packages}
    if found != set(locked):
        missing = sorted(f"{name} {version}" for name, version in set(locked) - found)
        raise LicenseError(f"Cargo.lock packages missing from cargo metadata: {missing}")
    return sorted(packages, key=lambda package: (package["name"], package["version"]))


def render(packages: list[dict]) -> bytes:
    """The THIRD_PARTY_LICENSES bytes for the collected packages."""
    lines = [
        "Third-party licenses for the sley binary",
        "",
        "The sley binary is statically linked. This file reproduces the",
        "license, copyright, notice and authors files shipped by every",
        "third-party crate in Cargo.lock, the locked dependency set the binary",
        "is built from. Only line endings and trailing whitespace are",
        "normalized. Some of these crates (build scripts and procedural",
        "macros) run only at build time and are not linked into the binary;",
        "they are included for completeness. Sley itself is licensed under the",
        "Apache License, Version 2.0 (see LICENSE and NOTICE).",
        "",
        f"Generated by {GENERATOR}. Do not edit by hand.",
        "",
        f"Packages ({len(packages)}):",
    ]
    lines += [f"  {package['name']} {package['version']}" for package in packages]
    for package in packages:
        lines += [
            "",
            RULE,
            f"{package['name']} {package['version']}",
            f"License: {package['license'] or '(not declared)'}",
            f"Source: {package['source']}",
        ]
        if package["checksum"]:
            lines.append(f"Checksum (sha256): {package['checksum']}")
        if package["repository"]:
            lines.append(f"Repository: {package['repository']}")
        if package["authors"]:
            lines.append("Authors (Cargo.toml):")
            lines += [f"  {author}".rstrip() for author in package["authors"]]
        for name, text in package["files"]:
            # Line endings become LF and trailing whitespace is dropped, so
            # the file is stable and clean under `git diff --check`.
            body = [line.rstrip() for line in text.replace("\r\n", "\n").split("\n")]
            while body and not body[-1]:
                body.pop()
            lines += ["", THIN_RULE, f"File: {name}", THIN_RULE, ""] + body
    return ("\n".join(lines) + "\n").encode("utf-8")


def indexed_packages(data: bytes) -> list[tuple[str, str]]:
    """The (name, version) index at the head of a rendered file."""
    text = data.decode("utf-8")
    lines = text.splitlines()
    try:
        start = next(index for index, line in enumerate(lines) if line.startswith("Packages ("))
    except StopIteration as error:
        raise LicenseError("package index missing") from error
    count = int(lines[start][len("Packages ("):].rstrip("):"))
    entries = []
    for line in lines[start + 1 : start + 1 + count]:
        match = INDEX_LINE.match(line)
        if match is None:
            raise LicenseError(f"malformed index line: {line!r}")
        entries.append((match.group(1), match.group(2)))
    if len(entries) != count:
        raise LicenseError("package index shorter than its count")
    return entries


def package_problems(root: Path, data: bytes) -> list[str]:
    """Offline drift between the file's package index and Cargo.lock."""
    locked = sorted(lock_packages((root / "Cargo.lock").read_text(encoding="utf-8")))
    try:
        indexed = indexed_packages(data)
    except (LicenseError, ValueError, UnicodeDecodeError) as error:
        return [f"index:{error}"]
    problems = []
    if indexed != sorted(indexed):
        problems.append("index:not-sorted")
    for name, version in sorted(set(locked) - set(indexed)):
        problems.append(f"missing:{name} {version}")
    for name, version in sorted(set(indexed) - set(locked)):
        problems.append(f"unlocked:{name} {version}")
    return problems


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="fail when the tracked file differs from a fresh rendering")
    mode.add_argument("--check-packages", action="store_true", help="offline: fail when the file's package index differs from Cargo.lock")
    arguments = parser.parse_args(argv)
    if arguments.check_packages:
        problems = package_problems(ROOT, OUTPUT.read_bytes()) if OUTPUT.is_file() else [f"missing:{OUTPUT_NAME}"]
        print(json.dumps({"file": OUTPUT_NAME, "problems": problems, "result": "FAIL" if problems else "PASS"}, indent=2))
        return 1 if problems else 0
    try:
        rendered = render(collect(ROOT))
    except LicenseError as error:
        print(json.dumps({"file": OUTPUT_NAME, "error": str(error), "result": "FAIL"}, indent=2))
        return 1
    if arguments.check:
        current = OUTPUT.read_bytes() if OUTPUT.is_file() else b""
        drift = current != rendered
        print(json.dumps({"file": OUTPUT_NAME, "drift": drift, "result": "FAIL" if drift else "PASS"}, indent=2))
        return 1 if drift else 0
    OUTPUT.write_bytes(rendered)
    print(json.dumps({"file": OUTPUT_NAME, "packages": len(indexed_packages(rendered)), "result": "PASS"}, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
