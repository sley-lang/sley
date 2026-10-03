# Private native worker vector v1

`observed-input.bin` is one synthetic `SLEYWRK1` request carrying a complete
portable `SLEYPRG1` program. The selected Sley `TestCase` calls a pure Boolean
identity function with `true` and expects `true`. `observed-report.bin` is its
canonical raw `SLEYNEX1` worker output. Neither file contains host
measurement, a signature, or native test admission authority.

The `sley-test-runner` fixture test reconstructs both files from the typed
plan, state root, objects, and pure VM result, then compares exact bytes. The
`sley-cli` real-binary test passes the input file through the supervisor's
rendered worker argv and compares exact stdout to the report file. The same
runner tests cover an execution rejection and an input-hash substitution.

| File | Bytes | SHA-256 |
|---|---:|---|
| `observed-input.bin` | 2370 | `4c769be0547685d0ae2115953fd0f52eb50acb215236ba87cd0d3c7c02c2e357` |
| `observed-report.bin` | 618 | `66bb48b34f1686730599a5412cdd1d195b0b6eb307f3cec7a8f3cf23efd7274c` |

This vector checks the worker transport and canonical report bytes. It is
derived from the Rust VM owner and is not an independent VM semantics oracle.
