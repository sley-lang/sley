.PHONY: evidence-refresh quick lint core conformance adversarial fuzz-smoke legacy-runner-smoke sley2-runner-smoke accounting-smoke release-candidate-smoke scb1-persistent-fuzz-smoke schema-persistent-fuzz-smoke pack-persistent-fuzz-smoke semantic-checkers-persistent-fuzz-smoke query-persistent-fuzz-smoke vm-persistent-fuzz-smoke adapter-responses-persistent-fuzz-smoke mutation-candidate-persistent-fuzz-smoke candidate-result-persistent-fuzz-smoke transaction-receipt-persistent-fuzz-smoke v2 release-check check-changed

quick:
	python3 scripts/check_m0.py
	python3 scripts/check_benchmark_baseline.py
	python3 scripts/check_scb1_spec.py
	python3 scripts/check_schema_epoch_spec.py
	python3 scripts/check_object_store_spec.py
	python3 scripts/check_state_root_spec.py
	python3 scripts/check_repository_pack_spec.py
	python3 scripts/check_repository_exchange_spec.py
	python3 scripts/check_gc_spec.py
	python3 scripts/check_type_system.py
	python3 scripts/check_cfg.py
	python3 scripts/check_effect_system.py
	python3 scripts/check_contract_test_profile.py
	python3 scripts/check_fingerprint_impact_profile.py
	python3 scripts/check_complete_entity_impact_profile.py
	python3 scripts/generate_complete_entity_impact_fixtures.py --check
	python3 scripts/check_semantic_comparison_spec.py
	python3 scripts/generate_semantic_comparison_fixtures.py --check
	python3 scripts/check_merge_spec.py
	python3 scripts/generate_merge_fixtures.py --check
	python3 scripts/check_complete_root_index_snapshot_profile.py
	python3 scripts/check_root_backed_query_profile.py
	python3 scripts/check_context_capsule_profile.py
	python3 scripts/check_smp1_contract.py
	python3 scripts/check_session_handle_profile.py
	python3 scripts/generate_smp1_fixtures.py --check
	python3 scripts/check_smp1_json_bridge_contract.py
	python3 scripts/generate_smp1_json_bridge_table.py --check
	python3 scripts/generate_smp1_json_bridge_fixtures.py --check
	python3 scripts/generate_release_demo_fixtures.py --check
	python3 scripts/generate_vm_extended_fixtures.py --check
	python3 scripts/check_cli_contract.py
	python3 scripts/check_cli_rules.py
	python3 scripts/generate_context_capsule_fixtures.py --check
	python3 scripts/generate_root_backed_query_fixtures.py --check
	python3 scripts/generate_complete_root_index_snapshot_fixtures.py --check
	python3 scripts/check_merge_persistent_fuzz_slice.py
	python3 scripts/check_semantic_delta_persistent_fuzz_slice.py
	python3 scripts/check_complete_root_persistent_fuzz_slice.py
	python3 scripts/check_complete_root_snapshot_persistent_fuzz_slice.py
	python3 scripts/check_root_query_persistent_fuzz_slice.py
	python3 scripts/check_context_capsule_persistent_fuzz_slice.py
	python3 scripts/check_smp1_persistent_fuzz_slice.py
	python3 scripts/check_smp1_json_bridge_persistent_fuzz_slice.py
	python3 scripts/check_vm_lowering_profile.py
	python3 scripts/check_vm_execution_profile.py
	python3 scripts/check_vm_extended_opcode_profile.py
	python3 scripts/check_reference_adapter_profile.py
	python3 scripts/check_report_envelope_profile.py
	python3 scripts/check_index_snapshot_profile.py
	python3 scripts/check_restricted_query_profile.py
	python3 scripts/check_restricted_query_capsule_profile.py
	python3 scripts/check_mutation_schema.py
	python3 scripts/check_mutation_value_codecs.py
	python3 scripts/check_mutation_candidate_persistent_fuzz_slice.py
	python3 scripts/check_policy_root.py
	python3 scripts/check_capability_token.py
	python3 scripts/check_legacy_runner.py
	python3 scripts/check_raw_baseline_runner.py
	python3 scripts/check_sley2_trial_runner.py
	python3 scripts/check_succession_accounting.py
	python3 scripts/check_release_candidate_packaging.py
	python3 scripts/check_reproducibility_and_independent_conformance.py
	python3 scripts/check_standards_sbom_and_provenance.py
	python3 scripts/check_finding_register.py
	python3 scripts/check_decision_dossier.py
	python3 scripts/check_epoch_migration_policy.py
	python3 scripts/check_required_contract_index.py
	python3 scripts/check_clean_room_boundary.py
	python3 scripts/check_error_symbol_registration.py --check
	python3 scripts/check_external_comparison_availability.py
	python3 scripts/check_supply_chain_audit.py
	python3 scripts/check_schema_fuzz_slice.py
	python3 scripts/check_s20_700_frontier.py
	python3 scripts/check_local_completion_frontier.py
	python3 scripts/check_candidate_contract_freeze.py
	python3 scripts/check_candidate_result_contract.py
	python3 scripts/generate_candidate_result_fixtures.py --check
	python3 scripts/check_candidate_result_persistent_fuzz_slice.py
	python3 scripts/generate_transaction_receipt_fixtures.py --check
	python3 scripts/check_transaction_contract.py
	python3 scripts/check_transaction_receipt_persistent_fuzz_slice.py
	python3 scripts/check_ref_branch_contract.py
	python3 scripts/generate_repository_exchange_fixtures.py --check
	python3 scripts/check_s20_530_acceptance_anchor.py
	git diff --check
	cargo check --workspace --locked
	cargo test --workspace --locked

