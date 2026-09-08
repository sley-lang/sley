#!/usr/bin/env python3
"""Remote-head locator check (SLEY-2.0-ARCHITECTURE-TIGHTENING section 15.5).

The locator's non-commit fields live in machine-summary.json under `locator`;
commit identity is read from git here and never stored as a second source of
truth. Every digest and version claim in the locator is verified against the
artifact it names: in-repo records by raw-byte SHA-256, spec status lines by
text, out-of-repo masters by SHA-256 when they resolve (reported as
UNAVAILABLE otherwise, never as a pass). `--render` writes the derived view
docs/status/SLEY2-REMOTE-HEAD.md; `--check` fails if that view is stale.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
VIEW = ROOT / "docs/status/SLEY2-REMOTE-HEAD.md"
REQUIRED = (
    "canonical_integration_branch",
    "latest_validated_integration_commit",
    "active_lanes",
    "canonical_spec",
    "reweave_master",
    "schema_epoch",
    "host_abi",
    "exec_package",
    "bootstrap_profile",
    "known_blocked_gates",
    "remote",
    "last_updated_utc",
)
ENV_OVERRIDES = {
    "canonical_spec": "SLEY2_MASTER_GOAL",
    "reweave_master": "SLEY2_REWEAVE_MASTER",
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git(*args: str, repo: Path = ROOT) -> str:
    proc = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, check=False)
    return proc.stdout.strip() if proc.returncode == 0 else ""


def verify(locator: dict, summary: dict, repo: Path) -> tuple[list[str], dict]:
    problems: list[str] = []
    facts: dict = {}
    for key in REQUIRED:
        if key not in locator:
            problems.append(f"locator:missing:{key}")
    if problems:
        return problems, facts

    for name in ("host_abi", "exec_package", "bootstrap_profile"):
        entry = locator[name]
        record = repo / entry["record"]
        doc = repo / entry["contract_doc"]
        if not record.is_file():
            problems.append(f"{name}:record-missing")
            continue
        actual = sha256(record)
        if actual != entry["digest"]:
            problems.append(f"{name}:digest-drift:{actual[:12]}")
        text = doc.read_text(encoding="utf-8") if doc.is_file() else ""
        for marker in (entry["identity"], entry["contract"], entry["digest"]):
            if marker not in text:
                problems.append(f"{name}:contract-doc-missing:{marker[:24]}")

    epoch = locator["schema_epoch"]
    recorded = summary.get("schema_epoch", {}).get("bootstrap_schema_epoch_id")
    if epoch.get("id") != recorded:
        problems.append("schema_epoch:id-drift")
    fixture = repo / epoch.get("fixture", "")
    if fixture.is_file() and sha256(fixture) != epoch.get("fixture_sha256"):
        problems.append("schema_epoch:fixture-drift")
    elif not fixture.is_file():
        problems.append("schema_epoch:fixture-missing")

    for name, env in ENV_OVERRIDES.items():
        entry = locator[name]
        path = Path(os.environ[env]) if os.environ.get(env) else Path(entry["path"])
        if path.is_file():
            actual = sha256(path)
            facts[f"{name}_state"] = "VERIFIED" if actual == entry["sha256"] else f"DRIFT:{actual[:12]}"
            if actual != entry["sha256"]:
                problems.append(f"{name}:sha256-drift:{actual[:12]}")
        else:
            facts[f"{name}_state"] = f"UNAVAILABLE (set {env})"

    branch = git("rev-parse", "--abbrev-ref", "HEAD", repo=repo)
    head = git("rev-parse", "HEAD", repo=repo)
    parent = git("rev-parse", "HEAD^", repo=repo)
    validated = locator["latest_validated_integration_commit"]
    facts.update({"branch": branch, "head": head, "parent": parent})
    if git("rev-parse", "--verify", "-q", f"{validated}^{{commit}}", repo=repo) == "":
        problems.append("validated-commit:not-in-git")
    else:
        ahead = git("rev-list", "--count", f"{validated}..HEAD", repo=repo)
        facts["head_ahead_of_validated"] = int(ahead) if ahead else None
        ancestor = subprocess.run(
            ["git", "-C", str(repo), "merge-base", "--is-ancestor", validated, head], capture_output=True, check=False
        )
        if ancestor.returncode != 0:
            problems.append("validated-commit:not-an-ancestor-of-HEAD")
    for lane in locator["active_lanes"]:
        for ref in lane.get("branches", []):
            if git("rev-parse", "--verify", "-q", ref, repo=repo) == "":
                problems.append(f"lane:{lane.get('lane')}:branch-missing:{ref}")
    return problems, facts


def render(locator: dict, facts: dict, problems: list[str]) -> str:
    lanes = "\n".join(
        f"- {lane['lane']}: {', '.join(lane.get('branches', []))} (owner {lane.get('owner', '?')}; {lane.get('state', '')})"
        for lane in locator["active_lanes"]
    )
    gates = "\n".join(f"- {gate}" for gate in locator["known_blocked_gates"])
    result = "PASS" if not problems else "FAIL: " + ", ".join(problems)
    return f"""# SLEY2 remote head locator

