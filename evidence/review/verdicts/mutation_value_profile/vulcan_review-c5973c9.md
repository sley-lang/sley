## Review: S20-130 oracle-independence repair (vulcan_review, mutation_value_profile)

**Baseline:** `git rev-parse HEAD` = `c5973c90180d03402f0d2e5d2d91e941ef5dc58d`. Match. Untracked paths present (`.forge/slices/*`, `evidence/review/requests/`) are outside the reviewed files.

**Delta scope:** `733d2fa..5b70052` is a single commit touching only `oracle/scb1/src/sley2_scb1_oracle/entity_read.py` (+41/-9). `git diff --stat 733d2fa HEAD` on both checkers and the contract doc is empty, and the checker's last change (`197198d`) is an ancestor of `733d2fa`. The checker was therefore the gate that caught the `subprocess` import, and it was not tuned to admit the repair.

### Implementation (`entity_read.py:2982-3023`)

- **No process spawn, no git dependency.** `import subprocess` removed; remaining imports are `argparse`, `hashlib`, `json`, `struct`, `pathlib`, `typing`, `blake3`, `unicodedata2`, and package siblings. No `os`, `importlib`, `ctypes`, `cffi`, `exec`, `eval`. Resolution is pure file reads.
- **Fail-closed.** Missing `.git` -> `FileNotFoundError` on `HEAD`. Non-`gitdir:` pointer -> `ValueError`. Symbolic ref with neither loose file nor `packed-refs` entry -> `ValueError` or `FileNotFoundError`. Anything not exactly 40 hex chars (empty, garbage, chained symref, SHA-256 object id) -> `ValueError`. `refresh()` does not catch, so the run aborts nonzero. There is no path that emits a wrong SHA silently.
- **Byte-identical evidence.** Main checkout: `.git/HEAD` = `ref: refs/heads/main`, `.git/refs/heads/main` = `c5973c90180d03402f0d2e5d2d91e941ef5dc58d`, identical to `git rev-parse HEAD`. Loose ref correctly takes precedence over `packed-refs`, matching git. Packed-refs parsing skips `#` headers and `^` peel lines and matches on exact ref name.
- **Detached HEAD:** raw SHA in `HEAD` passes the hex check directly. All 47 review pins under `~/cache/worktrees/` are detached and would resolve.
- **Worktree, branch-attached (defect):** `.git/worktrees/at-mw-02/HEAD` = `ref: refs/heads/feat/at-mw-02-entity-reads`; that directory contains `commondir` (`../..`) but no `refs/` and no `packed-refs`. The function joins `ref` and `packed-refs` onto the worktree gitdir rather than the common dir, so it raises `FileNotFoundError` where `git rev-parse HEAD` succeeds. Fail-closed, but the docstring's "including worktree pointer files ... reporting exactly what `git rev-parse HEAD` would" overclaims.

### Checkers

- `scripts/check_oracle_independence.py` unchanged; forbidden marker list still includes `subprocess`, `popen`, `os.system`, `ctypes`, `cffi`, plus Rust-implementation markers. I reproduced its substring scan with `git grep` over the exact scan set (`oracle/scb1/src/**/*.py` plus the 18 coverage-map scripts) at `c5973c9`: zero hits for every marker; the only `spec_from_file_location` in the set (`check_merge_vector.py`) loads another `scripts/` oracle, which the rule allows. Static outcome: PASS.
- `scripts/check_mutation_value_codecs.py` unchanged; it invokes the independence checker as a subprocess (line 659), which is fine since it is a gate script, not an oracle, and is not in the coverage map.

### Contract

`docs/spec/MUTATION_VALUE_CODEC_V1.md` names the independent Python oracle as a completeness-gate input but states no HEAD-resolution rule; the S20-130 independence contract is carried by the checker. No contract drift.

### Record/evidence

Committed corpus `conformance/entity-read/v2/{accepted,rejected}.json` records `refresh_head_revision` = `01cc239ebc28cd6f6516f42dc6475deb18fd5dc8`, a real ancestor commit; the repair did not regenerate the corpus and the emitted format (40 lowercase hex) is unchanged, so existing evidence remains consistent.

### Assumptions and limitations

- Python execution was not approved in this non-interactive session, so I could not run the checker or exercise `git_head_revision` dynamically. The checker's logic is a deterministic substring scan, which I reproduced with `git grep`; the worktree finding is derived from on-disk gitdir layout, not from a run.
- Per the READ-ONLY mandate I created no evidence artifacts; the evidence is the SHA and file-content observations above.
- I did not touch or evaluate prior Nabu or Codex verdicts.

VERDICT: PASS
SECTION: mutation_value_profile
FIELD: vulcan_review
SCOPE_SHA: c5973c90180d03402f0d2e5d2d91e941ef5dc58d
FINDINGS:
[P3] [implementation] oracle/scb1/src/sley2_scb1_oracle/entity_read.py:3005-3011 - branch-attached linked worktrees resolve refs and packed-refs from the worktree gitdir instead of `commondir`; refresh aborts with FileNotFoundError where `git rev-parse HEAD` succeeds (fail-closed, no wrong SHA possible; detached worktrees unaffected). Honor `commondir` or narrow the docstring claim.
[P3] [record] scripts/check_mutation_value_codecs.py:523-527 - pins `vulcan_independent_review: DEFERRED_FORGE_OAUTH_401` in `evidence/validation/s20-350-candidate-closeout-v1.json`; recording this dispatch's disposition requires a lockstep checker update or the gate reports "closeout review disposition drift".
[P4] [implementation] oracle/scb1/src/sley2_scb1_oracle/entity_read.py:3021 - accepts uppercase hex without lowercasing; git never writes uppercase refs so evidence stays byte-identical in practice, but normalizing would make the guarantee unconditional.
SUMMARY: The S20-130 repair removes the last process-spawn and git-executable dependency from the independent oracle: HEAD resolution is pure `.git/HEAD` -> loose ref -> `packed-refs` file reads with a strict 40-hex gate, and every unresolvable state raises so refresh aborts rather than recording a wrong revision. The main-checkout path is byte-identical to `git rev-parse HEAD` (verified against `.git/refs/heads/main` = `c5973c9...`), detached HEADs resolve directly, and packed refs are parsed with correct precedence. Neither checker nor the contract changed in or after the delta, and a `git grep` reproduction of the independence scan over the exact scan set at `c5973c9` finds no forbidden markers. The one substantive gap is branch-attached linked worktrees, where the function ignores `commondir` and fails closed instead of resolving; this is a functional shortfall against the docstring, not an independence or evidence-integrity defect, so the lane passes with follow-ups recorded. Confidence: high on independence and fail-closed behavior (static trace plus on-disk verification), medium on dynamic behavior since Python execution was unavailable in this session.
