# Decision packet — S20-410 negotiate() validation-failure code (round 7h, corrected)

Baseline `38fc94a`. Found by the smp1 fuzz oracle (deterministic seed
`seed-0004`, minimized regression `fuzz/regressions/S20_700_SMP1_001.json`).

## Corrected root cause (supersedes the round-7h draft of this packet)

The first draft blamed a decoded-but-invalid client hello. Re-examination
proved the trigger was the target's own `server_hello()` fixture: it built
`methods` from `Method::ALL`, which includes four reserved tags
(`Diagnostics`, `RefMoveProtected`, `TestsSelected`, `TestsAffected`), and
`Hello::validate()` rejects reserved tags. `server.validate()` therefore
failed every `negotiate_identity` call with `PayloadInvalid`, so the Ok
arm never executed on any input. Fixed in the target (reserved tags
filtered); the minimized input (the accepted fixture server hello, a
valid hello) now negotiates `Ok`.

## Remaining doc-accuracy question (S20-410 owner)

`negotiate()` (`crates/sley-protocol/src/lib.rs:864`) documents:
"Returns `PROTOCOL_NO_COMMON_PROFILE` when no common version, epoch, or
method exists, **or the hellos fail validation**."

The implementation returns `validate()`'s own codes (`PayloadInvalid`,
`LimitExceeded`) for a decoded-but-invalid client hello against a valid
server. Either amend the sentence to name those outcomes, or map
validation failures to `NoCommonProfile` (protocol semantics change: a
malformed hello becomes indistinguishable from an incompatible one).

## Target disposition (landed, fuzz lane)

`check_negotiated_hello` accepts `NoCommonProfile` (incompatible valid
hellos) and `PayloadInvalid` (decoded-but-invalid hellos); any other code
still panics. The acceptance is correct under either resolution of the
doc question above. No production code was changed in this wave.
