"""Current-source binding and live lifecycle evidence for the R2 gate.

This is local execution evidence, not a signed third-party attestation. Review
transcripts must independently name the same source digest; old prose and
machine-summary identifiers never substitute for executing the lifecycle.
"""
from __future__ import annotations

import hashlib
import json
import re
import subprocess
from pathlib import Path

COMMAND = [
    "cargo", "test", "-p", "sley-repo", "--test",
    "rw060_source_free_lifecycle", "--locked", "--",
    "--show-output", "--test-threads=1",
]
REQUIRED_TESTS = {
    "reject_invalid_candidate_keeps_accepted_state_identical",
    "repair_commit_execute_binds_profile_epoch_limits_observation",
    "pack_export_import_reconstructs_exact_root_in_clean_store",
    "lifecycle_uses_no_source_parser_text_path_or_normalization",
    "commit_parametrized_callee_validates_and_executes",
}
REQUIRED_INPUTS = {
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
    "docs/spec/SSMC1_EPOCH1_SCHEMA.txt",
    "evidence/validation/anti-goal-conformance.json",
}
# Library test groups bound to the source digest alongside the suites: the
# admission-authority invariants and the Sley-owned AR-05 replay (the
# closure workloads replayed through the v2 path with per-metric
# attribution), so AR-05 closure evidence is revision-bound by the gate
# (Nabu P3 at 178873d7).
LIB_SUITES = {
    "admission_authority::tests": {
        "admission_authority::tests::authority_error_codes_are_stable",
        "admission_authority::tests::sley_evidence_ingress_has_no_minting_path",
    },
    "bootstrap_closure::closure_workloads_replay_through_v2_with_attribution": {
        "bootstrap_closure::closure_workloads_replay_through_v2_with_attribution",
    },
}
SUCCESSOR_SUITES = {
    "rw075_raw_callable": {"v2_package_section_digests_and_observation_are_frozen",
                           "raw_successor_package_binds_and_mismatches_refuse"},
    "rw075_exec_closure": {"authority_refuses_tampered_image_before_any_receipt",
                          "observations_bind_the_complete_package_identity"},
    "rw075_hydration_workloads": {"branching_work_queue_with_fan_out_and_cycle",
                                 "real_image_emission_with_control_flow",
                                 "mixed_compiler_like_workload_within_bounds"},
}


def source_digest(root: Path) -> str:
    """Bind code, build inputs, gate scripts and frozen R2 profiles.

    Include tracked deletions (which refuse) and untracked source additions.
    Evidence/transcripts and mutable summaries are excluded to avoid a circular
    review identity. Symlinks in this source inventory are refused.
    """
    names = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=root,
    ).decode().split("\0")
    selected = sorted({name for name in names if name and (
        name in REQUIRED_INPUTS | {"host-boundary.json", "Makefile"}
        or name.startswith(("crates/", ".cargo/", "conformance/", "docs/spec/"))
        or name.startswith("scripts/") and name.endswith(".py")
    )})
    if not REQUIRED_INPUTS <= set(selected):
        raise ValueError("missing required source inventory")
    manifest = {}
    for name in selected:
        path = root / name
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"source input is missing or not a regular file: {name}")
        manifest[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    encoded = json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(b"SLEY/R2/SOURCE/1\0" + encoded).hexdigest()


def test_output_problems(output: str, required: set[str], *, filtered: bool = False) -> list[str]:
    """Require a complete, nonempty run, named passes and matching totals."""
    problems = []
    summaries = re.findall(
        r"^test result: ok\. (\d+) passed; 0 failed; 0 ignored; 0 measured; (\d+) filtered out;",
        output, re.MULTILINE,
    )
    passed = set(re.findall(r"^test ([a-z0-9_:]+)(?: - should panic)? \.\.\. ok$", output, re.MULTILINE))
    if (len(summaries) != 1 or not passed
            or int(summaries[0][0]) != len(passed)
            or not filtered and int(summaries[0][1]) != 0):
        problems.append("missing-complete-successful-test-run")
    if not required <= passed:
        problems.append("missing-required-tests")
    return problems


def lifecycle_output_problems(output: str, expected_ids: dict[str, str]) -> list[str]:
    """Check actual test completion and the frozen lifecycle identities."""
    problems = test_output_problems(output, REQUIRED_TESTS)
    emitted: dict[str, set[str]] = {}
    for name, value in re.findall(r"RW060_EVIDENCE ([a-z0-9_]+)=([a-zA-Z0-9_]+)", output):
        emitted.setdefault(name, set()).add(value)
    for name, expected in expected_ids.items():
        label = "observation" if name in {"observation_ok", "observation_err"} else name
        values = emitted.get(label, set())
        if expected not in values or label != "observation" and values != {expected}:
            problems.append("lifecycle-identity:" + name)
    return problems


def run_suite(root: Path, expected_source: str, command: list[str], validate) -> dict:
    """Execute against the same source before/after; failures never pass."""
    result: dict = {"command": command, "source_sha256": expected_source, "pass": False}
    try:
        if source_digest(root) != expected_source:
            raise ValueError("source changed before lifecycle execution")
        completed = subprocess.run(command, cwd=root, stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, text=True,
                                   timeout=180, check=False)
        result["exit_code"] = completed.returncode
        result["output_sha256"] = hashlib.sha256(completed.stdout.encode()).hexdigest()
        result["problems"] = validate(completed.stdout)
        if completed.returncode:
            result["problems"].append("lifecycle-command-failed")
            result["failure_tail"] = completed.stdout[-4000:]
        if source_digest(root) != expected_source:
            result["problems"].append("source-changed-during-lifecycle")
        result["pass"] = not result["problems"]
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        result["problems"] = [str(error)]
    return result


def run_lifecycle(root: Path, expected_source: str, expected_ids: dict[str, str]) -> dict:
    """Run the complete RW060 lifecycle with its emitted identity checks."""
    return run_suite(root, expected_source, COMMAND,
                     lambda output: lifecycle_output_problems(output, expected_ids))


def run_successor(root: Path, expected_source: str) -> dict:
    """Run actual v2 package, admission, execution and workload behavior."""
    results = {}
    for suite, required in SUCCESSOR_SUITES.items():
        command = ["cargo", "test", "-p", "sley-vm", "--test", suite, "--locked",
                   "--", "--show-output", "--test-threads=1"]
        results[suite] = run_suite(root, expected_source, command,
                                  lambda output: test_output_problems(output, required))
    for group, required in LIB_SUITES.items():
        command = ["cargo", "test", "-p", "sley-vm", "--lib", group,
                   "--locked", "--", "--show-output", "--test-threads=1"]
        results[group.split("::")[0]] = run_suite(
            root, expected_source, command,
            lambda output, required=required: test_output_problems(output, required, filtered=True))
    return {"pass": all(item["pass"] for item in results.values()), "suites": results}
