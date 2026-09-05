#!/usr/bin/env python3
"""Build and run the S20-420 SMP1 JSON bridge persistent libFuzzer target."""

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
RUNTIME = ROOT / "evidence/runtime/s20-700-smp1-json-bridge-libfuzzer"
CORPUS = RUNTIME / "corpus"
ARTIFACTS = RUNTIME / "artifacts"
EVIDENCE = RUNTIME / "evidence.json"
TARGET_DIR = RUNTIME / "target"
FUZZER = TARGET_DIR / "release/smp1_json_bridge"
FIXTURE = ROOT / "conformance/smp1-json-bridge/v1/roundtrip.json"
REJECTED = ROOT / "conformance/smp1-json-bridge/v1/rejected.json"
TABLE = ROOT / "conformance/smp1-json-bridge/v1/methods.json"
CLANG = "clang-18"
RUST_TOOLCHAIN = "nightly-2026-02-27"
LIBFUZZER = Path("/usr/lib/llvm-18/lib/clang/18/lib/linux/libclang_rt.fuzzer-x86_64.a")
MAX_PAYLOAD_LEN = 65_536
MAX_LEN = MAX_PAYLOAD_LEN + 1
SMOKE_RUNS = 512
SMOKE_TIMEOUT_SECONDS = 90
SELECTOR_COUNT = 3


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
        "contract": "s20-700-smp1-json-bridge-persistent-libfuzzer-slice-v1",
        "scope": "SMP1_JSON_BRIDGE_TEXT_AND_FRAME_ROUND_TRIP_ONLY",
        "full_s20_700_complete": False,
        "selector_count": SELECTOR_COUNT,
        "max_payload_bytes": MAX_PAYLOAD_LEN,
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
            "smp1_json_bridge",
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
        command = [str(FUZZER), f"-max_len={MAX_LEN}", f"-artifact_prefix={ARTIFACTS}/", str(CORPUS)]
        print(" ".join(command))
        return subprocess.call(command, cwd=ROOT)

    fuzz = run(
        [str(FUZZER), f"-runs={args.runs}", f"-max_len={MAX_LEN}", f"-artifact_prefix={ARTIFACTS}/", str(CORPUS)],
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


def generate_seed_corpus() -> int:
    if CORPUS.exists():
        shutil.rmtree(CORPUS)
    CORPUS.mkdir(parents=True)

    accepted = json.loads(FIXTURE.read_text(encoding="utf-8"))
    rejected = json.loads(REJECTED.read_text(encoding="utf-8"))
    table = json.loads(TABLE.read_text(encoding="utf-8"))
    if accepted.get("contract") != "sley2-smp1-json-bridge-v1":
        raise SystemExit("SMP1 JSON bridge fixture contract drifted")
    texts = [vector["json"].encode("utf-8") for vector in accepted["vectors"]]
    frames = [bytes.fromhex(vector["frame_hex"]) for vector in accepted["vectors"]]
    for frame in frames:
        if frame[8:16] != b"SLEYSCB1":
            raise SystemExit("SMP1 JSON bridge fixture structure drifted")
    texts.extend(mutation["json"].encode("utf-8") for mutation in rejected["mutations"])
    names = [row["name"] for row in table["methods"]]
    hello = {
        "adapters": ["a1" * 32, "a2" * 32],
        "effects": ["e1" * 32],
        "features": {"cancel": True, "checksum": False, "json_bridge": True, "stream": True},
        "limits": {
            "max_depth": 16,
            "max_edges": 10_000,
            "max_entities": 1_000,
            "max_frame_bytes": 1_048_576,
            "max_inflight": 4,
            "max_sessions": 16,
            "max_response_bytes": 1_048_576,
            "max_work": 1_000_000,
        },
        "methods": names,
        "protocol_versions": [1, 2],
        "schema_epochs": ["11" * 32, "12" * 32],
    }
    records = [
        json.dumps(hello, sort_keys=True, separators=(",", ":")).encode(),
        b'{"bytes":"00","index":0,"total":1}',
        b'{"code":40009,"details":"","incident":null,"phase":0,"retryability":"never","symbol":"PROTOCOL_LIMIT_EXCEEDED"}',
        b'{"code":"9007199254740992","details":"ff","incident":"' + b"ab" * 32 + b'","phase":1,"retryability":"after_requery","symbol":"QUERY_CURSOR_INVALID"}',
    ]

    def derive(canonical: bytes) -> list[bytes]:
        derived = [canonical + b"\x00", canonical + b"\xff", canonical[:1], b""]
        boundaries = {1, 8, 9, 16, 17, 18, 50, 51, 52, len(canonical) - 33, len(canonical) - 32, len(canonical) - 1}
        boundaries.update(range(20, len(canonical), 16))
        derived.extend(canonical[:length] for length in sorted(boundaries) if 0 < length < len(canonical))
        for seed in range(128):
            value = bytearray(canonical)
            index = (seed * 2_654_435_761) % len(value)
            value[index] ^= 1 << (seed % 8)
            derived.append(bytes(value))
        return derived

    lanes = {
        0: texts + records + derive(texts[0]),
        1: frames + derive(frames[0]),
        2: records + texts + derive(records[0]),
    }
    seeds = [bytes([selector]) + payload for selector, payloads in lanes.items() for payload in payloads]
    unique_seeds = list(dict.fromkeys(seeds))
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
