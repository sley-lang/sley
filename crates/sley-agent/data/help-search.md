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
  (`[{"name", "function", "args", "expect"}]`, each with an `args` array),
  with at least one case for `<fn>` whose arguments fit its parameters.
  Every usable case in the file runs. Expected values come only from the
  file, never from running a candidate.
- Tests: the TestCases of `<fn>` in the seed and in each neighbor (the
  frame's own, imported and provided ones) run too. They are evidence
  beside the public cases and are not ranked.

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
compiles, assembles and validates exactly that layered frame. A change no
such frame can state is skipped and counted, before it can take a
neighbor slot:

- code the tool generated (AF1-X continuations, checked switches, shared
  exits), or any `__` name in a frame of the authoring dialect;
- a block the seed's frame does not state whose code differs from the
  head's, unless an `arity` intent of the seed made the difference (below);
- any change to a function a `guard` intent of the seed rewrites: the
  intent refuses every frame that restates that function;
- a permutation of two nested operations (it would reorder their
  evaluation), or a block change beside the seed's own `edit`;
- a substitution that would drop a nested operation with a failure route
  (`op?`), a call, an effect, a contract check, an observation or a cell;
- `not x -> x` where `x` is a nested operation without a name (it would
  run twice).

A function the seed's `arity` intent rewrites without its frame stating it
(a caller of the changed function) is searched as the head states it: each
neighbor is a `patch` of one of its blocks, and the intent, layered in with
the seed's frame, rewrites the calls in that patch again. The values an
intent derives (its `value`) are not searched. The output says `derived:`.

## Ranking, bounds, output

Only kernel-Valid neighbors run the cases. The ranking is public cases
passed (descending), then edit size (ascending: 2 for a permutation or an
edge swap, 1 otherwise, plus every nested operation and literal a
substitution removes), then generator order (the table above), then
generation index. Neighbors are generated one generator at a time, with
operand substitution last, each in program order.

Bounds per command: 64 neighbors (`--max-neighbors`, up to 4096) and
10000 ms of wall time (`--max-millis`, up to 3600000). A seed case may use 10000000 fuel.
A neighbor's case may use ten times the seed's fuel on it, at least 1000000
and at most 10000000 (1000000 when the seed ran out). A case that runs
out is labeled `resource limit`. A TestCase runs under its declared fuel,
at most 10000000. A reached limit, a partial evaluation and a case that
could not run (`unknown`) are labeled. The wall limit is checked before
each case run, TestCase run and neighbor; a run in progress finishes, so a
command can exceed the limit by one run. The output states the bound and
the time actually taken.

An attempt gets 2 searches: every search on one accepted head of the
workspace counts, whatever its seed, function or draft (trying the same
frame again or chaining `try --on` changes nothing; a commit starts a new
head). Each search claims a slot file `.sley/search/<head>-<n>.json`
exclusively, so concurrent searches never exceed the bound. A third search
is refused; a refused search uses nothing.

The text shows the seed's result, the counts, the rule, the top 5
neighbors with their frames, the limits, local wall and CPU time and peak
memory, and `next:`, which is `try --on <seed>` with the top frame when it
passes more cases than the seed (with a `# caution:` naming the TestCases
of `<fn>` it fails, if any). `--json` lists every neighbor with its verdict,
case results and TestCase results. Exit status: 0 when the top neighbor
passes every case and every TestCase of `<fn>`, 1 otherwise, 2 for a
refusal (a flag given twice is `AGENT_USAGE_INVALID`):

    AGENT_SEARCH_NO_ORACLE      the case file is unreadable, holds no case, none for <fn>, or
                                none for <fn> whose arguments fit its parameters
    AGENT_SEARCH_SEED_INVALID   the seed is refused, incomplete, a text draft or not made from
                                a frame; <fn> is not one of its functions or cannot run; or the
                                attempt used its 2 searches

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
    > tests: no TestCase targets diff in the seed; only the public cases ran
    > neighbors: 6 generated, 6 kernel-valid, 0 refused, 6 evaluated
    >  1. #1 opcode swap at entry.r: add -> sub (size 1): public 2/2
    >     {"af1":1,"edit":[{"fn":"diff","replace_op":"entry.r","with":["sub","a","b"]}]}
    > verified: kernel-valid and evaluated against the stated public cases, not proof of correctness for all inputs
    > next: sley-agent try --on c1 '{"af1":1,"edit":[{"fn":"diff","replace_op":"entry.r","with":["sub","a","b"]}]}' --public cases.json
