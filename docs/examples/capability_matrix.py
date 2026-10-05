#!/usr/bin/env python3
"""Probe the published workbench at distinct capability boundaries.

Successful calls establish lowering/execution for those inputs, not universal
feature support. Source-only and unreachable stages stay explicitly separate.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import stock_reorder as app


def function(name: str, params: list, returns: str, expression: object) -> dict:
    return {"fn": name, "params": params, "returns": returns,
            "blocks": [{"name": "entry", "term": ["return", expression]}]}


def probes() -> list:
    arithmetic = function("checked_add", [["x", "i64"], ["y", "i64"]],
                          "Result<i64,ArithmeticError>", ["add", "x", "y"])
    branch = {"fn": "choose", "params": [["flag", "bool"], ["x", "i64"]], "returns": "i64",
              "blocks": [{"name": "entry", "term": ["cond", "flag", ["yes"], ["no"]]},
                         {"name": "yes", "term": ["return", "x"]},
                         {"name": "no", "term": ["return", 0]}]}
    product = {"fn": "product", "params": [["items", "Vec<i64>"]],
               "returns": "Result<i64,ArithmeticError>", "blocks": [
                   {"name": "entry", "ops": [["n", "vec_len", "items"]], "term": ["br", "loop", 0, 1]},
                   {"name": "loop", "params": [["i", "u64"], ["acc", "i64"]],
                    "term": ["cond", ["lt", "i", "entry.n"], ["body", "i", "acc"], ["done", "acc"]]},
                   {"name": "body", "params": [["i", "u64"], ["acc", "i64"]],
                    "ops": [["x", "vec_get?bounds", "items", "i"], ["next", "mul?", "acc", "x"],
                            ["j", "add?bounds", "i", 1]], "term": ["br", "loop", "j", "next"]},
                   {"name": "done", "params": [["acc", "i64"]], "term": ["ok", "acc"]},
                   {"name": "bounds", "term": ["trap"]}]}
    cell = {"fn": "local_update", "params": [["x", "i64"], ["y", "i64"]], "returns": "i64",
            "blocks": [{"name": "entry", "ops": [["c", "cell", "x"], ["set", "cell_set", "c", "y"],
                                                   ["got", "cell_get", "c"]], "term": ["return", "got"]}]}
    variant = {"fn": "unwrap", "params": [["value", "Choice"]], "returns": "i64", "blocks": [
        {"name": "entry", "term": ["switch", "value", ["Empty", "empty"], ["Number", "number", "$"]]},
        {"name": "empty", "term": ["return", 0]},
        {"name": "number", "params": [["n", "i64"]], "term": ["return", "n"]}]}
    return [
        ("checked-integers", [arithmetic], [], "checked_add", [([3, 4], {"Ok": 7}),
         ([app.I64_MAX, 1], {"Err": {"ArithmeticError": "Overflow"}})]),
        ("boolean-branches", [branch], [], "choose", [([True, 7], 7), ([False, 7], 0)]),
        ("vectors-and-loops", [product], [], "product", [([[]], {"Ok": 1}), ([[2, -3, 4]], {"Ok": -24})]),
        ("named-records", [function("x_of", [["p", "Point"]], "i64", ["field", "Point.x", "p"])],
         [{"name": "Point", "record": [["x", "i64"], ["y", "i64"]]}], "x_of",
         [([{"x": 7, "y": 11}], 7), ([{"x": -2, "y": 0}], -2)]),
        ("named-variants", [variant], [{"name": "Choice", "variant": ["Empty", ["Number", "i64"]]}],
         "unwrap", [(["Empty"], 0), ([{"Number": 9}], 9)]),
        ("text-equality", [function("same", [["a", "text"], ["b", "text"]], "bool", ["eq", "a", "b"])],
         [], "same", [(["bolt", "bolt"], True), (["bolt", "Bolt"], False)]),
        ("ordered-map-lookup", [function("lookup", [["m", "Map<text,i64>"], ["key", "text"]],
                                         "Option<i64>", ["map_get", "m", "key"])], [], "lookup",
         [([[["bolt", 8]], "bolt"], {"Some": 8}), ([[["bolt", 8]], "nut"], "None")]),
        ("floating-addition", [function("float_add", [["x", "f64"], ["y", "f64"]], "f64", ["fadd", "x", "y"])],
         [], "float_add", [([1.5, 2.25], 3.75), ([-1.0, 0.5], -0.5)]),
        ("local-cells", [cell], [], "local_update", [([1, 7], 7), ([7, -2], -2)]),
        ("direct-calls", [arithmetic, function("twice", [["x", "i64"]], "Result<i64,ArithmeticError>",
                                                ["call", "checked_add", "x", "x"])], [], "twice",
         [([4], {"Ok": 8}), ([-3], {"Ok": -6})]),
    ]


def generic_frame() -> list:
    return [
        {"class": "CreateEntity", "kind": 5, "key": "generic_marker", "payload": {
            "type_parameters": [0], "parameters": ["@generic_marker.x"], "result_type": "i64",
            "entry_block": "@generic_marker.entry", "blocks": ["@generic_marker.entry"]}},
        {"class": "CreateEntity", "kind": 6, "key": "generic_marker.x", "payload": {
            "owner": "@generic_marker", "role": "Function", "ordinal": 0, "value_type": "i64"}},
        {"class": "CreateEntity", "kind": 7, "key": "generic_marker.entry", "payload": {
            "function": "@generic_marker", "parameters": [], "operations": [], "terminator": {
                "variant": "Return", "value": {"value": {"variant": "Parameter", "value": "@generic_marker.x"}}}}},
    ]


def measure(agent: Path, output: Path) -> dict:
    output.mkdir(parents=True, exist_ok=False)
    (output / "rows").mkdir()
    (output / "inputs").mkdir()
    results = []
    events: list = []
    try:
        for name, functions, types, target, cases in probes():
            workspace = output / name
            app.run(agent, workspace, "init", "--seed", "26" * 32, events=events)
            frame = output / "inputs" / f"{name}.json"
            app.write_json(frame, {"af1": 1, "afx": 1, "fns": functions, "types": types})
            trial = app.run(agent, workspace, "try", str(frame), events=events)
            app.require(trial["verdict"]["valid"] is True and trial["tests"] == [], "expected untested valid candidate")
            proposed = app.run(agent, workspace, "view", "--after", trial["handle"], events=events)["root"]
            calls = []
            for args, expected in cases:
                result = app.run(agent, workspace, "call", target, *(json.dumps(a) for a in args),
                                 "--on", trial["handle"], "--stats", events=events)
                app.require(result["result"] == expected, f"{name}: call differs")
                calls.append(result)
            committed = app.run(agent, workspace, "commit", trial["handle"], events=events)
            app.require(app.run(agent, workspace, "view", events=events)["root"] == proposed, "commit root differs")
            tests = output / "inputs" / f"{name}-tests.json"
            test_definitions = [
                {"name": f"case_{index}", "fn": target, "args": args, "expect": expected}
                for index, (args, expected) in enumerate(cases)]
            app.write_json(tests, {"af1": 1, "tests": test_definitions})
            definitions = app.run(agent, workspace, "try", str(tests), "--base-root", proposed, events=events)
            app.require(definitions["verdict"]["selected_tests"] == 0 and
                        len(definitions["tests"]) == len(cases) and all(t["pass"] for t in definitions["tests"]),
                        "new test definitions should run advisory cases with zero kernel-selected tests")
            definitions_commit = app.run(agent, workspace, "commit", definitions["handle"], events=events)
            tested_workspace = output / f"{name}-tested"
            app.run(agent, tested_workspace, "init", "--seed", "26" * 32, events=events)
            before = app.repository_files(tested_workspace)
            combined = output / "inputs" / f"{name}-combined.json"
            app.write_json(combined, {"af1": 1, "afx": 1, "fns": functions, "types": types, "tests": test_definitions})
            tested = app.run(agent, tested_workspace, "try", str(combined), events=events)
            app.require(tested["verdict"]["selected_tests"] == len(cases) and
                        len(tested["tests"]) == len(cases) and all(t["pass"] for t in tested["tests"]), "combined cases fail")
            refused = app.run(agent, tested_workspace, "commit", tested["handle"], expected=2, events=events)
            app.require("TXN_TEST_EVIDENCE_UNSUPPORTED" in json.dumps(refused), "unexpected tested commit result")
            app.require(app.repository_files(tested_workspace) == before, "tested refusal changed accepted files")
            row = {"feature": name, "representation": "canonical candidate created", "validation": "Valid",
                   "lowering": "observed via successful VM calls", "advisory_execution": "matched finite cases",
                   "authoritative_test_evidence": "not supplied by this v1 route", "untested_commit": "accepted",
                   "tested_commit": "TXN_TEST_EVIDENCE_UNSUPPORTED", "verdict": trial["verdict"],
                   "proposed_root": proposed, "calls": calls, "untested_receipt": committed,
                   "test_definition_only": {"verdict": definitions["verdict"], "advisory_tests": definitions["tests"],
                                            "commit": definitions_commit, "native_evidence": False},
                   "combined_verdict": tested["verdict"], "advisory_tests": tested["tests"], "tested_refusal": refused}
            results.append(row)
            app.write_json(output / "rows" / f"{name}.json", row)

        workspace = output / "declared-type-parameter"
        app.run(agent, workspace, "init", "--seed", "27" * 32, events=events)
        path = output / "inputs/declared-type-parameter.json"
        app.write_json(path, generic_frame())
        trial = app.run(agent, workspace, "try", str(path), events=events)
        app.require(trial["verdict"]["valid"] is True, "generic declaration must validate")
        call = app.run(agent, workspace, "call", "generic_marker", "4", "--on", trial["handle"], expected=2, events=events)
        app.require(call["error"] == "AGENT_EXECUTION_REFUSED" and "VM_LOWER_PROFILE_UNSUPPORTED" in call["detail"],
                    "expected lowering profile refusal")
        committed = app.run(agent, workspace, "commit", trial["handle"], events=events)
        row = {"feature": "declared-type-parameter", "representation": "canonical raw candidate created",
               "validation": "Valid", "lowering": "VM_LOWER_PROFILE_UNSUPPORTED", "advisory_execution": "not reached",
               "authoritative_test_evidence": "not measured", "untested_commit": "accepted", "tested_commit": "not measured",
               "scope": "one declared, unused type parameter; not a claim of executable polymorphism",
               "verdict": trial["verdict"], "execution_refusal": call, "untested_receipt": committed}
        results.append(row)
        app.write_json(output / "rows/declared-type-parameter.json", row)

        # Test the authoring boundary, not a fabricated effect/contract runtime.
        for name, kind in [("effect-definition", 11), ("contract-definition", 13)]:
            workspace = output / name
            app.run(agent, workspace, "init", "--seed", "28" * 32, events=events)
            before = app.repository_files(workspace)
            path = output / "inputs" / f"{name}.json"
            app.write_json(path, [{"class": "CreateEntity", "kind": kind, "key": "probe", "payload": {}}])
            refusal = app.run(agent, workspace, "try", str(path), expected=2, events=events)
            app.require(refusal["error"] == "AGENT_FRAME_INVALID" and
                        f"kind {kind} is not supported on the raw path" in refusal["detail"], "unexpected boundary")
            app.require(app.repository_files(workspace) == before, "refused authoring changed accepted files")
            row = {"feature": name, "representation": "canonical kind exists in schema; raw authoring refuses kind",
                   "validation": "not reached", "lowering": "not reached", "advisory_execution": "not reached",
                   "authoritative_test_evidence": "not reached", "untested_commit": "not reached",
                   "tested_commit": "not reached", "refusal": refusal}
            results.append(row)
            app.write_json(output / "rows" / f"{name}.json", row)
        app.write_json(output / "transcript.json", events)
        lines = ["# Executed capability observations\n",
                 "Published 2.0.6 Linux x86_64 workbench, exact executable hash in `result.json`. "
                 "Finite cases only. A call establishes lowering occurred but does not time it separately.\n",
                 "| Probe | Representation | Validation | Lowering | Advisory execution | Authoritative test evidence | Untested commit | Selected-test commit |",
                 "|---|---|---|---|---|---|---|---|"]
        columns = ["representation", "validation", "lowering", "advisory_execution",
                   "authoritative_test_evidence", "untested_commit", "tested_commit"]
        for row in results:
            lines.append(f"| [{row['feature']}](rows/{row['feature']}.json) | " + " | ".join(row[key] for key in columns) + " |")
        lines += ["", "For each executable row, adding only TestCase definitions to unchanged accepted code also commits: "
                  "the kernel selects zero tests, although the workbench runs the newly authored cases. "
                  "The separate combined code-and-test candidate selects two tests and refuses at commit. "
                  "These definition-only commits are not native tested admission; see each row's verdicts and receipts.", "",
                  "Effect/contract rows stop at the unsupported raw-kind reader, before body validation. "
                  "They do not establish kernel rejection of those semantic kinds or test an effectful runtime. "
                  "No native supervisor, host effect adapter or authoritative test evidence was exercised.", ""]
        (output / "MATRIX.md").write_text("\n".join(lines), encoding="utf-8")
        report = {"result": "PASS", "agent_sha256": app.AGENT_SHA256,
                  "script_sha256": app.sha(Path(__file__).read_bytes()),
                  "helper_sha256": app.sha(Path(app.__file__).read_bytes()),
                  "rows": len(results), "successful_execution_rows": 10, "advisory_cases": 20,
                  "native_test_evidence_produced": False,
                  "files": {str(p.relative_to(output)): app.sha(p.read_bytes())
                            for folder in [output / "rows", output / "inputs"] for p in sorted(folder.iterdir())},
                  "transcript_sha256": app.sha((output / "transcript.json").read_bytes())}
        app.write_json(output / "result.json", report)
        return report
    except (ValueError, OSError, KeyError) as error:
        app.write_json(output / "result.json", {"result": "FAIL", "detail": str(error)})
        raise
    finally:
        app.write_json(output / "transcript.json", events)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--agent", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(measure(app.checked_agent(args.agent), args.output.absolute()), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
