"""Frozen prompt and per-arm tool training material for live attempts."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path

from bench.live.manifest import ARMS, canonical_json_bytes


ROOT = Path(__file__).resolve().parents[2]
CORPUS = ROOT / "bench" / "corpus" / "v1" / "tasks.json"
LEGACY_TOOL = ROOT / "bench" / "live" / "legacy_tool.py"
TASKS = {
    task["id"]: task
    for task in json.loads(CORPUS.read_text(encoding="utf-8"))["tasks"]
}
PROMPT_TEMPLATE = """Complete exactly the frozen programming task below in the isolated workspace.
Read `.sley-live/TOOLING.md` before acting and use only its documented surface.
You may inspect and change candidate program files. Do not modify `.sley-live`.
Run the available checks before finishing. Do not access the network or files outside the workspace except through the documented tool command.
Preserve unrelated behavior and make no prose-only substitute for a working change.

Trial seed: {seed}
Frozen task JSON:
{task_json}
"""

RAW_TOOLING = """# Raw-files arm tool surface

Candidate files are ordinary files in this workspace. Use shell file-reading and editing commands.
Run the supplied Python tests with:

```sh
python3 -m unittest -v
```

The independent oracle will rerun the tests and additional structural checks after the attempt.
Do not modify `.sley-live`.
"""

LEGACY_TOOLING = """# Frozen Sley 1.2.0 arm tool surface

Edit the candidate `.sley` and JSON files in this workspace. The only Sley executable available for task work is the verified frozen 1.2.0 artifact through `.sley-live/sley-tool`.

Commands:

```text
.sley-live/sley-tool check FILE
.sley-live/sley-tool run FILE [CAP ...]
.sley-live/sley-tool verify FILE [EXTRA ...]
.sley-live/sley-tool test FILE
.sley-live/sley-tool query FILE [KIND [MODULE]]
.sley-live/sley-tool lint FILE
.sley-live/sley-tool plan FILE
.sley-live/sley-tool graph FILE SURFACE
.sley-live/sley-tool graph-diff BASE OURS THEIRS
.sley-live/sley-tool machine FILE TASK JSON_OBJECT
```

Each invocation privately stages and verifies the same pinned artifact. Paths must name regular files directly in this workspace. The independent oracle will stage and verify the artifact again.
Do not modify `.sley-live`.
"""

SLEY2_TOOLING = """# Sley 2.0 arm tool surface

Work against the served repository in this workspace through `.sley-live/sley-tool`.
Each invocation runs one disposable server session: it imports the staged
base pack, opens a session, executes exactly one command, and closes.
No state survives across invocations except the repository files and
`final_candidate.hex`, which only `finish` writes.

Request bodies cross as lowercase hex and stay opaque per the protocol
bridge contract (owner bodies have no JSON form); reads additionally
return decoded JSON views for inspection. The tool assembles candidate
records mechanically from structured operations and derives preconditions
from live reads; it judges nothing — the server refuses invalid input and
the independent oracle alone decides acceptance.

Commands:

```text
.sley-live/sley-tool inventory
.sley-live/sley-tool read ENTITY_HEX
.sley-live/sley-tool sig ENTITY_HEX
.sley-live/sley-tool open
.sley-live/sley-tool revision TX_HEX
.sley-live/sley-tool caps
.sley-live/sley-tool budgets
.sley-live/sley-tool raw METHOD BODY_HEX
.sley-live/sley-tool propose OPS_JSON
.sley-live/sley-tool append RECORD_HEX OPS_JSON
.sley-live/sley-tool compose RECORD_HEX OPS_JSON
.sley-live/sley-tool inspect RECORD_HEX
.sley-live/sley-tool validate RECORD_HEX
.sley-live/sley-tool finish RECORD_HEX
.sley-live/sley-tool side ours|theirs
```

`side` reports the frozen branch contents with decoded bodies (read-only):
the branch states are trial inputs, and the merged outcome is composed
from them through `propose`/`compose`/`finish` like any other change.


`open` reports the accepted-head summary (`tx`, `root`, `policy`,
`workspace`, `epoch`, counts, `receipt`) and, only when the head's index
snapshot is already materialized, its identity as `snapshot` (the binding
root-backed queries name; a bounded `query.root` materializes it).
`revision` takes a transaction id from a prior `open` (or any other
reported tx), because no state survives across invocations.
`raw` sends one request body to one of these methods and returns the
response body as hex (`failed` true for a refusal): candidate.append,
candidate.create, candidate.discard, candidate.inspect, candidate.validate,
capsule, compare, entity.signature, entity.version, handle.expand,
query.continue, query.restricted, query.root, refs.list, refs.resolve,
revision.read, session.budgets, session.capabilities, workspace.open.

