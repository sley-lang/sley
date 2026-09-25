#!/usr/bin/env python3
"""A minimal SMP1 client: drive `sley serve --json` over JSON lines.

Imports the release-demo program into a fresh repository, opens a session,
runs a root query, executes a function, and closes the session. Every answer
is compared with the demo fixture's expected bytes.

    python3 docs/examples/smp1_json_client.py [path/to/sley]

Standard library only. Walked through in docs/QUICKSTART.md.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

SLEY = sys.argv[1] if len(sys.argv) > 1 else "sley"
FIXTURE = Path(__file__).resolve().parents[2] / "conformance/release-demo/v1/demo.json"

# Every frame carries a bounded-context record. Requests send zeros; each
# response fills it with the negotiated limits and the counts it returned.
LIMITS = ("max_depth", "max_edges", "max_entities", "max_frame_bytes",
          "max_inflight", "max_response_bytes", "max_sessions", "max_work")
BOUNDS = {"applied_limits": dict.fromkeys(LIMITS, 0), "continuation": False,
          "omitted": 0, "reached_depth": 0, "returned_bytes": 0,
          "returned_edges": 0, "returned_entities": 0, "truncated": False}
FLAGS = {"cancel": False, "failed": False, "stream": False}


def sley(*args: str, stdin: str | bytes = b"") -> bytes:
    data = stdin.encode() if isinstance(stdin, str) else stdin
    return subprocess.run([SLEY, *args], input=data, capture_output=True, check=True).stdout


def main() -> int:
    vectors = json.loads(FIXTURE.read_text())["vectors"]
    work = Path(tempfile.mkdtemp(prefix="sley-client-"))

    # 1. The client hello: the frame this build offers, rendered as JSON.
    hello = sley("frame", "decode", stdin=sley("hello")).decode()

    # 2. The handshake identity is deterministic. A one-frame probe records it.
    report = work / "probe.json"
    sley("serve", "--repository", str(work / "probe"), "--json", "--report", str(report), stdin=hello)
    handshake = json.loads(report.read_text())["handshake_id"]

    # 3. Start an endpoint that speaks one JSON frame per line.
    server = subprocess.Popen(
        [SLEY, "serve", "--repository", str(work / "repo"), "--json"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
    )

    def send(frame: dict) -> dict:
        server.stdin.write(json.dumps(frame) + "\n")
        server.stdin.flush()
        while True:  # stream event frames may arrive before the response
            reply = json.loads(server.stdout.readline())
            if reply["kind"] in ("hello", "response"):
                return reply

    def request(method: str, body: str = "", request_id: int = 0, session: str | None = None) -> dict:
        reply = send({"kind": "request", "method": method, "body": body, "protocol_version": 1,
                      "request_id": request_id, "session": session, "bounds": BOUNDS, "flags": FLAGS})
        if reply["flags"]["failed"]:
            raise SystemExit(f"{method} failed: {reply['body'][:64]}")
        return reply

    # 4. Negotiate, then import a program. Session-less requests use id 0.
    print("hello        ->", send(json.loads(hello))["kind"])
    request("exchange.import", vectors["exchange_hex"])
    print("import       -> ok")

    # 5. Open a session. Session requests number upward from 1.
    session = request("session.open", handshake)["body"]
    print("session.open ->", session[:16] + "…")

    root = request("query.root", vectors["query_root_request_hex"], 1, session)
    print("query.root   ->", "matches fixture" if root["body"] == vectors["query_root_response_hex"] else "DIFFERS")

    run = request("execute", vectors["execute_request_hex"], 2, session)
    print("execute      ->", "matches fixture" if run["body"] == vectors["execute_response_hex"] else "DIFFERS")

    request("session.close", "", 3, session)
    server.stdin.close()
    print("exit status  ->", server.wait())
    shutil.rmtree(work)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
