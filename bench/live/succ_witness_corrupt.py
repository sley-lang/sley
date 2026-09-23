#!/usr/bin/env python3
"""Deterministic CORRUPT witness through the real trial surface (no model).

Stages the unflipped base, restores the constant to its expected value
as a ReplaceEntityVersion through sley2_tool propose/finish, then runs
the frozen live judge. Beside the restored-value smoke check the judge
runs two judge-side exchange paths (the judge corrupts and imports the
bytes itself through the privileged `_raw_request`; no agent path can
attempt an import):
  - corpus vector: one canonical object byte flipped inside the staged
    exchange's embedded pack, exchange trailer resealed, exact
    PACK_DIGEST_MISMATCH through `exchange.import` (two flips, retries,
    fresh + populated destination unchanged, clean control accepted);
  - regression: unresealed bit-flips, exact EXCHANGE_DIGEST_MISMATCH.
Variants:
  pos             restore expected value (must accept)
  neg             flip to the wrong value (must reject
                  ORACLE_CORRUPT_UNRESTORED; smoke path)
  neg-accepted    pos candidate, the corruption vector replaced by the
                  unflipped control (an accepted-corruption stand-in):
                  must reject ORACLE_CORRUPT_ACCEPTED
  neg-unresealed  pos candidate, the corruption vector replaced by an
                  unresealed flip: must reject ORACLE_CORRUPT_UNREFUSED
                  carrying EXCHANGE_DIGEST_MISMATCH (never accepted for
                  the PACK code)

The log carries the judge's JSON verdict (status/code/detail), the
resealed-vector evidence the judge observed, and env provenance.

Usage: succ_witness_corrupt.py [pos|neg|neg-accepted|neg-unresealed] [logfile]
Env: SLEY2_SLEY_BINARY, SUCC_JUDGE_TEST_BINARY (both required).
"""

from __future__ import annotations

import contextlib
import datetime
import hashlib
import io
import json
import os
import platform
import subprocess
import sys
import tempfile
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from bench.fixtures import sley2_live_judge as judge  # noqa: E402
from bench.live import sley2_tool  # noqa: E402
from bench.live.taskpacks import stage_initial  # noqa: E402
from bench.live.tooling import stage_tooling  # noqa: E402


VARIANTS = ("pos", "neg", "neg-accepted", "neg-unresealed")


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _provenance() -> dict:
    def git(*argv: str) -> str:
        return subprocess.run(["git", *argv], cwd=ROOT, capture_output=True,
                              text=True, check=False).stdout.strip()

    sley = Path(os.environ["SLEY2_SLEY_BINARY"])
    driver = Path(os.environ["SUCC_JUDGE_TEST_BINARY"])
    return {
        "utc": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "git_head": git("rev-parse", "HEAD"),
        # Dirty paths outside the witness log directories: the evidence
        # must come from committed code.
        "git_dirty_paths": len(git("status", "--porcelain", "--", ".",
                                   ":(exclude)bench/live/succ-trials-*").splitlines()),
        "sley_binary": str(sley), "sley_sha256": _sha256(sley),
        "driver_binary": driver.name, "driver_sha256": _sha256(driver),
        "base_pack_sha256": _sha256(ROOT / "bench" / "fixtures" / "sley2"
                                    / "S2B-CORRUPT-001" / "base.pack"),
        "python": platform.python_version(),
    }


def _standin(variant: str) -> dict | None:
    """Stand-in vectors that target the rejection path (negatives)."""

    pack = (ROOT / "bench" / "fixtures" / "sley2" / "S2B-CORRUPT-001"
            / "base.pack").read_bytes()
    if variant == "neg-accepted":
        return {"control_hex": pack.hex(),
                "vectors": [{"label": "accepted-standin", "offset": None,
                             "hex": pack.hex()}]}
    if variant == "neg-unresealed":
        built = judge._resealed_pack_vectors(pack)
        offset = built["vectors"][0]["offset"]
        broken = bytearray(pack)
        broken[offset] ^= 0x01
        return {"control_hex": built["control_hex"],
                "vectors": [{"label": "unresealed-standin", "offset": offset,
                             "hex": bytes(broken).hex()}]}
    return None