core:
	cargo test --workspace --locked
	python3 scripts/check_m1_gate.py core

# The workspace configures `clippy::all` and `clippy::pedantic` as warnings, so
# nothing enforced them. This target denies them.
lint:
	cargo fmt --all --check
	cargo clippy --no-deps --workspace --all-targets --locked -- -D warnings

conformance:
	python3 scripts/check_scb1_spec.py
	python3 scripts/check_oracle_independence.py
	cargo test -p sley-scb1 --locked
	cargo test -p sley-schema --locked
	cargo test -p sley-store --locked
	cargo test -p sley-state-root --locked
	cargo test -p sley-repo --locked
	cargo test -p sley-txn --locked
	uv run --project oracle/scb1 --frozen python -m unittest discover -s oracle/scb1/tests -v
	uv run --project oracle/scb1 --frozen sley2-scb1-oracle check --accepted conformance/scb1/v1/accepted.json --rejected conformance/scb1/v1/rejected.json
	uv run --project oracle/scb1 --frozen sley2-scb1-oracle check-mutation-value --accepted conformance/mutation-value/v1/accepted.json --rejected conformance/mutation-value/v1/rejected.json
	uv run --project oracle/scb1 --frozen sley2-scb1-oracle check-mutation-candidate --accepted conformance/mutation-candidate/v1/accepted.json --rejected conformance/mutation-candidate/v1/rejected.json
	uv run --project oracle/scb1 --frozen sley2-scb1-oracle check-candidate-result --accepted conformance/candidate-result/v1/accepted.json --rejected conformance/candidate-result/v1/rejected.json
	uv run --project oracle/scb1 --frozen sley2-scb1-oracle check-transaction-receipt --accepted conformance/transaction-receipt/v1/accepted.json --rejected conformance/transaction-receipt/v1/rejected.json
	uv run --project oracle/scb1 --frozen python scripts/check_schema_epoch_vector.py
	uv run --project oracle/scb1 --frozen python scripts/check_state_root_vector.py
	uv run --project oracle/scb1 --frozen python scripts/check_repository_pack_vector.py
	uv run --project oracle/scb1 --frozen python scripts/check_repository_exchange_vector.py
	python3 scripts/check_complete_entity_impact_vector.py
	uv run --project oracle/scb1 --frozen python scripts/check_semantic_comparison_vector.py
	uv run --project oracle/scb1 --frozen python scripts/check_complete_root_index_snapshot_vector.py
	uv run --project oracle/scb1 --frozen python scripts/check_root_backed_query_vector.py
	uv run --project oracle/scb1 --frozen python scripts/check_context_capsule_vector.py
	uv run --project oracle/scb1 --frozen python scripts/check_smp1_vector.py
	uv run --project oracle/scb1 --frozen python scripts/check_smp1_json_bridge_vector.py
	uv run --project oracle/scb1 --frozen python scripts/check_merge_vector.py
	uv run --project oracle/scb1 --frozen sley2-scb1-oracle check-vm-extended --accepted conformance/vm-extended/v1/accepted.json --rejected conformance/vm-extended/v1/rejected.json
	uv run --project oracle/scb1 --frozen python scripts/check_release_demo_vector.py

