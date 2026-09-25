# Council review checkpoint

Date: 2026-09-04T02:40Z. Integrator: Claude (front line), on `greyforge`.

## The gate opened, and it was never a credential

`claude-cli/claude-opus-5` answers. The Council lane is up and the staged
reviews are running.

The diagnosis recorded through 2026-09-03 was wrong in its conclusion, though
right in its symptoms. Every symptom (HTTP 401 `invalid x-api-key` on
`anthropic/*` and `claude-cli/*`, "Unknown model" on
`claude-cli/claude-fable-5-1`, and "No provider token-auth plugins found" from
`setup-token`) came from one fact: **the stock Anthropic plugin was installed
but blocked by the plugin allowlist**, so its provider, its CLI backend, and
its model catalog never loaded, and requests fell through to a generic HTTP
path with an empty key.

The fix, in `~/.openclaw/openclaw.json` (backup:
session scratchpad `openclaw.json.bak`), authorized by the operator's
"fix the claude auth":

- `plugins.allow` gained `anthropic`;
- `plugins.entries.anthropic.enabled = true`.

No credential was pasted, created, or required. The plugin declares
`claude-cli` as a **CLI backend** (`cliBackendIds`), so that lane uses the
local authenticated Claude Code CLI session rather than a gateway key.
`openclaw plugins doctor` passes. `openai/gpt-5.6-sol` remains in its
provider-side rate-limit cooldown; it is no longer on the critical path.

## Dispatcher state

`scratchpad/s20-reviews/patient_dispatcher.sh` detected the lane on its next
probe and is working through **69 staged bounded review requests**, one
session each, roughly seven minutes apiece.

```text
LANE_UP claude-cli/claude-opus-5 2026-09-04T02:29:23Z
DONE 260-ariadne-contract answered=1 exit=0 2026-09-04T02:36:46Z
```

Each request writes `<package>-<role>.log` beside its request file, and the
dispatcher skips a request whose log already carries its verdict key, so it is
safe to restart.

## First verdict: S20-260/S20-270 Ariadne contract review is FAIL

Retained at `machineresearch/sley-2.0/reviews/`. Two P0 findings, six P1.

**P0-1, map canonicality.** S20-210 `check_map_constant` imposes no key order,
and maps arriving as inputs or through `constant_ref`/`global_get` are never
normalized, so `equal`/`not_equal` over maps is representation equality and
both the S20-250 success value hash and the observation identity depend on
entry order. Not yet reproduced by me.

**P0-2, signed `int_shl_checked` inverted at the width boundary.** Reproduced
and confirmed at `bits = 128`, `amount = 127`:

| value | amount | current | correct |
|---|---|---|---|
| `1` | 127 | `Value(i128::MIN)` | `Failure(overflow)` |
| `-1` | 127 | `Failure(overflow)` | `Value(i128::MIN)` |

The implementation computes `signed_value.checked_mul(1_i128 << amount)`, and
`1_i128 << 127` is itself `i128::MIN`, so the multiplier is wrong at the top
of the width in both directions. Widths 8 and 32 are the only ones tested.
The fix is to compare against the width bounds without multiplying: for
`v > 0`, `v <= high >> amount`; for `v < 0`, `v >= low >> amount`.

Six P1 findings include a call-depth off-by-one (the real ceiling is 255, not
the contracted 256), ordered-map iteration following LEB128 encoded-byte order
rather than key order, the negative-zero flush being observable through
composition and unstated in section 3, and the `SLEYBC02` callee table missing
from the section 1 layout.

## What this changes about the six gates

Council model access is **open**. The other five stand: the narrowed
schema-epoch decision, succession trials (model access plus spend
authorization), the root license text, second-host attestation, and the
release decision.

## Next

1. Let the dispatcher finish; it needs roughly eight hours for 69 reviews.
2. Fix P0-2 with a boundary test at every width, then reproduce and fix P0-1.
3. Triage each verdict as it lands, into the S20-740 finding register.
