# Decision packet — S20-410 negotiate() validation-failure code (round 7h)

Baseline `38fc94a`. Found by the smp1 fuzz oracle (deterministic seed
`seed-0004`, minimized regression `fuzz/regressions/S20_700_SMP1_001.json`).

## Divergence

`negotiate()` (`crates/sley-protocol/src/lib.rs:864`) documents:
"Returns `PROTOCOL_NO_COMMON_PROFILE` when no common version, epoch, or
method exists, **or the hellos fail validation**."

The implementation calls `client.validate()?` / `server.validate()?`
first (`:865-866`), and `Hello::validate()` (`:735-758`) fails with
`PayloadInvalid` (shape-valid but semantically invalid hellos:
unsorted/oversize lists, reserved method tags, bad features) or
`LimitExceeded`. A decoded-but-invalid hello therefore negotiates to
`PayloadInvalid`, not `NoCommonProfile`.

## Why it surfaced now

The bare-record lane (`check_hello`) never decoded an `Ok` hello, so its
`NoCommonProfile`-only Err arm never fired. Wiring the shared
negotiation oracle into `check_frame`'s Hello arm (round 7h) executes
negotiation on real fixture hello frames, and a decodable-but-invalid
frame reached the validate-then-negotiate path.

## Target disposition (landed, fuzz lane)

`check_negotiated_hello` accepts `NoCommonProfile` (incompatible valid
hellos) and `PayloadInvalid` (decoded-but-invalid hellos); any other code
still panics. The oracle stays fail-closed on unknown codes.

## Operator decision needed (S20-410 owner)

Either amend the `negotiate()` doc sentence to name `PayloadInvalid`
(and `LimitExceeded`) as validation-failure outcomes, or change the
implementation to map validation failures to `NoCommonProfile` (protocol
semantics change: a malformed hello would then be indistinguishable
from an incompatible one). The fuzz oracle follows the decision either
way. No production code was changed in this wave.
