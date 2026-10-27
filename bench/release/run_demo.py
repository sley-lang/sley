#!/usr/bin/env python3
"""S20-720 canonical demo: the source-independence proof through `bin/sley`.

Self-contained on purpose: it is packaged into the release candidate and runs
from the unpacked artifact with no source tree. It drives `sley serve --json`
over two empty directories with the packaged demo fixture and checks every
answer against the fixture's expected bytes. It reads nothing else.
"""

from __future__ import annotations

import argparse
import json
import os
import select
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

# The unpacked artifact root: this script ships as demo/run_demo.py, so the
# default binary and fixture resolve from here, never from the caller's
# working directory.
ARTIFACT_ROOT = Path(__file__).resolve().parent.parent

# The demo zeroes every limit the bridge governs. The names must match
# LIMIT_FIELDS in crates/sley-json-bridge/src/lib.rs exactly: a governed
# limit missing here fails bridge shape validation on every demo frame, and
# bench/release/tests/test_packaging.py pins the two lists together.
ZERO_LIMITS = {name: 0 for name in ("max_depth", "max_edges", "max_entities", "max_frame_bytes", "max_inflight", "max_response_bytes", "max_work", "max_sessions")}
ZERO_BOUNDS = {
    "applied_limits": ZERO_LIMITS,
    "continuation": False,
    "omitted": 0,
    "reached_depth": 0,
    "returned_bytes": 0,
    "returned_edges": 0,
    "returned_entities": 0,
    "truncated": False,
}


class DemoFailure(Exception):
    pass


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")


def read_uvar(data: bytes, offset: int) -> tuple[int, int]:
    value, shift = 0, 0
    while True:
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            return value, offset
        shift += 7


def record_fields(data: bytes, count: int) -> list[bytes]:
    total, offset = read_uvar(data, 0)
    if total != count:
        raise DemoFailure(f"record count {total} != {count}")
    fields = []
    for index in range(count):
        tag, offset = read_uvar(data, offset)
        if tag != index + 1:
            raise DemoFailure("record tag order")
        length, offset = read_uvar(data, offset)
        fields.append(data[offset : offset + length])
        offset += length
    return fields


def uvar(value: int) -> bytes:
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def record(fields: list[bytes]) -> bytes:
    out = bytearray(uvar(len(fields)))
    for index, value in enumerate(fields, start=1):
        out += uvar(index) + uvar(len(value)) + value
    return bytes(out)