Bounded root queries (`raw query.root BODY_HEX`, continuation
`raw query.continue BODY_HEX`). Integers are big-endian; ids are 32 bytes.

```text
request body:
  "SLEYRQQ1"                     8 bytes
  u32 1, u32 1                   format, profile
  snapshot, epoch, root, workspace   32 bytes each, from `open`
  u32 2, u32 1                   completeness, limits profile
  u64 max_entities               1..65535 entries per page
  u64 max_edges                  1..400000
  u32 max_depth                  0..65535
  u64 max_response_bytes         1..67108864
  u64 max_work                   1..100000000
  u32 paging                     1 = single page, 2 = allow continuation
  cursor                         u32 1 (none) | u32 2, u32 1, entity id
  u32 class, class body:
    2   entity                   id -> its kind and object id
    4   entities of one kind     u32 kind (4 = type definition, 9 = constant)
    14  reverse impact closure   u64 n (1..65535), then n seed ids, strictly ascending
response body:
  "SLEYRQR1", u32 1, u32 1, query id, snapshot, epoch, root, workspace,
  u32 2, u32 1, the five limits, u32 paging, cursor (echoed request)
  u32 class, u64 total (whole result), u64 returned (this page),
  u32 truncated (1 no, 2 yes),
  next cursor (same encoding), u32 depth, u64 work, u64 bytes, u32 class
  result, class 2: u32 kind, object id, fingerprint (u32 1 | u32 2, 32 bytes)
  result, classes 4 and 14: u64 count, then count entity ids
```

The server rebuilds every request from the accepted head and refuses
`QUERY_SNAPSHOT_MISMATCH` unless snapshot, epoch, root, and workspace equal
the head's. When `open` reports no `snapshot`, send one well-formed query
with 32 zero bytes as the snapshot. It is refused: with
`QUERY_SNAPSHOT_MISMATCH` when the query is otherwise answerable (the
server then materializes the head snapshot while answering, if the index
cache write succeeds, and the next `open` reports it), or with the owner's
code (for example `INDEX_SNAPSHOT_ROOT_INCOMPLETE`) when it is not, in
which case nothing materializes and no `snapshot` is ever reported. With
paging = 1 a result larger than `max_entities` is refused
(`QUERY_REQUIRED_FACT_OMITTED`); that refusal is counted, not rejected.

A page with truncated = 2 must be continued: send `query.continue` with the
same body (paging 2) and the cursor set to that page's next cursor, from
this or any later invocation; continuation is bound to the query and the
cursor, not to the invocation. Only `query.continue` carries a cursor. The
trial is rejected if any truncated page is never continued, if a successful
`query.continue` does not continue a still-open page of the same query at
exactly its next cursor, or if a page with no continuation route (a
`query.root` sent with a cursor, `query.restricted`, `refs.list`) reports
omissions or truncation; a refused `query.continue` continues nothing.

Budgets: every tool reply (the whole JSON the command prints, in which a
body appears as hex, twice its byte length) must stay within 1048576 bytes,
so keep `max_response_bytes` at or below 524000; a larger reply fails the
trial. All replies together must stay within 4194304 bytes per trial (the
judged total; on the mediated route the runner's capture also stops a trial
outright at 8388608).
`inventory` and `side` read the whole store and reject a CONTEXT trial.

