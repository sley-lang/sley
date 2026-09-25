# Decision packet — pinned `clang-18` provisioning result (target-closure wave)

Baseline `ba10c41` (`HEAD == main == origin/main`). Lane STOPPED per
operator direction: canonical provisioning cannot be achieved from this
session; the override is NOT promoted to normative status.

## Declared pin (unchanged, not weakened)

- C compiler default: `clang-18` (every `scripts/run_*_persistent_fuzz.py:CLANG`).
- libFuzzer runtime default:
  `/usr/lib/llvm-18/lib/clang/18/lib/linux/libclang_rt.fuzzer-x86_64.a`.
- Rust: `nightly-2026-02-27` (present; unrelated to this packet).

## Provisioning attempt (this session, mechanical record)

1. `clang-18` is absent from `PATH`; `/usr/lib/llvm-18/` does not exist.
   Host carries `clang version 22.1.8` (`/usr/bin/clang`, `clang-22`) with
   runtime `/usr/lib/clang/22/lib/linux/libclang_rt.fuzzer-x86_64.a`.
2. Sanctioned system mechanism on this Arch (Omarchy) host is
   `pacman`/`yay` (`extra/clang18 18.1.8-2`, depends `llvm18-libs`, `gcc`,
   `compiler-rt18` — all published and fetchable; network to Arch mirrors
   verified reachable).
3. `yay -S --noconfirm --needed clang18 compiler-rt18` fails at privilege
   escalation: `sudo: a password is required` (no passwordless sudo, no
   askpass helper, no `doas`/`polkit`). This session runs as an unprivileged
   user and cannot write the pinned system paths.
4. Docker client exists (`29.7.2`) but the daemon socket is denied
   (`permission denied ... /var/run/docker.sock`), so an `llvm-18`
   container lane is not available from this session either.

## Determination

- Canonical provisioning is **blocked on privilege, not on technical
  incompatibility**: the pinned `clang-18` family is published for this
  distro and there is no evidence of substantive incompatibility. No
  re-pin is authorized on this basis.
- A user-space extraction (Arch packages unpacked under `$HOME`) was
  considered and REJECTED for this lane: it would not populate the pinned
  `/usr/lib/llvm-18/...` path, so the absolute-path runtime default would
  still require `SLEY_FUZZ_LIBFUZZER_A` — i.e. still an override — and it
  is not the sanctioned system mechanism. It must not be presented as
  canonical evidence.
- All proofs of record therefore remain override-derived (`SLEY_FUZZ_CC` /
  `SLEY_FUZZ_LIBFUZZER_A` → clang 22.1.8), honestly labeled
  `toolchain_overridden: true` / `pinned default unproven on this host` in
  every `last_local_proof`. That labeling stands.

## Operator options (no action taken here)

1. Provision `extra/clang18 + compiler-rt18` with elevation (one
   `pacman -S clang18 compiler-rt18` as root), then re-run the affected
   fuzz/instrumentation proofs with a clean environment (no `SLEY_FUZZ_*`)
   and diff artifact/coverage/sanitizer/behavior against the
   override-derived evidence.
2. Direct an `llvm-18` container or CI lane to mint one proof under the pin
   (the closing condition reviewers named), with host-identity provenance
   recorded rather than folded into this host's attestation.
3. Explicitly re-pin the qualification toolchain family-wide (requires
   separate operator approval; NOT done here).

The canonical `clang-18` pin is preserved. This packet changes no source,
no contract, and no verdict.