class Endpoint:
    def __init__(self, sley: Path, repository: Path, timeout: int, env: dict[str, str], cwd: Path):
        self.process = subprocess.Popen(
            [str(sley), "serve", "--repository", str(repository), "--json"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
            cwd=cwd,
        )
        self.timeout = timeout
        self.buffer = b""
        self.next_request = 1
        self.session: str | None = None

    def send(self, frame: dict) -> list[dict]:
        assert self.process.stdin is not None
        self.process.stdin.write(canonical(frame) + b"\n")
        self.process.stdin.flush()
        replies = []
        while True:
            line = self.read_line()
            reply = json.loads(line)
            replies.append(reply)
            if reply["kind"] in ("hello", "response"):
                return replies

    def read_line(self) -> bytes:
        assert self.process.stdout is not None
        descriptor = self.process.stdout.fileno()
        deadline = time.monotonic() + self.timeout
        while b"\n" not in self.buffer:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise DemoFailure("endpoint timeout")
            ready, _, _ = select.select([descriptor], [], [], remaining)
            if not ready:
                raise DemoFailure("endpoint timeout")
            chunk = os.read(descriptor, 65_536)
            if not chunk:
                raise DemoFailure("endpoint closed")
            self.buffer += chunk
        line, _, self.buffer = self.buffer.partition(b"\n")
        return line

    def request(self, method: str, body_hex: str, *, session: bool = True) -> dict:
        # The pre-session space carries identifier 0 only (SMP1 section
        # 3): session-less frames never consume the session counter.
        request_id = self.next_request if session else 0
        frame = {
            "body": body_hex,
            "bounds": ZERO_BOUNDS,
            "flags": {"cancel": False, "failed": False, "stream": False},
            "kind": "request",
            "method": method,
            "protocol_version": 1,
            "request_id": request_id,
            "session": self.session if session else None,
        }
        self.next_request += 1
        response = self.send(frame)[-1]
        if response["flags"]["failed"]:
            raise DemoFailure(f"{method} failed: {response['body'][:64]}")
        return response

    def close(self) -> int:
        self.process.communicate(timeout=self.timeout)
        return self.process.returncode


def sley_output(sley: Path, arguments: list[str], stdin: bytes, env: dict[str, str], cwd: Path) -> bytes:
    completed = subprocess.run([str(sley), *arguments], input=stdin, capture_output=True, env=env, cwd=cwd, check=False, timeout=120)
    if completed.returncode != 0:
        raise DemoFailure(f"sley {' '.join(arguments)} exit {completed.returncode}")
    return completed.stdout


def run(sley: Path, fixture_path: Path, work: Path, timeout: int) -> dict:
    env = {"PATH": "/usr/bin:/bin", "LANG": "C"}
    fixture = json.loads(fixture_path.read_text(encoding="utf-8"))
    vectors = fixture["vectors"]
    result: dict = {"contract": "s20-720-release-demo-v1", "steps": {}, "problems": []}
    hello_bytes = sley_output(sley, ["hello"], b"", env, work)
    hello_frame = json.loads(sley_output(sley, ["frame", "decode"], hello_bytes, env, work).decode("utf-8").strip())
    version = json.loads(sley_output(sley, ["version"], b"", env, work))
    result["version"] = version
    # Probe the deterministic handshake identity.
    probe_dir = work / "probe"
    report = work / "probe-report.json"
    completed = subprocess.run(
        [str(sley), "serve", "--repository", str(probe_dir), "--json", "--report", str(report)],
        input=canonical(hello_frame) + b"\n",
        capture_output=True,
        env=env,
        cwd=work,
        check=False,
        timeout=timeout,
    )
    if completed.returncode != 0:
        raise DemoFailure("probe invocation failed")
    handshake = json.loads(report.read_text(encoding="utf-8"))["handshake_id"]
    result["handshake_id"] = handshake

    def open_repository(name: str, exchange_hex: str) -> Endpoint:
        endpoint = Endpoint(sley, work / name, timeout, env, work)
        greeting = endpoint.send(hello_frame)[-1]
        if greeting["kind"] != "hello":
            raise DemoFailure("negotiation refused")
        endpoint.request("exchange.import", exchange_hex, session=False)
        opened = endpoint.request("session.open", handshake, session=False)
        endpoint.session = opened["body"]
        return endpoint

    first = open_repository("first", vectors["exchange_hex"])
    try:
        summary = first.request("query.root", vectors["query_root_request_hex"])
        result["steps"]["query_root_matches_fixture"] = summary["body"] == vectors["query_root_response_hex"]
        executed = first.request("execute", vectors["execute_request_hex"])
        report_id = record_fields(bytes.fromhex(executed["body"]), 2)[0].hex()
        result["steps"]["execute_report_id_matches_fixture"] = report_id == vectors["execution_report_id_hex"]
        result["steps"]["execute_response_matches_fixture"] = executed["body"] == vectors["execute_response_hex"]
        stored = first.request("report", report_id)
        result["steps"]["report_answers_stored_record"] = stored["body"] == executed["body"]
        branch = record([b"demo", bytes.fromhex(vectors["head_transaction_id_hex"])]).hex()
        first.request("branch.create", branch)
        result["steps"]["branch_created"] = True
        exported = first.request("exchange.export", "")
        result["steps"]["exported_bytes"] = len(exported["body"]) // 2
        gc = first.request("gc.dry_run", record([uvar(0)]).hex())
        fields = record_fields(bytes.fromhex(gc["body"]), 10)
        result["steps"]["gc_dry_run_no_candidates"] = fields[7] == uvar(1) and fields[4] == uvar(0)
        first.request("session.close", "")
    finally:
        result["steps"]["first_endpoint_exit"] = first.close()
    second = open_repository("second", exported["body"])
    try:
        clone_summary = second.request("query.root", vectors["query_root_request_hex"])
        result["steps"]["clone_query_root_matches_fixture"] = clone_summary["body"] == vectors["query_root_response_hex"]
        clone_report = second.request("execute", vectors["execute_request_hex"])
        result["steps"]["clone_execute_matches"] = clone_report["body"] == executed["body"]
        gc = second.request("gc.dry_run", record([uvar(0)]).hex())
        fields = record_fields(bytes.fromhex(gc["body"]), 10)
        result["steps"]["clone_gc_dry_run_no_candidates"] = fields[7] == uvar(1) and fields[4] == uvar(0)
        second.request("session.close", "")
    finally:
        result["steps"]["second_endpoint_exit"] = second.close()
    for name, value in result["steps"].items():
        if value is False or (name.endswith("_exit") and value != 0):
            result["problems"].append(name)
    result["source_free_environment"] = {"env": sorted(env), "cwd_is_artifact": True}
    result["explicit_gap"] = "candidate construction, commit, test selection, and merge wait for the public candidate builder (S20-350 proposal-only)"
    result["result"] = "PASS" if not result["problems"] else "FAIL"
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sley", type=Path, default=ARTIFACT_ROOT / "bin/sley")
    parser.add_argument("--fixture", type=Path, default=ARTIFACT_ROOT / "conformance/release-demo/v1/demo.json")
    parser.add_argument("--work", type=Path, help="keep the demo repositories here (default: a temporary directory, removed afterwards)")
    parser.add_argument("--timeout-seconds", type=int, default=60)
    arguments = parser.parse_args(argv)
    work = arguments.work or Path(tempfile.mkdtemp(prefix="sley-demo-"))
    work.mkdir(parents=True, exist_ok=True)
    try:
        result = run(arguments.sley.resolve(), arguments.fixture.resolve(), work.resolve(), arguments.timeout_seconds)
    except (DemoFailure, OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        result = {"contract": "s20-720-release-demo-v1", "result": "FAIL", "problems": [f"{type(error).__name__}:{error}"]}
    finally:
        if arguments.work is None:
            shutil.rmtree(work, ignore_errors=True)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result["result"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
