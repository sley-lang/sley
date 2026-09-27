# search: verified local repairs (reference)

    sley-agent search <fn> --public <cases.json> [--from <cK | dN | dN@rK>]
                      [--max-neighbors <n>] [--max-millis <ms>] [--json]

`search` proposes small, typed changes to one function and checks each one
against the public cases you pass. It prints ranked edit frames; you apply
one with `try --on`. It never submits or commits, and it stores no
candidate.

- Seed: `--from` names a Valid candidate made from a frame, or a draft
  revision whose state is `valid` (`d1` is its latest revision). Without
  it, the function as the head states it.
- Cases: the file `try --public` and `import` read
  (`[{"name", "function", "args", "expect"}]`), with at least one case for
  `<fn>`. Every usable case in the file runs. Expected values come only
  from the file, never from running a candidate.

## Neighbors

Six generators, over the operations and terminators of `<fn>`:

    opcode swap           add sub mul div rem | shl shr | fadd fsub fmul fdiv | eq ne | lt le gt ge | and or
    operand permutation   the operands of sub div rem shl shr fsub fdiv lt le gt ge
    operand substitution  another visible value of the same type, for an operand, a returned value or a br argument
    constant nudge        an integer constant +1, -1 or negated, as a new literal
    edge swap             the targets of a cond; the targets of two switch cases whose arguments fit
    negation              a cond (or exit) condition c becomes not c, and not x becomes x

A visible value is a function parameter, a result of a dominating block,
or a parameter or earlier result of the same block. Each neighbor is an
`edit` (one operation) or a `patch` (one block) written in the seed's
dialect, so `try --on <seed>` layers it on the seed's frame. Search
compiles, assembles and validates exactly that layered frame. Changes to
code the tool generated (AF1-X continuations and shared exits) have no
frame, so they are skipped and counted.

## Ranking, bounds, output

Only kernel-Valid neighbors run the cases. The ranking is public cases
passed (descending), then edit size (ascending: 2 for a permutation or an
edge swap, 1 otherwise), then generator order (the table above), then
generation index. Neighbors are generated one generator at a time, with
operand substitution last, each in program order.

Bounds per command: 64 neighbors (`--max-neighbors`, up to 4096) and
10000 ms of wall time (`--max-millis`). A seed case may use 10000000 fuel.
A neighbor's case may use ten times the seed's fuel on it, at least 1000000
and at most 10000000 (1000000 when the seed ran out). A case that runs
out is labeled `resource limit`. A reached limit, a partial evaluation and
a case that could not run (`unknown`) are labeled.

Each seed lineage gets 2 searches. A lineage is a draft together with the
candidates of its revisions and the drafts started with `try --on` one of
them; a candidate no draft made; or the head until the next commit.
`.sley/search.json` records the uses. A third search is refused.

The text shows the seed's result, the counts, the rule, the top 5
neighbors with their frames, the limits, local wall and CPU time and peak
memory, and `next:`, which is `try --on <seed>` with the top frame when it
passes more cases than the seed. `--json` lists every neighbor with its
verdict and case results. Exit status: 0 when the top neighbor passes every
case, 1 otherwise, 2 for a refusal:

    AGENT_SEARCH_NO_ORACLE      the case file is unreadable, holds no case, or none for <fn>
    AGENT_SEARCH_SEED_INVALID   the seed is refused, incomplete, a text draft or not made from
                                a frame; <fn> is not one of its functions; or its lineage used
                                its 2 searches

"Verified" means kernel-valid and evaluated against the stated public
cases, not proof of correctness for all inputs.

## Example

`diff` should subtract:

```json
{"af1": 1, "fns": [{"fn": "diff", "params": [["a", "i64"], ["b", "i64"]],
  "returns": "Result<i64,ArithmeticError>",
  "blocks": [{"name": "entry", "ops": [["r", "add", "a", "b"]], "term": ["return", "r"]}]}]}
```

```json
[{"name": "t1", "function": "diff", "args": [5, 3], "expect": {"Ok": 2}},
 {"name": "t2", "function": "diff", "args": [1, 4], "expect": {"Ok": -3}}]
```

    $ sley-agent try diff.json --public cases.json    # c1: Valid; public: 0/2 passed
    $ sley-agent search diff --public cases.json --from c1
    > search diff from c1: the seed passes 0/2 public cases (fail: t1, t2)
    > neighbors: 6 generated, 6 kernel-valid, 0 refused, 6 evaluated
    >  1. #1 opcode swap at entry.r: add -> sub (size 1): public 2/2
    >     {"af1":1,"edit":[{"fn":"diff","replace_op":"entry.r","with":["sub","a","b"]}]}
    > verified: kernel-valid and evaluated against the stated public cases, not proof of correctness for all inputs
    > next: sley-agent try --on c1 '{"af1":1,"edit":[{"fn":"diff","replace_op":"entry.r","with":["sub","a","b"]}]}' --public cases.json
