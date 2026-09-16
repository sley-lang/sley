//! Native evidence bundle (`SLEYNBU1`) from `NATIVE_TEST_ADMISSION_V1.md` §5.
//!
//! The bundle carries the complete small test evidence inline: exact plan,
//! approval, and deterministic test-report envelopes, per-test execution
//! and measurement envelopes keyed by test entity, and the supervisor
//! configurations the measurements reference. Parsing verifies every
//! envelope's magic and digest through its owning codec, then checks the
//! closed cross-bindings: approval and report cover the plan, report covers
//! exactly the plan selection, execution/measurement entity sets equal the
//! selection, measurement objects and plan agree per entity, attestation
//! bindings join measurements by entity, and the configuration set equals
//! exactly the referenced configurations. Empty selections travel as an
//! explicit empty triple with empty lists, never omitted fields. The bundle
//! record may approach the 48 MiB aggregate evidence ceiling; each embedded
//! item keeps its own bound and execution items keep 256 KiB.

use sley_id::{EntityId, NativeEvidenceBundleId};
use sley_scb1::{ScbError, ScbErrorCode, ScbValueCursor, encode_record, encode_uvar};

use crate::approval::NativeTestApprovalV1;
use crate::codec::{
    MAX_BUNDLE_RECORD_BYTES, MAX_BUNDLE_STORED_BYTES, MAX_SELECTED_ENTRIES, RECORD_VERSION,
    check_sorted_unique, decode_envelope_bounded, decode_fields, expect_tags,
    preimage_bytes_bounded, read_id,
};
use crate::measurement::MeasuredTestAttestationV1;
use crate::plan::NativeTestPlanV1;
use crate::report::{NativeExecutionReportV1, NativeTestReportV1};
use crate::supervisor::SupervisorConfigV1;

/// `SLEYNBU1` envelope magic for [`NativeEvidenceBundleV1`].
pub const BUNDLE_MAGIC: [u8; 8] = *b"SLEYNBU1";
/// Maximum embedded item bytes: the 16 MiB SCB1 Bytes bound.
pub const MAX_BUNDLE_EMBEDDED_BYTES: usize = 16_777_216;
/// Maximum execution-report item bytes: the 256 KiB report bound.
pub const MAX_EXECUTION_ITEM_BYTES: usize = 262_144;

/// One per-test embedded envelope keyed by test entity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestEmbedded {
    /// Selected test entity this envelope belongs to.
    pub test_entity: EntityId,
    /// Exact stored envelope bytes with digest trailer.
    pub stored: Vec<u8>,
}

impl TestEmbedded {
    fn record(&self) -> Result<Vec<u8>, ScbError> {
        if self.stored.len() > MAX_BUNDLE_EMBEDDED_BYTES {
            return Err(ScbError::new(ScbErrorCode::LengthOverflow));
        }
        encode_record(&[
            (1, self.test_entity.as_bytes().to_vec()),
            (2, self.stored.clone()),
        ])
    }

    fn parse(value: &[u8], max_stored: usize) -> Result<Self, ScbError> {
        let fields = decode_fields(value)?;
        expect_tags(&fields, &[1, 2])?;
        let item = Self {
            test_entity: EntityId::from_bytes(read_id(&fields[0].1)?),
            stored: fields[1].1.clone(),
        };
        if item.stored.len() > max_stored || item.stored.len() > MAX_BUNDLE_EMBEDDED_BYTES {
            return Err(ScbError::new(ScbErrorCode::LengthOverflow));
        }
        Ok(item)
    }
}

/// Caller-supplied bundle facts; constructing these proves no coverage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeEvidenceBundleParts {
    /// Exact plan envelope bytes.
    pub plan_stored: Vec<u8>,
    /// Exact approval envelope bytes.
    pub approval_stored: Vec<u8>,
    /// Exact deterministic test-report envelope bytes.
    pub test_report_stored: Vec<u8>,
    /// Execution envelopes keyed by test entity, sorted unique.
    pub executions: Vec<TestEmbedded>,
    /// Measurement envelopes keyed by test entity, sorted unique.
    pub measurements: Vec<TestEmbedded>,
    /// Referenced supervisor-configuration envelopes, config-ID sorted unique.
    pub supervisor_configs: Vec<Vec<u8>>,
}