adversarial:
	cargo test -p sley-mutate mutation_value_codec_adversarial --locked
	cargo test -p sley-store --locked
	cargo test -p sley-repo --locked
	cargo test -p sley-txn --locked
	cargo test -p sley-adapter authorized_adapter_request_binding_confusion_fails_before_charge --locked
	python3 scripts/check_m1_gate.py adversarial

fuzz-smoke:
	cargo test -p sley-mutate bounded_mutation_value_codec_fuzz_smoke --locked
	cargo test -p sley-scb1 bounded_scb1_decoder_fuzz_smoke --locked
	cargo test -p sley-schema bounded_schema_bootstrap_import_fuzz_smoke --locked
	cargo test -p sley-store randomized_invalid_records_never_promote --locked
	cargo test -p sley-repo bounded_pack_import_fuzz_smoke --locked
	python3 scripts/check_m1_gate.py fuzz-smoke

sley2-runner-smoke:
	cargo build -p sley-cli --locked
	python3 -m bench.sley2.runner smoke --sley target/debug/sley --evidence-dir evidence/runtime/s20-620-sley2-smoke --timeout-seconds 90
	python3 scripts/check_sley2_trial_runner.py

accounting-smoke:
	python3 -m bench.accounting.report smoke --sley2-evidence evidence/runtime/s20-620-sley2-smoke/evidence.json --output-dir evidence/runtime/s20-630-accounting-smoke
	python3 scripts/check_succession_accounting.py

# Every builder runs before every checker: the release checkers run the shared
# bench/release test suite, whose S20-710/S20-730 tests read the evidence a
# candidate build has just replaced.
# Regenerates every derived evidence document in dependency order: the T52
# inventory feeds the SBOM, the SBOM and the conformance report feed the
# provenance, and the machine summary feeds the register and the dossier. The
# summary's own register and dossier counters are synced and both documents
# rebuilt, which converges in one pass because each digests its derived entries
# rather than the summary bytes. The supply-chain generator runs again at the
# end because its T54 scan covers the documents the earlier steps rewrote. The reproducibility report is rebuilt
# only by the release smoke, because it attests a clean-tree candidate build.
evidence-refresh:
	python3 scripts/check_error_symbol_registration.py
	python3 scripts/generate_supply_chain_evidence.py
	python3 scripts/build_independent_conformance_report.py
	python3 scripts/build_standards_sbom.py
	python3 scripts/build_release_provenance.py
	python3 scripts/build_test_inventory.py
	python3 scripts/build_threat_coverage_report.py
	python3 scripts/build_anti_goal_conformance.py
	python3 scripts/build_ga_acceptance_report.py
	python3 scripts/build_finding_register.py
	python3 scripts/build_decision_dossier.py
	python3 scripts/sync_evidence_counters.py
	python3 scripts/build_finding_register.py
	python3 scripts/build_decision_dossier.py
	python3 scripts/generate_supply_chain_evidence.py

release-candidate-smoke:
	python3 scripts/build_release_candidate.py --timeout-seconds 900
	python3 scripts/build_reproducibility_report.py
	python3 scripts/build_standards_sbom.py
	python3 scripts/build_release_provenance.py
	python3 scripts/build_finding_register.py
	python3 scripts/build_decision_dossier.py
	python3 scripts/sync_evidence_counters.py
	python3 scripts/build_finding_register.py
	python3 scripts/build_decision_dossier.py
	# The T54 scan covers the evidence documents the builders above rewrote,
	# so it runs last and the tree is consistent when the smoke returns.
	python3 scripts/generate_supply_chain_evidence.py
	python3 scripts/check_release_candidate_packaging.py
	python3 scripts/check_reproducibility_and_independent_conformance.py
	python3 scripts/check_standards_sbom_and_provenance.py
	python3 scripts/check_finding_register.py
	python3 scripts/check_decision_dossier.py

