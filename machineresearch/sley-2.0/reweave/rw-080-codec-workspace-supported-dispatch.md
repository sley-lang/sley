# RW-080 §1.1 program slice 25: Workspace supported dispatch and flat supported value — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice adds Workspace
kind 1 to the bounded supported decode and encode images. It is development
evidence, not accepted runtime authority. RW-080 remains BLOCKED and R2
remains NOT_READY; independent review and acceptance debt are unchanged.

## 1. Uniform supported value

The aggregate codec now uses one canonical six-field tuple inside its codec
result:

`(kind, entity_id, first_bytes, second_bytes, third_bytes, scalar)`.

EntryPoint uses `first_bytes` for its function and `scalar` for exposure.
Namespace and PolicyBinding use the first two byte fields. DependencyBinding
uses all three. Package and Workspace carry their already validated canonical
body in `first_bytes`. Unused byte fields must be empty and unused scalar
fields must be zero. The encode entry checks those reserved positions before
dispatch. This replaces the nested five-arm `Result` representation while
preserving the stored bytes, refusal codes and declared-kind matching rules.

Namespace and PolicyBinding now share one identical normalization block. That
removes a duplicated tuple construction and recovers image capacity without
changing either body decoder.

## 2. Workspace integration

Workspace kind 1 uses the exact bounded profile constructed in program slice
24: a 49-byte canonical body with one 32-byte root namespace and four empty
sets. Workspace and Package share one counted body comparator. The declared
kind participates in template selection, so a valid Workspace body declared
as Package, or a valid Package body declared as Workspace, returns
`SSMC_RESERVED_FIELD_PRESENT`.

The paired supported encoder shares one fixed-body witness composer. After
the body checker selects the profile, the composer chooses the canonical
payload length, body length and object prefix, verifies the entity width,
derives the object digest through RHW1 and emits byte-identical stored bytes.
No host semantic codec, unchecked body path, charging change or execution
limit increase was introduced.

## 3. Evidence and resources

All six supported kinds decode and encode through their aggregate images and
round trip through decode then encode. Cross-profile Workspace/Package bodies,
reserved tuple positions, declaration mismatches, known unsupported kinds and
unknown kinds fail closed with their existing typed codes.

Under the unchanged protected limits:

| Direction | Kind | Fuel | Instructions | Peak value units |
|---|---|---:|---:|---:|
| Decode | Workspace 1 | 25,103 | 3,173 | 633,753 |
| Decode | Package 2 | 30,370 | 3,598 | 850,455 |
| Decode | Namespace 3 | 29,138 | 3,661 | 620,541 |
| Decode | EntryPoint 16 | 27,825 | 3,493 | 602,266 |
| Decode | PolicyBinding 17 | 41,677 | 4,688 | 876,437 |
| Decode | DependencyBinding 18 | 28,271 | 3,296 | 999,597 |
| Encode | Workspace 1 | 17,683 | 2,044 | 726,313 |
| Encode | Package 2 | 20,516 | 2,269 | 941,007 |
| Encode | Namespace 3 | 26,284 | 3,405 | 997,018 |
| Encode | EntryPoint 16 | 22,530 | 2,853 | 930,875 |
| Encode | PolicyBinding 17 | 25,756 | 3,339 | 968,676 |
| Encode | DependencyBinding 18 | 3,789 | 1,041 | 399,728 |

The limits remain 100,000 instructions, 1,000,000 fuel, 1,000,000 value
units and 100,000 output units. The narrow DependencyBinding decode margin is
an explicit regression guard, not evidence for widening the limit.

## 4. Validation and review state

Validation for this slice:

- `cargo test -p sley-vm --test rw080_codec_program_outer`
- `cargo clippy -p sley-vm --test rw080_codec_program_outer -- -D warnings`
- `cargo test -p sley-vm --test rw080_codec_uvar`
- `cargo test -p sley-vm --test rw080_codec_envelope`
- `cargo test -p sley-vm --test rw080_codec_scaffold`
- `cargo test -p sley-vm --test rw080_checker_scaffold`
- `cargo test -p sley-vm --test rw080_lower_scaffold`
- `cargo test -p sley-scb1 --locked --lib`
- `cargo test -p sley-mutate --locked --lib object`
- `cargo fmt --all -- --check`
- `python3 scripts/build_anti_goal_conformance.py --check`
- `python3 -m json.tool evidence/reweave/sh2-work-items.json`
- `git diff --check`

Local strict review checks the uniform tuple contract, all six dispatch paths,
Workspace/Package profile identity, exact native bytes, typed refusals,
adapter confinement and frozen resource limits. Independent review is
unavailable in this session, so no gate, ledger, acceptance verdict or
runtime-authority state is promoted.