/// Immutable canonical evidence bundle; callers cannot forge one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeEvidenceBundleV1 {
    parts: NativeEvidenceBundleParts,
    record: Vec<u8>,
    stored: Vec<u8>,
    id: NativeEvidenceBundleId,
}

impl NativeEvidenceBundleV1 {
    /// Builds a validated bundle from caller-supplied envelopes.
    ///
    /// Every envelope parses through its owning codec and the closed
    /// cross-bindings must hold; a surplus, missing, or mismatched item
    /// refuses before any identity derives.
    ///
    /// # Errors
    /// Returns the stable refusal for shape, bound, digest, order, or
    /// binding violations.
    pub fn build(parts: NativeEvidenceBundleParts) -> Result<Self, ScbError> {
        let record = record_parts(&parts)?;
        // Verification runs over the encoded record; the input parts produced
        // those exact bytes, so storing them preserves the checked invariant
        // without re-decoding.
        parse_and_verify(&record)?;
        let preimage = preimage_bytes_bounded(BUNDLE_MAGIC, &record, MAX_BUNDLE_RECORD_BYTES)?;
        let id = NativeEvidenceBundleId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(id.as_bytes());
        Ok(Self {
            parts,
            record,
            stored,
            id,
        })
    }

    /// Parses and strictly validates stored bundle bytes, including every
    /// embedded envelope and cross-binding.
    ///
    /// # Errors
    /// Returns the stable refusal for shape, bound, digest, order, or
    /// binding violations.
    pub fn parse(stored: &[u8]) -> Result<Self, ScbError> {
        let (record, trailer) =
            decode_envelope_bounded(stored, BUNDLE_MAGIC, MAX_BUNDLE_STORED_BYTES)?;
        if record.len() > MAX_BUNDLE_RECORD_BYTES {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        let parts = parse_and_verify(&record)?;
        let preimage = preimage_bytes_bounded(BUNDLE_MAGIC, &record, MAX_BUNDLE_RECORD_BYTES)?;
        let id = NativeEvidenceBundleId::derive(&preimage);
        if id.as_bytes() != &trailer {
            return Err(ScbError::new(ScbErrorCode::DigestMismatch));
        }
        Ok(Self {
            parts,
            record,
            stored: stored.to_vec(),
            id,
        })
    }

    /// Canonical SCB1 record without envelope or trailer.
    #[must_use]
    pub fn record_bytes(&self) -> &[u8] {
        &self.record
    }

    /// Complete stored envelope with digest trailer.
    #[must_use]
    pub fn stored_bytes(&self) -> &[u8] {
        &self.stored
    }

    /// Domain-separated bundle identity.
    #[must_use]
    pub const fn id(&self) -> NativeEvidenceBundleId {
        self.id
    }

    /// Execution envelopes in test-ID order.
    #[must_use]
    pub fn executions(&self) -> &[TestEmbedded] {
        &self.parts.executions
    }

    /// Measurement envelopes in test-ID order.
    #[must_use]
    pub fn measurements(&self) -> &[TestEmbedded] {
        &self.parts.measurements
    }

    /// Supervisor-configuration envelopes in config-ID order.
    #[must_use]
    pub fn supervisor_configs(&self) -> &[Vec<u8>] {
        &self.parts.supervisor_configs
    }
}

/// Decodes the bundle record shape, verifies every embedded envelope, and
/// returns the canonical parts with list order normalized by construction.
fn parse_and_verify(record: &[u8]) -> Result<NativeEvidenceBundleParts, ScbError> {
    let fields = decode_fields(record)?;
    expect_tags(&fields, &[1, 2, 3, 4, 5, 6, 7])?;
    for index in [1, 2, 3] {
        if fields[index].1.len() > MAX_BUNDLE_EMBEDDED_BYTES {
            return Err(ScbError::new(ScbErrorCode::LengthOverflow));
        }
    }
    let plan = NativeTestPlanV1::parse(&fields[1].1)?;
    let approval = NativeTestApprovalV1::parse(&fields[2].1)?;
    let report = NativeTestReportV1::parse(&fields[3].1)?;
    if approval.plan_id() != plan.plan_id() || approval.test_report_id() != report.report_id() {
        return Err(ScbError::new(ScbErrorCode::ContractUnknown));
    }
    report.verify_plan_coverage(&plan)?;

    let selected: Vec<([u8; 32], [u8; 32])> = plan
        .selected()
        .iter()
        .map(|entry| (*entry.test_entity.as_bytes(), *entry.test_object.as_bytes()))
        .collect();

    let executions = parse_test_list(&fields[4].1, MAX_EXECUTION_ITEM_BYTES)?;
    let measurements = parse_test_list(&fields[5].1, MAX_BUNDLE_EMBEDDED_BYTES)?;
    let execution_keys: Vec<[u8; 32]> = executions
        .iter()
        .map(|item| *item.test_entity.as_bytes())
        .collect();
    let measurement_keys: Vec<[u8; 32]> = measurements
        .iter()
        .map(|item| *item.test_entity.as_bytes())
        .collect();
    let selected_keys: Vec<[u8; 32]> = selected.iter().map(|(entity, _)| *entity).collect();
    if execution_keys != selected_keys || measurement_keys != selected_keys {
        return Err(ScbError::new(ScbErrorCode::ContractUnknown));
    }

    let mut referenced_configs: Vec<[u8; 32]> = Vec::new();
    for item in &measurements {
        let measurement = MeasuredTestAttestationV1::parse(&item.stored)?;
        if measurement.plan_id() != plan.plan_id() {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        let expected_object = selected
            .iter()
            .find(|(entity, _)| entity == item.test_entity.as_bytes())
            .map(|(_, object)| *object)
            .ok_or_else(|| ScbError::new(ScbErrorCode::ContractUnknown))?;
        if measurement.test_object().as_bytes() != &expected_object {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        let binding = approval
            .attestations()
            .iter()
            .find(|binding| binding.test_entity == item.test_entity)
            .ok_or_else(|| ScbError::new(ScbErrorCode::ContractUnknown))?;
        if binding.attestation_id != measurement.id() {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        referenced_configs.push(measurement.supervisor_config_id());
    }
    for item in &executions {
        let execution = NativeExecutionReportV1::parse(&item.stored)?;
        if execution.plan_id() != plan.plan_id() {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        let expected_object = selected
            .iter()
            .find(|(entity, _)| entity == item.test_entity.as_bytes())
            .map(|(_, object)| *object)
            .ok_or_else(|| ScbError::new(ScbErrorCode::ContractUnknown))?;
        if execution.test_object().as_bytes() != &expected_object {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
    }

    let configs = parse_config_list(&fields[6].1)?;
    let mut config_ids: Vec<[u8; 32]> = Vec::new();
    for stored in &configs {
        config_ids.push(*SupervisorConfigV1::parse(stored)?.id().as_bytes());
    }
    referenced_configs.sort_unstable();
    referenced_configs.dedup();
    if config_ids != referenced_configs {
        return Err(ScbError::new(ScbErrorCode::ContractUnknown));
    }

    Ok(NativeEvidenceBundleParts {
        plan_stored: fields[1].1.clone(),
        approval_stored: fields[2].1.clone(),
        test_report_stored: fields[3].1.clone(),
        executions,
        measurements,
        supervisor_configs: configs,
    })
}

fn parse_test_list(value: &[u8], max_stored: usize) -> Result<Vec<TestEmbedded>, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let count = cursor.read_list_count()?;
    if count > MAX_SELECTED_ENTRIES {
        return Err(ScbError::new(ScbErrorCode::ResourceLimit));
    }
    let mut items = Vec::new();
    for _ in 0..count {
        items.push(TestEmbedded::parse(cursor.read_bytes()?, max_stored)?);
    }
    cursor.check_finished()?;
    let keys: Vec<[u8; 32]> = items
        .iter()
        .map(|item| *item.test_entity.as_bytes())
        .collect();
    check_sorted_unique(&keys)?;
    Ok(items)
}

fn parse_config_list(value: &[u8]) -> Result<Vec<Vec<u8>>, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let count = cursor.read_list_count()?;
    if count > MAX_SELECTED_ENTRIES {
        return Err(ScbError::new(ScbErrorCode::ResourceLimit));
    }
    let mut configs = Vec::new();
    for _ in 0..count {
        let stored = cursor.read_bytes()?.to_vec();
        if stored.len() > MAX_BUNDLE_EMBEDDED_BYTES {
            return Err(ScbError::new(ScbErrorCode::LengthOverflow));
        }
        configs.push(stored);
    }
    cursor.check_finished()?;
    Ok(configs)
}

fn record_parts(parts: &NativeEvidenceBundleParts) -> Result<Vec<u8>, ScbError> {
    if parts.plan_stored.len() > MAX_BUNDLE_EMBEDDED_BYTES
        || parts.approval_stored.len() > MAX_BUNDLE_EMBEDDED_BYTES
        || parts.test_report_stored.len() > MAX_BUNDLE_EMBEDDED_BYTES
    {
        return Err(ScbError::new(ScbErrorCode::LengthOverflow));
    }
    let mut executions = Vec::new();
    for item in &parts.executions {
        executions.push(item.record()?);
    }
    let mut measurements = Vec::new();
    for item in &parts.measurements {
        measurements.push(item.record()?);
    }
    encode_record(&[
        (1, encode_uvar(RECORD_VERSION)),
        (2, parts.plan_stored.clone()),
        (3, parts.approval_stored.clone()),
        (4, parts.test_report_stored.clone()),
        (5, sley_scb1::encode_list(&executions)?),
        (6, sley_scb1::encode_list(&measurements)?),
        (7, sley_scb1::encode_list(&parts.supervisor_configs)?),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_hex(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex decodes"))
            .collect()
    }

    fn golden(field: &str) -> Vec<u8> {
        let text = include_str!(
            "/home/gfarch/Work/checkpoints/sley2-finish-20260915/native-final-golden.json"
        );
        let marker = format!("\"{field}\": \"");
        let start = text.find(&marker).expect("golden field present") + marker.len();
        let end = text[start..].find('"').expect("golden field ends") + start;
        decode_hex(&text[start..end])
    }

    fn golden_parts() -> NativeEvidenceBundleParts {
        NativeEvidenceBundleParts {
            plan_stored: golden("plan_stored"),
            approval_stored: golden("approval_stored"),
            test_report_stored: golden("report_stored"),
            executions: Vec::new(),
            measurements: Vec::new(),
            supervisor_configs: Vec::new(),
        }
    }

    #[test]
    fn golden_bundle_parses_with_exact_id_and_bindings() {
        let stored = golden("bundle_stored");
        let parsed = NativeEvidenceBundleV1::parse(&stored).expect("golden parses");
        assert_eq!(parsed.id().as_bytes().to_vec(), golden("bundle_id"));
        assert_eq!(parsed.record_bytes(), golden("bundle_record").as_slice());
        assert!(parsed.executions().is_empty());
        assert!(parsed.measurements().is_empty());
        assert!(parsed.supervisor_configs().is_empty());
        let rebuilt = NativeEvidenceBundleV1::build(golden_parts()).expect("parts build");
        assert_eq!(rebuilt.stored_bytes(), stored.as_slice());
    }

    #[test]
    fn bundle_binding_refusals_keep_stable_codes() {
        // Approval golden from N1c binds a different plan/report: must refuse.
        let foreign = {
            let text = include_str!(
                "/home/gfarch/Work/checkpoints/sley2-finish-20260915/native-approval-golden.json"
            );
            let marker = "\"approval_accepted_stored\": \"";
            let start = text.find(marker).expect("field present") + marker.len();
            let end = text[start..].find('"').expect("field ends") + start;
            decode_hex(&text[start..end])
        };
        let mut swapped_approval = golden_parts();
        swapped_approval.approval_stored = foreign;
        assert_eq!(
            NativeEvidenceBundleV1::build(swapped_approval)
                .expect_err("foreign approval")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let stored = golden("bundle_stored");
        assert_eq!(
            NativeEvidenceBundleV1::parse(&stored[..stored.len() - 1])
                .expect_err("truncated")
                .code(),
            ScbErrorCode::LengthOverflow
        );
        let mut tampered = stored.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert_eq!(
            NativeEvidenceBundleV1::parse(&tampered)
                .expect_err("digest")
                .code(),
            ScbErrorCode::DigestMismatch
        );
    }
}
