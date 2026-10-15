#!/usr/bin/env python3
"""Bounded, offline phase measurements on newly created disposable workspaces."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import selectors
import shutil
import signal
import subprocess
import time


def terminate_group(child):
    """Bounded cleanup of only this newly created session/process group."""
    def exists():
        try:
            os.killpg(child.pid, 0)
            return True
        except ProcessLookupError:
            return False

    for sig, grace in [(signal.SIGTERM, 1.0), (signal.SIGKILL, 1.0)]:
        if not exists():
            return
        try:
            os.killpg(child.pid, sig)
        except ProcessLookupError:
            return
        end = time.monotonic() + grace
        while time.monotonic() < end:
            child.poll()  # Reap our direct child; do not wait indefinitely.
            if not exists():
                return
            time.sleep(0.01)
    raise RuntimeError("owned process group cleanup unverified; do not retry")


def capture(argv, cwd, timeout):
    """Cap each output stream to 1 MiB and close the owned group on every exit."""
    child = subprocess.Popen([str(x) for x in argv], cwd=cwd,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             start_new_session=True)
    end = time.monotonic() + timeout
    streams = {"stdout": bytearray(), "stderr": bytearray()}
    try:
        with selectors.DefaultSelector() as selector:
            for name, pipe in [("stdout", child.stdout), ("stderr", child.stderr)]:
                os.set_blocking(pipe.fileno(), False)
                selector.register(pipe, selectors.EVENT_READ, name)
            while selector.get_map() or child.poll() is None:
                remaining = end - time.monotonic()
                if remaining <= 0:
                    raise RuntimeError("command exceeded deadline")
                if not selector.get_map():
                    try:
                        child.wait(timeout=remaining)
                    except subprocess.TimeoutExpired:
                        raise RuntimeError("command exceeded deadline") from None
                    break
                for key, _ in selector.select(min(0.05, remaining)):
                    data = os.read(key.fd, 65536)
                    if not data:
                        selector.unregister(key.fileobj)
                    else:
                        streams[key.data].extend(data)
                        if len(streams[key.data]) > 1024**2:
                            raise RuntimeError("child output exceeded 1 MiB stream cap")
            return child.returncode, bytes(streams["stdout"]), bytes(streams["stderr"])
    finally:
        try:
            terminate_group(child)
        finally:
            child.stdout.close()
            child.stderr.close()


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def function(name, value="x"):
    return {"fn": name, "params": [["x", "i64"]], "returns": "i64",
            "blocks": [{"name": "entry", "ops": [], "term": ["return", value]}]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--agent", required=True, type=Path)
    parser.add_argument("--probe", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--sizes", nargs="+", type=int, default=[1, 10, 100, 1000])
    parser.add_argument("--samples", type=int, default=5)
    args = parser.parse_args()
    if (not 1 <= args.samples <= 10 or len(args.sizes) > 8
            or len(set(args.sizes)) != len(args.sizes)
            or any(not 1 <= n <= 1000 for n in args.sizes)):
        parser.error("use 1–10 samples and at most eight distinct sizes in 1–1000")
    agent, probe = args.agent.resolve(strict=True), args.probe.resolve(strict=True)
    output = args.output.absolute()
    output.mkdir(mode=0o700, parents=False, exist_ok=False)
    deadline = time.monotonic() + 300
    if hasattr(os, "sched_getaffinity"):
        os.sched_setaffinity(0, {min(os.sched_getaffinity(0))})
    commands = []

    def write(name, value):
        (output / name).write_text(json.dumps(value, indent=2) + "\n")

    def run(argv, cwd):
        if shutil.disk_usage(output).free < 40 * 1024**3:
            raise RuntimeError("40 GiB operating reserve reached")
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise RuntimeError("300-second campaign limit reached")
        started = time.perf_counter_ns()
        try:
            code, stdout, stderr = capture(argv, cwd, min(30, remaining))
        except BaseException as error:
            commands.append({"argv": [str(x) for x in argv], "error": str(error),
                             "wall_ns": time.perf_counter_ns() - started})
            write("commands.json", commands)
            raise
        wall_ns = time.perf_counter_ns() - started
        row = {"argv": [str(x) for x in argv], "cwd": str(cwd),
               "returncode": code, "wall_ns": wall_ns,
               "stdout": stdout.decode(), "stderr": stderr.decode()}
        commands.append(row)
        write("commands.json", commands)
        if code != 0:
            raise RuntimeError(f"command failed; see commands.json: {argv[1:]}")
        return row

    manifest = {
        "kind": "sley.scaling-run.v1", "owner": "sley-agent scaling example",
        "purpose": "Issue21 finding9 source-level phase measurements",
        "agent_sha256": digest(agent), "probe_sha256": digest(probe),
        "script_sha256": digest(Path(__file__)), "sizes": args.sizes,
        "samples": args.samples, "host": platform.platform(),
        "cpu_affinity": sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
        "cache_conditions": "Fresh process each sample; OS page cache warmed by fixture construction; no cache dropping. Probe lowers once then repeats calls in-process.",
        "retention": "Keep inputs, raw evidence and report; generated workspaces contain disposable genesis authority and are not public artifacts.",
        "review_status": "unreviewed", "started_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }
    write("manifest.json", manifest)
    fixtures = {}
    for size in args.sizes:
        workspace = output / f"workspace-{size}"
        run([agent, "init", workspace], output)
        initial = output / f"initial-{size}.json"
        initial.write_text(json.dumps({"af1": 1, "afx": 1,
            "fns": [function("target")] + [function(f"padding_{i:04}") for i in range(size - 1)]}) + "\n")
        run([agent, "try", initial], workspace)
        run([agent, "commit", "c1"], workspace)
        edit = output / f"edit-{size}.json"
        edit.write_text(json.dumps({"af1": 1, "afx": 1, "fns": [function("target", 7)]}) + "\n")
        run([agent, "try", edit], workspace)
        fixtures[size] = {"initial_sha256": digest(initial), "edit_sha256": digest(edit),
                          "initial_bytes": initial.stat().st_size, "edit_bytes": edit.stat().st_size}
    write("fixtures.json", fixtures)
    schedule = [(size, repeat) for size in args.sizes for repeat in range(args.samples)]
    random.Random(20261015).shuffle(schedule)
    write("schedule.json", schedule)
    samples = []
    for size, repeat in schedule:
        workspace = output / f"workspace-{size}"
        empty = run([probe, "--empty"], output)
        observed = run([probe, workspace, "c2"], output)
        value = json.loads(observed["stdout"])
        if value["oracle"]["passed_calls"] != 101 or not value["candidate_valid"]:
            raise RuntimeError("probe correctness checks incomplete")
        if value["fixture_checks"]["accepted_functions"] != size:
            raise RuntimeError("fixture function count differs from requested size")
        view = run([agent, "view", "--focus", "target", "--x"], workspace)
        if "padding_" in view["stdout"] or "target" not in view["stdout"]:
            raise RuntimeError("focused view did not select the fixed target")
        call = run([agent, "call", "target", "42"], workspace)
        if json.loads(call["stdout"]) != 42:
            raise RuntimeError("CLI identity oracle failed: expected 42")
        samples.append({"functions": size, "repeat": repeat,
                        "empty_process_wall_ns": empty["wall_ns"],
                        "probe_process_wall_ns": observed["wall_ns"],
                        "cli_focused_view_wall_ns": view["wall_ns"],
                        "cli_focused_view_output_bytes": len(view["stdout"].encode()),
                        "cli_call_wall_ns": call["wall_ns"],
                        "cli_call_stdout": call["stdout"], "probe": value})
        write("samples.json", samples)
    manifest["completed_utc"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    manifest["completed_samples"] = len(samples)
    manifest["status"] = "completed"
    write("manifest.json", manifest)
    print(json.dumps({"status": "completed", "samples": len(samples), "output": str(output)}))


if __name__ == "__main__":
    def interrupted(signum, _frame):
        raise SystemExit(128 + signum)

    signal.signal(signal.SIGTERM, interrupted)
    main()
