#!/usr/bin/env python3
"""Live-trial oracle entry for S2B-STALE-001: frozen S3 conformance stays in
`oracle.py`; live judging (agent-produced candidate workspaces) routes
here. Stdlib only. Prints exactly one JSON verdict; exit 0 accepted,
1 rejected, 2 harness error. See bench/fixtures/sley2_live_judge.py."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from sley2_live_judge import main


if __name__ == "__main__":
    raise SystemExit(main("S2B-STALE-001"))
