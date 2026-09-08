#!/usr/bin/env python3
"""Independently check the S20-310 entity-read v2 vectors (stage A).

Normal mode is read-only: the oracle rebuilds every expected byte string
from the hand-authored semantic inputs in
`conformance/entity-read/v2/inputs.json` (plus frozen schema/descriptor
constants, the existing `codec` primitives, and the existing
`mutation_value` body encoders) and compares the reconstruction against
`conformance/entity-read/v2/accepted.json` / `rejected.json`. It never
invokes Rust and never reads emitted epochs, objects, IDs, or malformed
bytes from any emitter or generated corpus.

Refresh mode derives `accepted.json`, `rejected.json`, and `SHA256SUMS`
from the semantic inputs into an explicit caller-supplied output directory
(a root-owned staging directory outside the repository pin). It never
writes the in-repo corpus paths and never overwrites expected files.

Stage A boundary: this script only checks or stages. The Rust
actual-output comparison, corpus materialization, Makefile/registration,
and required-index changes belong to the root integrator's later stages.

Root execution contract (never executed by the author of this file):
  uv run --project oracle/scb1 --frozen python3 scripts/check_entity_read_vectors.py
  uv run --project oracle/scb1 --frozen python3 scripts/check_entity_read_vectors.py --refresh --output-dir <staging-outside-pin>
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ORACLE_SRC = ROOT / "oracle" / "scb1" / "src"
sys.path.insert(0, str(ORACLE_SRC))

from sley2_scb1_oracle.entity_read import (  # noqa: E402
    check_accepted,
    check_rejected,
    refresh,
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--inputs", default=str(ROOT / "conformance" / "entity-read" / "v2" / "inputs.json"))
    parser.add_argument("--accepted", default=str(ROOT / "conformance" / "entity-read" / "v2" / "accepted.json"))
    parser.add_argument("--rejected", default=str(ROOT / "conformance" / "entity-read" / "v2" / "rejected.json"))
    parser.add_argument("--refresh", action="store_true", help="derive artifacts into --output-dir instead of checking")
    parser.add_argument("--output-dir", default=None, help="root-owned staging directory outside the corpus pin (refresh only)")
    args = parser.parse_args()
    if args.refresh:
        if not args.output_dir:
            parser.error("refresh requires an explicit --output-dir outside the corpus pin")
        try:
            result = refresh(Path(args.inputs), Path(args.output_dir), ROOT)
        except (ValueError, OSError) as error:
            print(json.dumps({"contract": "s20-310-entity-read-oracle-v2", "result": "REFRESH-FAIL", "error": str(error)}, indent=2, sort_keys=True))
            return 1
        print(json.dumps({"contract": "s20-310-entity-read-oracle-v2", "result": "REFRESHED", **result}, indent=2, sort_keys=True))
        return 0
    try:
        inputs = json.loads(Path(args.inputs).read_text(encoding="utf-8"))
        accepted = json.loads(Path(args.accepted).read_text(encoding="utf-8"))
        rejected = json.loads(Path(args.rejected).read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        print(json.dumps({"contract": "s20-310-entity-read-oracle-v2", "result": "FAIL", "problems": [f"unreadable-input:{error}"]}, indent=2, sort_keys=True))
        return 1
    problems = check_accepted(inputs, accepted) + check_rejected(inputs, rejected)
    print(
        json.dumps(
            {
                "contract": "s20-310-entity-read-oracle-v2",
                "cases": len(accepted.get("cases", {})),
                "rejections": len(rejected.get("cases", [])),
                "problems": problems,
                "result": "PASS" if not problems else "FAIL",
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())
