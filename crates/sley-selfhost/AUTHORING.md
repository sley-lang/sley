# Authoring the SH2 toolchain

The SH2 toolchain is a Sley program. It is constructed once by the C0 seed
assembler (`src/seed.rs`) and from then on exists as canonical SSMC1 state
(`src/canonical.rs`). This guide is for anyone writing toolchain modules
under `src/toolchain/`.

## The seed assembler

`Module::func(name, params, result, body)` declares a function. The body is a
closure over `FnB`, the structured builder:

- statements: `let_` (immutable), `var` (mutable), `set`, `expr`, `ret`,
  `ret_ok`, `ret_err`, `trap`;
- control flow: `if_`, `if_else`, `while_`, `loop_` + `break_`/`continue_`,
  `for_u64(name, start, end, body)` (no `continue_` inside it),
  `match_opt`, `match_res`, `match_var` (every case);
- expressions (`E`): `v("name")`, literals `u8c u32c u64c u128c boolc bytesc
  textc unitc`, arithmetic `add sub mul div rem shl shr` (checked; they trap
  on failure), `try_add try_sub try_mul try_shl` (raw `Result`), comparisons
  `eq ne lt le gt ge`, `and or not` (both sides always evaluate), `select`
  (only the chosen side runs), `call`, `rec`/`field`, `var_new`/`var_get`/
  `is_case`, `some none ok err`, `tup tget`, `vnew vlen vget vget_opt vset`,
  `mnew mget mget_opt mhas mins mdel`, `vhash`, `unwrap is_some is_ok`, and
  `tri(e)` (Rust's `?`: the `Ok` payload or an early `Err` return; the
  function must return `Result<_, UInt(32)>` with the same error type);
- host bridges (`HOST_ABI_V3`): `b2v` (`Bytes -> Vector<UInt(8)>`, at most
  1 MiB), `v2b`, `push` (any element type), `concat` (`Vector<Bytes> ->
  Bytes`), `slice(bytes, start, len)`, `blake3` (at most 1 MiB),
  `blake3_large`, `sha256`, `blen` (`Bytes` length).

Records and variants are declared with `Module::record` / `Module::variant`
(no generics, no recursion: epoch 1 rejects recursive type definitions).
Function names are global: prefix every function with its module prefix.

## Sley-side conventions

- Integers are `UInt(64)` everywhere except shift amounts (`UInt(32)`),
  bytes (`UInt(8)`), and error codes (`UInt(32)`). There is no width
  conversion opcode: use `rt_b` (`UInt(8) -> UInt(64)`) and `rt_byte`
  (`UInt(64) -> UInt(8)`, value below 256). There are no bitwise operators:
  use `div`/`rem`/`shl`/`shr`.
- Recursion is excluded by the bootstrap profile. Walk recursive structures
  with explicit stacks (see `scb_walk`).
- `TypeExpr` and `ConstValue` are carried as their canonical SCB1 bytes
  (`Cst.value`, `Prm.ty`, `Op.results[i]`, ...). Equality is byte equality.
- Readers take `(b, p, e)` (`Vector<UInt(8)>`, position, exclusive end) and
  return `Result<(value, new_position), UInt(32)>`; bind them with
  `rt::rd(f, "x", "p", rt::rcall("rt_rd_uvar", vec![u32c(32)]))`.
- Writers take and return `o: Vector<UInt(8)>` (`wr_u8 wr_be wr_u32 wr_u64
  wr_uvar wr_bytes`). Build large outputs as `Bytes` pieces and `concat`.
- Errors: `src/toolchain/codes.rs`. Keep existing numeric codes
  (`TYPE_*` 21000+, `GRAPH_*`/`CFG_*` 22000+, `VM_LOWER_*` 26000+).
- The program inventory is the `Inv` record (`src/toolchain/inv.rs`): one
  vector per entity kind plus `index` (entity id -> `kind * 2^32 +
  position`). Look entities up with `inv_fun`, `inv_blk`, `inv_op`,
  `inv_prm`, `inv_cst`, `inv_tdef`, `inv_imp`, `inv_ept` (all return
  `Option`).

## Performance rules

The VM interprets roughly 3 million instructions per second. Values are
immutable trees; the VM moves a value when its register is dead and
otherwise shares or copies it.

- A value updated in place (`vset`, `push`, `mins`) must be dead after the
  update: keep it in one `var` and reassign it (`f.set("xs", push(v("xs"),
  x))`). Never keep a second live copy of a large vector you mutate.
- `field`/`tget` on a live record share the selected part (no copy), but a
  following mutation of that part copies it once. `vget` copies the
  element.
- Map lookups binary-search canonical key bytes. Insert keys in ascending
  canonical order where you can (inserting out of order shifts the map).
- Memoize repeated pure work (for example per-type byte conversions) in a
  map keyed by the input bytes.
- Never re-walk a large structure per element.
- Project large parts of a record once (`f.let_("ops", field(v("inv"),
  "ops"))`) and reuse the binding: each `field` walks the selected part to
  count its value units.
- Embedding a large shared value in a new aggregate (`rec`, `tup`,
  `vnew`, `some`, ...) deep-copies it: aggregates own their parts. Pass
  large values as separate arguments instead.
- The assembler carries each distinct live block-parameter value once
  across a branch (a value bound and also pending is not passed twice).
  Long-lived values in branch-heavy code still cost one parameter per
  split; that is cheap at run time (a shared reference per edge).

## Testing

Tests live in `tests/`. `tests/common/mod.rs` runs a toolchain function
natively (`run`, `run_counted`, `result`) and builds canonical S
(`toolchain_s(entry)`, `objects_input`). A module test typically:

1. defines a test-entry Sley function (prefix `t_`) in the module's
   `define`, which loads S with `drv_load` and calls the module function;
2. builds canonical S for a program (the toolchain itself is the largest
   available corpus);
3. compares the Sley result with the native reference implementation, and
   mutates inputs to compare refusals.

Test entries never reach the build driver, so canonical S (pruned to the
driver's call closure) never contains them.

Cage every cargo command: `systemd-run --user --scope -p MemoryMax=3G
--quiet cargo ...` and use a private `CARGO_TARGET_DIR` when several builds
run at once. The host has 16 GB of RAM and other work runs on it.