Derived view of `machineresearch/sley-2.0/machine-summary.json` `locator`
(rendered by `scripts/check_remote_head.py --render`; git is authoritative for
commit identity, this file only makes remote navigation fast).

Canonical integration branch: `{locator['canonical_integration_branch']}`
Latest validated integration commit: `{locator['latest_validated_integration_commit']}`
Active REWEAVE / campaign lanes:
{lanes}
Current canonical Sley 2.0 spec path: `{locator['canonical_spec']['path']}` (sha256 `{locator['canonical_spec']['sha256']}`; in-repo dossier `{locator['canonical_spec']['in_repo_dossier']}`)
Current REWEAVE master path: `{locator['reweave_master']['path']}` (sha256 `{locator['reweave_master']['sha256']}`; adopted by `{locator['reweave_master']['adoption']}`)
Current schema epoch: `{locator['schema_epoch']['id']}` ({locator['schema_epoch']['name']})
Current host ABI: {locator['host_abi']['identity']} (`{locator['host_abi']['contract']}`, digest `{locator['host_abi']['digest']}`)
Current execution-package version: {locator['exec_package']['identity']} (`{locator['exec_package']['contract']}`, digest `{locator['exec_package']['digest']}`)
Current bootstrap profile: {locator['bootstrap_profile']['identity']} (`{locator['bootstrap_profile']['contract']}`, digest `{locator['bootstrap_profile']['digest']}`)
Known blocked gates:
{gates}
Remote: {locator['remote']['repository']} ({locator['remote']['state']})
Last updated UTC: {locator['last_updated_utc']}

Verification at render time: {result}
"""


def self_test() -> int:
    summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
    base = json.loads(json.dumps(summary["locator"]))
    failures: list[str] = []
    problems, _ = verify(base, summary, ROOT)
    if problems:
        failures.append("clean:" + ",".join(problems))
    stale = json.loads(json.dumps(base))
    stale["host_abi"]["digest"] = "00" * 32
    problems, _ = verify(stale, summary, ROOT)
    if not any(p.startswith("host_abi:digest-drift") for p in problems):
        failures.append("stale-digest-not-detected")
    missing = json.loads(json.dumps(base))
    del missing["known_blocked_gates"]
    problems, _ = verify(missing, summary, ROOT)
    if "locator:missing:known_blocked_gates" not in problems:
        failures.append("missing-field-not-detected")
    epoch = json.loads(json.dumps(base))
    epoch["schema_epoch"]["id"] = "11" * 32
    problems, _ = verify(epoch, summary, ROOT)
    if "schema_epoch:id-drift" not in problems:
        failures.append("stale-epoch-not-detected")
    ghost = json.loads(json.dumps(base))
    ghost["latest_validated_integration_commit"] = "0" * 40
    problems, _ = verify(ghost, summary, ROOT)
    if "validated-commit:not-in-git" not in problems:
        failures.append("ghost-commit-not-detected")
    if failures:
        print("SELF_TEST FAIL: " + "; ".join(failures))
        return 1
    print("SELF_TEST PASS: 5 cases")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--render", action="store_true", help="write docs/status/SLEY2-REMOTE-HEAD.md from the locator")
    parser.add_argument("--check", action="store_true", help="fail if the rendered view differs from the locator")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
    locator = summary.get("locator")
    if not isinstance(locator, dict):
        print(json.dumps({"problems": ["locator:absent"], "result": "FAIL"}, indent=2))
        return 1
    problems, facts = verify(locator, summary, ROOT)
    text = render(locator, facts, problems)
    if args.render:
        VIEW.parent.mkdir(parents=True, exist_ok=True)
        VIEW.write_text(text, encoding="utf-8")
    if args.check:
        if not VIEW.is_file():
            problems.append("view:missing")
        elif VIEW.read_text(encoding="utf-8") != text:
            problems.append("view:stale")
    report = {
        "facts": facts,
        "last_updated_utc": locator.get("last_updated_utc"),
        "problems": problems,
        "rendered_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ") if args.render else None,
        "result": "PASS" if not problems else "FAIL",
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    sys.exit(main())