legacy-runner-smoke:
	python3 -m bench.legacy.runner smoke --timeout-seconds 90 --output-limit-bytes 65536 --evidence-dir evidence/runtime/s20-600-legacy-smoke

scb1-persistent-fuzz-smoke:
	python3 scripts/check_scb1_persistent_fuzz_slice.py
	python3 scripts/run_scb1_persistent_fuzz.py

schema-persistent-fuzz-smoke:
	python3 scripts/check_schema_persistent_fuzz_slice.py
	python3 scripts/run_schema_persistent_fuzz.py

pack-persistent-fuzz-smoke:
	python3 scripts/check_pack_persistent_fuzz_slice.py
	python3 scripts/run_pack_persistent_fuzz.py

semantic-checkers-persistent-fuzz-smoke:
	python3 scripts/check_semantic_checkers_persistent_fuzz_slice.py
	python3 scripts/run_semantic_checkers_persistent_fuzz.py

query-persistent-fuzz-smoke:
	python3 scripts/check_query_persistent_fuzz_slice.py
	python3 scripts/run_query_persistent_fuzz.py

vm-persistent-fuzz-smoke:
	python3 scripts/check_vm_persistent_fuzz_slice.py
	python3 scripts/run_vm_persistent_fuzz.py

adapter-responses-persistent-fuzz-smoke:
	python3 scripts/check_adapter_responses_persistent_fuzz_slice.py
	python3 scripts/run_adapter_responses_persistent_fuzz.py

mutation-candidate-persistent-fuzz-smoke:
	python3 scripts/check_mutation_candidate_persistent_fuzz_slice.py
	python3 scripts/run_mutation_candidate_persistent_fuzz.py

candidate-result-persistent-fuzz-smoke:
	python3 scripts/check_candidate_result_persistent_fuzz_slice.py
	python3 scripts/run_candidate_result_persistent_fuzz.py

exchange-persistent-fuzz-smoke:
	python3 scripts/check_exchange_persistent_fuzz_slice.py
	python3 scripts/run_exchange_persistent_fuzz.py

complete-root-persistent-fuzz-smoke:
	python3 scripts/check_complete_root_persistent_fuzz_slice.py
	python3 scripts/run_complete_root_persistent_fuzz.py

semantic-delta-persistent-fuzz-smoke:
	python3 scripts/check_semantic_delta_persistent_fuzz_slice.py
	python3 scripts/run_semantic_delta_persistent_fuzz.py

merge-persistent-fuzz-smoke:
	python3 scripts/check_merge_persistent_fuzz_slice.py
	python3 scripts/run_merge_persistent_fuzz.py

complete-root-snapshot-persistent-fuzz-smoke:
	python3 scripts/check_complete_root_snapshot_persistent_fuzz_slice.py
	python3 scripts/run_complete_root_snapshot_persistent_fuzz.py

root-query-persistent-fuzz-smoke:
	python3 scripts/check_root_query_persistent_fuzz_slice.py
	python3 scripts/run_root_query_persistent_fuzz.py

context-capsule-persistent-fuzz-smoke:
	python3 scripts/check_context_capsule_persistent_fuzz_slice.py
	python3 scripts/run_context_capsule_persistent_fuzz.py

smp1-persistent-fuzz-smoke:
	python3 scripts/check_smp1_persistent_fuzz_slice.py
	python3 scripts/run_smp1_persistent_fuzz.py

smp1-json-bridge-persistent-fuzz-smoke:
	python3 scripts/check_smp1_json_bridge_persistent_fuzz_slice.py
	python3 scripts/run_smp1_json_bridge_persistent_fuzz.py

transaction-receipt-persistent-fuzz-smoke:
	python3 scripts/generate_transaction_receipt_fixtures.py --check
	python3 scripts/check_transaction_receipt_persistent_fuzz_slice.py
	python3 scripts/run_transaction_receipt_persistent_fuzz.py

s20-530-verify:
	python3 scripts/verify_s20_530_accepted_state.py

check-changed: quick core conformance adversarial fuzz-smoke
	@python3 scripts/check_changed.py

v2 release-check:
	@python3 scripts/gate_status.py $@
