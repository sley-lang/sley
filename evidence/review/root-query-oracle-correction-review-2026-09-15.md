# Root-query oracle correction review

Date: 2026-09-15. Scope: the seven-file correction listed in `root-query-oracle-correction-summary.md`, source provenance of the two rejected contexts, contiguous page-walk claims and accepted-vector preservation. Independent source review and existing bounded Python checks; no source changes or new reproduction inputs.

## Verdict

**Accept the scoped correction; zero findings.** The descriptor-driven arm/root inference is replaced with actual emitted input evidence and independent reconstruction. The page property now checks the exact same-query cursor chain rather than assuming arbitrary request pages are disjoint. These changes close the historical Vulcan43f2f5b P3 arm-oracle independence residue and P4 page-union assumption within the frozen fixture scope. They do not claim a general arbitrary-input query implementation proof, new Council verdict or release acceptance.

## Provenance and semantic checks

- `crates/sley-query/src/root_query.rs:3355` and `:3398` serialize context from the same `RootQueryInput` passed into the real constructor/executor for each refusal. The helper emits that input's root, constructor-owned snapshot bytes and snapshot ID. The Rust changes are confined to the conformance emitter/helper; production query semantics are unchanged.
- `scripts/generate_root_backed_query_fixtures.py:117` copies the additional emitter field and validates its shape. It no longer manufactures an arm/root substitution from the row ID. A label is diagnostic only; the focused rename test confirms rejection semantics do not depend on it.
- `scripts/check_root_backed_query_vector.py:110` rederives direct edges from frozen semantic entities rather than trusting the expected edge list as its algorithm. It also checks equality with that frozen inventory. The shared independent Python snapshot encoder is used for exact complete and restricted snapshots; no Rust code or emitted expected-error label participates in that reconstruction.
- `validated_input_context` at `scripts/check_root_backed_query_vector.py:162` validates the emitted header/epoch/schema/context, reads the actual arm, and requires full equality with the independently rebuilt snapshot plus matching snapshot ID. This covers inventory, direct/reverse edges and digest. A merely self-consistent altered snapshot is not sufficient evidence for the frozen input. Invalid fixture evidence fails the oracle instead of becoming a manufactured query refusal.
- `answer` at `scripts/check_root_backed_query_vector.py:540` applies the validated arm before binding rules. The substituted root is checked against the common context and the independently reconstructed nine-field StateRoot commitment. For contextual refusal rows, the unmodified request must first succeed, ensuring the changed input is responsible for the claimed refusal. Missing context falls back to the honest input and therefore cannot establish these two expected rejections; legacy tamper descriptors are explicitly refused.
- `scripts/check_complete_root_index_snapshot_vector.py:68` preserves arm 2 as its default, adds an explicit bounded arm choice, and restricts arm-1 inventory kinds. Its existing arm-2 inspector is unchanged; the change does not widen that decoder's admitted completeness profile.

## Page completeness claim

`contiguous_walk_problem` at `scripts/check_root_backed_query_vector.py:584` requires equal query and limit records, both continuation-enabled, no initial cursor, the actual preceding final key, a terminal second page, exact totals and ordered concatenation equal to the full independently computed result. Root/epoch/workspace/snapshot identity is shared through the single corpus context; per-vector source contexts are not supplied.

For a nontruncated singleton first response, the second request is explicitly an empty terminal probe after the final item. Ordered equality forces it to contribute no duplicate item. This is not a claim that the first response was truncated or that an arbitrary two-response set satisfies the normative continuation rule. The checker now applies the same narrow property to namespaces, edges, roots and entry-point fixtures.

The tests independently rebuild individually valid overlapping, different-query and different-limit responses, then require the aggregate walk check to reject the combination. Thus the property is checked in addition to each page's own byte/ID reproduction.

## Preservation and validation

Independently compared current bytes with `git show HEAD`:

- `conformance/root-backed-query/v1/accepted.json` is unchanged, SHA256 `6cb756ebd96cb6d501656c6ca0f70bce57d8a4671bddd4c887286413e752ac05`; all 27 accepted vectors/IDs/records are therefore preserved.
- Rejected JSON differs only in the two contextual rows, replacing `tamper` with `input_context`. Expected code/numeric, request, limits and all other rows are unchanged. Its SHA256 is `59291ac4be72c9eb289e3b13de9df2c95b4368d5262649f7d4afd12e2cc2f691`; SHA256SUMS changes only for that file.

Independently executed with the existing frozen oracle environment:

- `python -m unittest discover -s oracle/scb1/tests -p test_root_query_oracle.py -v`: **13 passed**, exit 0.
- Root-query checker: **PASS**, 27 vectors, 10 rejected cases, 19 classes, no problems, exit 0.
- Complete-root snapshot checker: **PASS**, one accepted vector, six rejected cases, no problems, exit 0.

Inspected supplied logs separately: generator `--check` reports zero drift at 27/10; full independent Python suite reports 133 passed; Rust query suite reports 104 passed with four fixture emitters ignored, with the relevant emitter separately exercised by the generator check. These latter executions were not rerun by this reviewer. No full gate or packaged build was run.

## Reviewed source fingerprints

- `crates/sley-query/src/root_query.rs`: `103c1aa52f6026a74a098829ef206a4d881b7ba3e94b91d8367b583ae6ee49a8`
- `scripts/generate_root_backed_query_fixtures.py`: `0f4b7ca2ae4ef6a81734912870c218dd84284126fddbc708baf3425e725f742b`
- `scripts/check_root_backed_query_vector.py`: `ce6e98f2595a0e3c0bde5d861c1629f8798f4c0700e5d288a86ed67ed1684d63`
- `scripts/check_complete_root_index_snapshot_vector.py`: `22b666abed03a5aedb17b8598837fb71ca88ccd83ea4603e001e4c67a3f0c0f7`
- `oracle/scb1/tests/test_root_query_oracle.py`: `80f796ee8a73bd84581302ae75d78f8496e7bdbebc50e6229753a79ad0e9ee1f`
- `conformance/root-backed-query/v1/rejected.json`: `59291ac4be72c9eb289e3b13de9df2c95b4368d5262649f7d4afd12e2cc2f691`
