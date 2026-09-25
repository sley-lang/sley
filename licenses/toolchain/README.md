# Toolchain notices

The `sley` release binary is statically linked for `x86_64-unknown-linux-musl`.
Besides the crates in `Cargo.lock`, it carries the Rust standard library and
musl libc from the pinned toolchain. Their notices are vendored here so the
release build doesn't depend on the local toolchain's documentation, and
`scripts/generate_third_party_licenses.py` appends them to
`THIRD_PARTY_LICENSES`.

| File | Component | Source |
|---|---|---|
| `rust-1.93.0-COPYRIGHT-library.txt` | Rust standard library 1.93.0 (MIT OR Apache-2.0, plus the notices of its bundled dependencies) | `share/doc/rust/COPYRIGHT-library.html` from the Rust 1.93.0 toolchain (sha256 `d93d65853736bde4e8649691233a98848ba709673873aeb39487bc1ca6ee292a`), converted to plain text with its markup removed and its wording unchanged |
| `musl-1.2.5-COPYRIGHT` | musl libc 1.2.5 (MIT), the version in the toolchain's self-contained `libc.a` | `COPYRIGHT` at tag `v1.2.5` of `git://git.musl-libc.org/musl`, unchanged |

Update these files together with `rust-toolchain.toml`.