def main() -> int:
    variant = sys.argv[1] if len(sys.argv) > 1 else "pos"
    if variant not in VARIANTS:
        raise SystemExit(f"variant must be one of {VARIANTS}")
    log_path = Path(sys.argv[2]) if len(sys.argv) > 2 else None
    if log_path is not None and log_path.exists():
        raise SystemExit(f"refusing to overwrite existing log {log_path}")
    for key in ("SLEY2_SLEY_BINARY", "SUCC_JUDGE_TEST_BINARY"):
        if not os.environ.get(key):
            raise SystemExit(f"missing env {key}")
    lines: list[str] = []

    def emit(text: str) -> None:
        lines.append(text)
        print(text, flush=True)

    tmp = tempfile.mkdtemp(prefix="sley2-corrupt-witness-")
    ws = Path(tmp) / "ws"
    stage_initial("sley_2_0", "S2B-CORRUPT-001", ws)
    stage_tooling("sley_2_0", ws)
    manifest = json.loads((ROOT / "bench" / "fixtures" / "sley2"
                           / "S2B-CORRUPT-001" / "task_manifest.json"
                           ).read_text())
    corrupt_ref = manifest["judge"]["corrupt"]["entity"]
    target = manifest["entities"].get(corrupt_ref, corrupt_ref)
    expected = manifest["judge"]["corrupt"]["expected"]
    saved_cwd = os.getcwd()
    os.chdir(ws)
    try:
        def run(*argv: str) -> tuple[int, dict]:
            out = io.StringIO()
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
            return code, json.loads(out.getvalue())

        _, rep = run("read", target)
        view = rep["report"]
        assert not view.get("failed"), view
        entry = view["decoded"]["entries"][0]
        body = dict(entry["body"])
        value = dict(body["value"])
        data = dict(value["data"])
        body["value"] = {**value, "data": {**data, "value":
                                           not expected if variant == "neg"
                                           else expected}}
        ops = [{"class": "ReplaceEntityVersion", "kind": entry["kind"],
                "target": target, "field_tag": None, "payload": body}]
        code, rep2 = run("propose", json.dumps(ops))
        report2 = rep2.get("report", {})
        emit(f"CORRUPT witness/{variant}: propose valid "
             f"{report2.get('valid')}")
        if not report2.get("valid"):
            emit(f"propose detail {json.dumps(report2)[:300]}")
            return 2
        code, rep3 = run("finish", report2["record"])
        emit(f"CORRUPT witness/{variant}: finished "
             f"{rep3.get('report', {}).get('finished')}")
        observed: list[dict] = []
        regression: list[str] = []
        original = judge._judge_corrupt_pack_resealed
        original_regression = judge._judge_corrupt_exchange

        def recorded_regression(task_dir: Path) -> None:
            original_regression(task_dir)
            regression.append("returned without rejection (requires exact "
                              "EXCHANGE_DIGEST_MISMATCH on both unresealed flips, "
                              "head tx and live object count unchanged)")
        standin = _standin(variant)

        def recorded(task_dir: Path, vectors: dict | None = None) -> dict:
            evidence = original(task_dir, standin if standin is not None else vectors)
            observed.append(evidence)
            return evidence

        saved_argv = sys.argv
        sys.argv = ["sley2_live_judge", str(ws)]
        verdict_out = io.StringIO()
        try:
            with mock.patch.object(judge, "_judge_corrupt_pack_resealed", recorded), \
                    mock.patch.object(judge, "_judge_corrupt_exchange", recorded_regression), \
                    contextlib.redirect_stdout(verdict_out):
                exit_code = judge.main("S2B-CORRUPT-001")
        finally:
            sys.argv = saved_argv
        verdict = json.loads(verdict_out.getvalue().strip().splitlines()[-1])
        emit(f"S2B-CORRUPT-001 witness/{variant} judge exit: {exit_code}")
        emit(f"S2B-CORRUPT-001 witness/{variant} verdict: "
             f"{json.dumps(verdict, sort_keys=True)}")
        if standin is not None:
            emit(f"S2B-CORRUPT-001 witness/{variant} stand-in vectors: "
                 f"{json.dumps([v['label'] for v in standin['vectors']])}")
        for note in regression:
            emit(f"S2B-CORRUPT-001 witness/{variant} exchange-trailer regression: {note}")
        for evidence in observed:
            emit(f"S2B-CORRUPT-001 witness/{variant} resealed-vector evidence: "
                 f"{json.dumps(evidence, sort_keys=True)}")
        emit(f"S2B-CORRUPT-001 witness/{variant} provenance: "
             f"{json.dumps(_provenance(), sort_keys=True)}")
        emit(f"workspace kept at: {ws}")
    finally:
        os.chdir(saved_cwd)
    if log_path is not None:
        log_path.parent.mkdir(parents=True, exist_ok=True)
        log_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
