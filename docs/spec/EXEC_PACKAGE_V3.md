# EXEC_PACKAGE_V3

Status: SH2 successor (2026-10-12). Version 3. `EXEC_PACKAGE_V1` and
`EXEC_PACKAGE_V2` are preserved byte-identical as history.

- Contract: `sley2-exec-package-3`. Rust surface: `sley_vm::exec_package`
  (`EXEC_PACKAGE_V3_*`, `BOOTSTRAP_PROFILE_3_DIGEST`, `package_digests_v3`,
  `encode_package_envelope_v3`, `decode_package_envelope_v3`,
  `hydrate_package_envelope_v3`, `approve_package_v3`,
  `approve_pinned_package_v3`, `verify_package_binding_v3`) plus
  `sley_vm::execute_approved_package_v3`.
- Relation to v2: identical layout, sections, bounds, hydration, and
  approval discipline. The header binds `u32(3)`, the
  `BOOTSTRAP_PROFILE_3` digest, and `HOST_ABI_V3` version 3, so v2 and v3
  packages never share authority evidence.

## Header (package identity preimage, 316 bytes)

```text
SLEYPKG1 || u32(3) || BOOTSTRAP_PROFILE_3 digest || u32(3) ||
vm_version (3 x u32) || image || constants || layouts || imports ||
dependency (five SHA-256 section digests) || entry || epoch || root
```

The package identity is SHA-256 over the header. The envelope is the header
followed by the five sections (image, constants, layouts, imports,
dependency), each framed by a big-endian `u64` length.

## Approval routes

- `approve_package_v3(package, digests, receipt, gate)` runs every v2 gate
  cross-check against a `BootstrapProfileVersion::V3` report (native
  seed route, minted by `admit_v3_package`).
- `approve_pinned_package_v3(package, digests, receipt)` is the SH2 host
  route behind `host_admission::admit_pinned_v3_package`. The package's
  gate claims (operation and bridge counts, closure fingerprints) were
  produced by the pinned toolchain itself and are bound through the package
  digest; every structural binding (profile, ABI, cache key, limits,
  sections) is still checked.