`inventory` lists served object ids with decoded kinds. `read`/`sig` show
an entity with its decoded body: edit by authoring the modified body as
structured JSON (field names per the decoded view) or, for scalar fields,
plain values. `propose` takes a JSON list of operations
`[{class, kind, target, field_tag, payload}]` with classes CreateEntity,
ReplaceEntityVersion, DeleteEntityBinding, SetScalarField,
ReplaceTypedField, RetargetReference, InsertOrderedChild,
RemoveOrderedChild, MoveOrderedChild; targets of fresh entities must be
null (identities derive inside assembly). The tool fills ordinals,
ExactEntityVersion/ExactContainerVersion preconditions from live reads,
the empty capability projection over the fixed trial principal, the
frozen validation profile, a fresh nonce, and the fixed expiry bound,
then creates and validates the candidate and reports its bytes.
`valid` means the server's validation decision is Valid (the verdict
is read from the result object, never assumed from delivery).
Created reports also list `identities`: the deterministic derived
entity of every CreateEntity, in order — derivation, not a validity
claim; later phases reference them, and only Valid wholes finish.
`append` extends a record from an earlier `propose`/`append` in the same
trial (pass back its exact reported bytes) through the server's
`candidate.append`: the base contributes server-stored bytes, the new
operations assemble standalone, and the server rebuilds the
concatenation. Composition never commits: intermediate records write
nothing, and only `finish` writes `final_candidate.hex`.
`compose` reassembles the FULL op list (earlier phases resupplied
verbatim first, new operations after) under an earlier record's own
nonce: created identities re-derive deterministically, so earlier
identities survive byte-for-byte, and the whole validates through the
normal create path. Later operations may reference earlier created
identities in their payloads — that is how multi-phase programs name
things that did not exist when the first phase was authored.
`finish` re-validates the given bytes and writes `final_candidate.hex`
only for a Valid decision; anything else is refused unwritten.
Only that file is judged. Commit, merge, execute, export, import,
report, session management, and tests are unavailable by construction.
Every invocation appends a sealed entry to the tool-managed
`.sley-live-transcript.jsonl` evidence chain (plus cumulative counters
in `.sley-live-usage`); a `.sley-live-seed` marker tracks the staged
pack. These tool-managed files are access evidence: do not modify them.
Do not modify `.sley-live`.
"""


def prompt_template_digest() -> str:
    return hashlib.sha256(PROMPT_TEMPLATE.encode("utf-8")).hexdigest()


def build_prompt(task_id: str, seed: int, arm_id: str) -> bytes:
    """Build the arm-neutral task prompt; arm_id is validated but never rendered."""

    if arm_id not in ARMS or task_id not in TASKS:
        raise ValueError("LIVE_PROMPT_INVALID")
    if isinstance(seed, bool) or not isinstance(seed, int) or seed < 0:
        raise ValueError("LIVE_PROMPT_INVALID")
    task_json = canonical_json_bytes(TASKS[task_id]).decode("utf-8")
    return PROMPT_TEMPLATE.format(seed=seed, task_json=task_json).encode("utf-8")


def _files(arm_id: str) -> dict[str, tuple[bytes, int]]:
    if arm_id == "raw_files":
        return {"TOOLING.md": (RAW_TOOLING.encode("utf-8"), 0o444)}
    if arm_id == "sley_1_2_0":
        launcher = (
            "#!/usr/bin/python3\n"
            "import sys\n"
            f"sys.path.insert(0, {str(ROOT)!r})\n"
            "from bench.live.legacy_tool import main\n"
            "raise SystemExit(main())\n"
        ).encode("utf-8")
        return {
            "TOOLING.md": (LEGACY_TOOLING.encode("utf-8"), 0o444),
            "sley-tool": (launcher, 0o555),
        }
    if arm_id == "sley_2_0":
        launcher = (
            "#!/usr/bin/python3\n"
            "import sys\n"
            f"sys.path.insert(0, {str(ROOT)!r})\n"
            "from bench.live.sley2_tool import main\n"
            "raise SystemExit(main())\n"
        ).encode("utf-8")
        return {
            "TOOLING.md": (SLEY2_TOOLING.encode("utf-8"), 0o444),
            "sley-tool": (launcher, 0o555),
        }
    raise ValueError("LIVE_TOOLING_UNAVAILABLE")


def tooling_digest(arm_id: str) -> str:
    inventory = [
        {
            "mode": mode,
            "path": path,
            "sha256": hashlib.sha256(content).hexdigest(),
            "size": len(content),
        }
        for path, (content, mode) in sorted(_files(arm_id).items())
    ]
    return hashlib.sha256(
        b"sley2.live-tooling.v1\0"
        + canonical_json_bytes(
            {"arm_id": arm_id, "contract": "sley2.live-tooling.v1", "files": inventory}
        )
    ).hexdigest()


def stage_tooling(arm_id: str, workspace: Path) -> None:
    files = _files(arm_id)
    root = Path(workspace)
    control = root / ".sley-live"
    if not root.is_dir() or control.exists() or control.is_symlink():
        raise ValueError("LIVE_TOOLING_DESTINATION")
    control.mkdir(mode=0o700)
    for name, (content, mode) in sorted(files.items()):
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        descriptor = os.open(control / name, flags, 0o600)
        try:
            written = 0
            while written < len(content):
                count = os.write(descriptor, content[written:])
                if count <= 0:
                    raise ValueError("LIVE_TOOLING_WRITE_FAILED")
                written += count
            os.fchmod(descriptor, mode)
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    control.chmod(0o555)
