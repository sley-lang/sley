#!/usr/bin/env python3
"""Build and run the S20-250 full complete-root judgment persistent libFuzzer target."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / "evidence/runtime/s20-700-complete-root-libfuzzer"
CORPUS = RUNTIME / "corpus"
ARTIFACTS = RUNTIME / "artifacts"
EVIDENCE = RUNTIME / "evidence.json"
TARGET_DIR = RUNTIME / "target"
FUZZER = TARGET_DIR / "release/complete_root_judgment"
FIXTURE = ROOT / "conformance/complete-entity-impact/v1/accepted.json"
REJECTED = ROOT / "conformance/complete-entity-impact/v1/rejected.json"
CLANG = "clang-18"
RUST_TOOLCHAIN = "nightly-2026-02-27"
LIBFUZZER = Path("/usr/lib/llvm-18/lib/clang/18/lib/linux/libclang_rt.fuzzer-x86_64.a")
MAX_LEN = 4_096
SMOKE_RUNS = 512
SMOKE_TIMEOUT_SECONDS = 60
FLAG_LANES = 4
# The target's set grammar: 4 + (b % 21).
MAX_SET_MEMBERS = 24


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manual", action="store_true", help="run indefinitely until interrupted")
    parser.add_argument("--runs", type=int, default=SMOKE_RUNS)
    parser.add_argument("--timeout", type=int, default=SMOKE_TIMEOUT_SECONDS)
    args = parser.parse_args()
    if args.runs < 0 or args.timeout <= 0:
        parser.error("--runs must be nonnegative and --timeout must be positive")

    started = time.monotonic()
    RUNTIME.mkdir(parents=True, exist_ok=True)
    ARTIFACTS.mkdir(parents=True, exist_ok=True)
    corpus_count = generate_seed_corpus()

    evidence: dict[str, object] = {
        "contract": "s20-700-complete-root-judgment-persistent-libfuzzer-slice-v1",
        "scope": "COMPLETE_ROOT_JUDGMENT_ONLY",
        "full_s20_700_complete": False,
        "flag_lanes": FLAG_LANES,
        "max_set_members": MAX_SET_MEMBERS,
        "max_input_bytes": MAX_LEN,
        "corpus_count": corpus_count,
        "seed_source": str(FIXTURE.relative_to(ROOT)),
        "runtime_path": str(RUNTIME.relative_to(ROOT)),
        "commands": [],
        "problems": [],
    }

    for problem in toolchain_problems():
        evidence["problems"].append(problem)
    if evidence["problems"]:
        evidence["result"] = "BLOCKED"
        EVIDENCE.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
        print(json.dumps(evidence, indent=2, sort_keys=True))
        return 2

    env = os.environ.copy()
    env["CC"] = CLANG
    build = run(
        [
            "cargo",
            f"+{RUST_TOOLCHAIN}",
            "rustc",
            "--manifest-path",
            "fuzz/Cargo.toml",
            "--bin",
            "complete_root_judgment",
            "--release",
            "--target-dir",
            str(TARGET_DIR),
            "--",
            "-Cpasses=sancov-module",
            "-Cllvm-args=-sanitizer-coverage-level=4",
            "-Cllvm-args=-sanitizer-coverage-inline-8bit-counters",
            f"-Clink-arg={LIBFUZZER}",
            "-Clink-arg=-lstdc++",
        ],
        env=env,
        timeout=args.timeout,
    )
    evidence["commands"].append(build)
    if build["returncode"] != 0:
        evidence["result"] = "FAIL"
        EVIDENCE.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
        print(json.dumps(evidence, indent=2, sort_keys=True))
        return 1

    if args.manual:
        command = [
            str(FUZZER),
            f"-max_len={MAX_LEN}",
            f"-artifact_prefix={ARTIFACTS}/",
            str(CORPUS),
        ]
        print(" ".join(command))
        return subprocess.call(command, cwd=ROOT)

    fuzz = run(
        [
            str(FUZZER),
            f"-runs={args.runs}",
            f"-max_len={MAX_LEN}",
            f"-artifact_prefix={ARTIFACTS}/",
            str(CORPUS),
        ],
        timeout=args.timeout,
    )
    evidence["commands"].append(fuzz)
    evidence["libfuzzer_output_tail"] = (fuzz["stderr"] + fuzz["stdout"])[-4000:]
    evidence["duration_seconds"] = round(time.monotonic() - started, 3)
    evidence["result"] = "PASS" if fuzz["returncode"] == 0 else "FAIL"
    EVIDENCE.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
    print(json.dumps(evidence, indent=2, sort_keys=True))
    return 0 if evidence["result"] == "PASS" else 1


def toolchain_problems() -> list[str]:
    problems = []
    if shutil.which(CLANG) is None:
        problems.append(f"{CLANG}-missing")
    if not LIBFUZZER.exists():
        problems.append(f"libfuzzer-runtime-missing:{LIBFUZZER}")
    rustup = shutil.which("rustup")
    if rustup is None:
        problems.append("rustup-missing")
    else:
        result = subprocess.run(
            ["rustup", "toolchain", "list"],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if RUST_TOOLCHAIN not in result.stdout:
            problems.append(f"rust-toolchain-missing:{RUST_TOOLCHAIN}")
    return problems


def id_byte(value: str) -> int:
    """Fixture identities are 32 repeated bytes; the target reads one byte."""
    raw = bytes.fromhex(value)
    if raw != raw[:1] * 32:
        raise SystemExit("complete-entity-impact fixture identity is not a repeated byte")
    return raw[0]


def encode_ids(values: list[str]) -> bytes:
    """Encode one set under the target's extensible length grammar.

    A length below four is one byte. Four or more is `4` followed by the
    remainder, which the target reads as `4 + b % 21`, so a seed carries a
    full fixture set instead of its first four members.
    """
    count = len(values)
    if count > 24:
        raise SystemExit("fixture set exceeds the target's twenty-four-element grammar")
    prefix = bytes([count]) if count < 4 else bytes([4, count - 4])
    return prefix + bytes(id_byte(value) for value in values)


def encode_entity(entity: dict) -> bytes:
    kind = entity["kind"]
    out = bytes([kind - 1, id_byte(entity["id"])])
    if kind == 1:
        out += bytes([id_byte(entity["root_namespace"])])
        for field in ("packages", "capability_requirements", "contracts", "tests"):
            out += encode_ids(entity[field])
    elif kind == 2:
        out += bytes([id_byte(entity["workspace"]), id_byte(entity["root_namespace"])])
        out += encode_ids(entity["dependencies"]) + encode_ids(entity["exports"])
    elif kind == 3:
        parent = entity["parent"]
        out += b"\x00" if parent is None else bytes([1, id_byte(parent)])
        out += encode_ids(entity["members"])
    elif kind == 4:
        out += encode_ids(entity["invariants"])
    elif kind == 5:
        out += bytes([id_byte(entity["entry_block"])])
        out += encode_ids(entity["parameters"]) + encode_ids(entity["effects"])
        out += encode_ids(entity["blocks"]) + encode_ids(entity["contracts"])
    elif kind == 6:
        out += bytes([id_byte(entity["owner"]), 0 if entity["role"] == "function" else 1])
    elif kind == 7:
        out += bytes([id_byte(entity["function"]), id_byte(entity["return_parameter"])])
        out += encode_ids(entity["parameters"]) + encode_ids(entity["operations"])
    elif kind == 10:
        out += bytes([id_byte(entity["initializer"])])
    elif kind == 12:
        out += bytes([id_byte(entity["effect"])]) + encode_ids(entity["constraint_contracts"])
    elif kind == 13:
        out += bytes([id_byte(entity["target"]), id_byte(entity["predicate"])])
    elif kind == 14:
        out += bytes([id_byte(entity["target"])])
    elif kind == 15:
        out += encode_ids(entity["effects"])
    elif kind == 16:
        out += bytes([id_byte(entity["function"]), entity["exposure"] - 1])
    elif kind == 17:
        out += bytes([id_byte(entity["subject"])]) + encode_ids(entity["requirements"])
    elif kind == 18:
        out += bytes(
            [
                id_byte(entity["dependency_root"]),
                id_byte(entity["external_package"]),
                id_byte(entity["local_namespace"]),
            ]
        )
    return out


def encode_request(request: dict, flags: int) -> bytes:
    entities = request["entities"]
    if len(entities) > 24:
        raise SystemExit("fixture exceeds the target's entity grammar")
    out = bytes([len(entities)])
    for entity in entities:
        out += encode_entity(entity)
    return out + bytes([flags])


def generate_seed_corpus() -> int:
    if CORPUS.exists():
        shutil.rmtree(CORPUS)
    CORPUS.mkdir(parents=True)

    accepted = json.loads(FIXTURE.read_text(encoding="utf-8"))
    rejected = json.loads(REJECTED.read_text(encoding="utf-8"))
    if accepted.get("contract") != "sley2-complete-entity-impact-v1":
        raise SystemExit("complete-entity-impact fixture contract drifted")
    requests = [vector["request"] for vector in accepted["vectors"]]
    requests.extend(mutation["request"] for mutation in rejected["mutations"])

    payloads = []
    for request in requests:
        for flags in range(FLAG_LANES):
            payloads.append(encode_request(request, flags))
    canonical = payloads[0]
    payloads.extend([canonical + b"\x00", b"\x00", b"\x01\x00\x00\x00"])
    for seed in range(128):
        value = bytearray(canonical)
        index = (seed * 2_654_435_761) % len(value)
        value[index] ^= 1 << (seed % 8)
        payloads.append(bytes(value))
    unique_seeds = list(dict.fromkeys(payloads))
    for index, seed in enumerate(unique_seeds):
        digest = hashlib.sha256(seed).hexdigest()[:16]
        (CORPUS / f"seed-{index:04d}-{digest}").write_bytes(seed)
    return len(unique_seeds)


def run(command: list[str], *, env: dict[str, str] | None = None, timeout: int) -> dict[str, object]:
    started = time.monotonic()
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
        return {
            "argv": command,
            "returncode": completed.returncode,
            "duration_seconds": round(time.monotonic() - started, 3),
            "stdout": completed.stdout[-4000:],
            "stderr": completed.stderr[-4000:],
        }
    except subprocess.TimeoutExpired as error:
        return {
            "argv": command,
            "returncode": 124,
            "duration_seconds": round(time.monotonic() - started, 3),
            "stdout": output_tail(error.stdout),
            "stderr": output_tail(error.stderr),
            "timeout_seconds": timeout,
        }


def output_tail(value: str | bytes | None) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        value = value.decode("utf-8", errors="replace")
    return value[-4000:]


if __name__ == "__main__":
    sys.exit(main())
