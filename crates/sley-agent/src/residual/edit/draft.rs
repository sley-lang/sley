//! Compose a literal lens over a validated draft candidate without recreating
//! its graph. CLI receipt binding and revision publication are separate steps.

use std::collections::{BTreeMap, BTreeSet};

pub mod authoring;
pub(crate) mod receipt;

use serde_json::{Value, json};
use sley_mutate::value::EntityBodyValue;
use sley_mutate::{ImportedCandidate, MutationClass, MutationPayload};
use sley_ssmc::Immediate;

use super::{Contract, expand_graph, preserve, shape};
use crate::candidate::{self, Authority, PlannedOp};
use crate::error::Result;
use crate::frame::{self, Compiled};
use crate::names::{NameMap, Names};
use crate::residual::{BaseRef, Request, fragments::Expansion, frontier::Budget};
use crate::workspace::{Head, Program};

/// A verified composition over an immutable source candidate. This value does
/// not authorize publication or establish that a draft receipt still matches.
pub struct Prepared {
    /// Validated source graph used by the preservation contract.
    pub source: Program,
    /// Validator-owned result for the composed candidate.
    pub output: sley_policy::CandidateValidationOutput,
    /// Repaired source authoring and its expansion, when loaded through the
    /// bound receipt wrapper. The candidate-only API has no source frame.
    pub authoring: Option<authoring::Retained>,
    /// Sparse ordinary AF1 edit and provenance over the source graph.
    pub expansion: Expansion,
    /// Literal lens checked against the actual source and composed graph.
    pub contract: Contract,
    /// Complete mutation list against the original accepted head. The bound
    /// source loader adds inherited function/test IDs and authoring artifacts;
    /// the candidate-only API's auxiliary metadata describes the sparse delta.
    pub compiled: Compiled,
    /// Candidate retaining the parent's nonce and original create order.
    pub candidate: ImportedCandidate,
    /// Actual validated proposed graph, with original non-target bytes intact.
    pub program: Program,
    /// Read-only source/candidate and preservation evidence.
    pub report: Value,
}

/// Validate the source, compile a sparse edit, compose and independently
/// validate its complete candidate. No files or accepted state are written.
///
/// The caller must obtain `parent_bytes` and `map` from the exact bound draft
/// revision and recheck that binding before publication. This function cannot
/// infer a local receipt's identity from candidate bytes.
///
/// # Errors
/// Refuses non-draft requests, invalid source candidates, nonmatching lenses,
/// invalid compositions, identity collisions, preservation failures or budget
/// exhaustion. A no-op returns the original candidate bytes unchanged.
pub fn prepare(
    head: &Head,
    parent_bytes: &[u8],
    map: &NameMap,
    request: &Request,
    budget: &mut Budget,
) -> Result<Prepared> {
    let BaseRef::Draft(reference) = &request.base else {
        return Err(shape(
            "draft composition requires an explicit draft revision",
        ));
    };
    let Some(revision) = reference.revision else {
        return Err(shape(
            "draft composition requires an explicit draft revision",
        ));
    };
    budget.checkpoint()?;
    if parent_bytes.len() > crate::residual::binding::MAX_BOUND_ARTIFACT_BYTES {
        return Err(super::limit(
            "source candidate exceeds its 16 MiB read bound",
        ));
    }
    let parent = sley_mutate::import_candidate(parent_bytes)
        .map_err(|error| preserve(&format!("invalid draft candidate bytes: {error}")))?;
    let authority = Authority::of(head)?;
    let source_output = candidate::validate(head, &authority, &parent.stored_bytes)?;
    budget.checkpoint()?;
    let source = candidate::proposed_program(head, &source_output).ok_or_else(|| {
        preserve("draft source candidate does not validate against the current head")
    })?;
    if source.objects().len() > super::MAX_SOURCE_OBJECTS {
        return Err(super::limit(
            "source graph exceeds the bounded lens inventory",
        ));
    }
    let names = Names::build(&source, map);
    let edit = expand_graph(&source, &names, request, budget)?;
    budget.checkpoint()?;
    let sparse = frame::compile(
        &source,
        &names,
        &authority.ceilings,
        &edit.expansion.frame,
        candidate::fresh_nonce()?,
        &mut candidate::random32,
    )?;
    budget.checkpoint()?;
    edit.contract.check(&source, &sparse)?;
    let compiled = compose(&source, &parent, sparse, budget)?;
    let candidate = if edit.contract.no_change() {
        parent.clone()
    } else {
        candidate::assemble(
            head,
            &authority,
            parent.record.candidate_nonce,
            compiled.ops.clone(),
        )?
    };
    budget.checkpoint()?;
    let output = candidate::validate(head, &authority, &candidate.stored_bytes)?;
    budget.checkpoint()?;
    let program = candidate::proposed_program(head, &output)
        .ok_or_else(|| preserve("composed draft candidate failed ordinary kernel validation"))?;
    edit.contract.verify(&source, &program)?;
    budget.checkpoint()?;
    let report = json!({"draft_graph_edit":1,
        "source_revision":crate::draft::spell(&reference.handle,revision),
        "source_candidate_sha256":crate::draft::sha256(&parent.stored_bytes),
        "composed_candidate_sha256":crate::draft::sha256(&candidate.stored_bytes),
        "source_kernel":"valid", "composed_kernel":"valid",
        "nonce":"inherited", "original_create_order":"preserved",
        "source_graph_preservation":"verified", "receipt_binding":"caller_required",
        "no_change":edit.contract.no_change(), "native_admission":"not_attempted"});
    Ok(Prepared {
        source,
        output,
        authoring: None,
        expansion: edit.expansion,
        contract: edit.contract,
        compiled,
        candidate,
        program,
        report,
    })
}

fn compose(
    source: &Program,
    parent: &ImportedCandidate,
    mut sparse: Compiled,
    budget: &mut Budget,
) -> Result<Compiled> {
    budget.charge(parent.record.operations.len())?;
    let mut ops: Vec<_> = parent
        .record
        .operations
        .iter()
        .map(|operation| PlannedOp {
            kind: operation.target_kind,
            target: operation.target_entity,
            payload: operation.payload.clone(),
            field_tag: operation.field_tag,
        })
        .collect();
    let (identities, names) = relocate_constants(source, parent, &mut sparse, budget)?;
    for mut operation in sparse.ops {
        budget.checkpoint()?;
        match &mut operation.payload {
            MutationPayload::CreateEntity(EntityBodyValue::Constant(_)) => ops.push(operation),
            MutationPayload::ReplaceEntityVersion(EntityBodyValue::Operation(value)) => {
                if let Immediate::Entity(id) = &mut value.immediate
                    && let Some(next) = identities.get(id)
                {
                    *id = *next;
                }
                budget.charge(ops.len())?;
                let matches: Vec<_> = ops
                    .iter()
                    .enumerate()
                    .filter(|(_, old)| old.target == operation.target)
                    .map(|(index, _)| index)
                    .collect();
                match matches.as_slice() {
                    [] => ops.push(operation),
                    [index] => match &mut ops[*index].payload {
                        MutationPayload::CreateEntity(EntityBodyValue::Operation(old))
                        | MutationPayload::ReplaceEntityVersion(EntityBodyValue::Operation(old)) => {
                            *old = value.clone();
                        }
                        _ => {
                            return Err(preserve(
                                "source target was not a whole operation mutation",
                            ));
                        }
                    },
                    _ => return Err(preserve("source repeats mutation of a literal target")),
                }
            }
            _ => return Err(preserve("sparse compiler escaped the literal lens")),
        }
    }
    let created = ops
        .iter()
        .filter(|op| op.payload.class() == MutationClass::CreateEntity)
        .count();
    let deleted = ops
        .iter()
        .filter(|op| op.payload.class() == MutationClass::DeleteEntityBinding)
        .count();
    let replaced = ops
        .iter()
        .filter(|op| op.payload.class() == MutationClass::ReplaceEntityVersion)
        .count();
    Ok(Compiled {
        ops,
        names,
        created,
        replaced,
        deleted,
        functions: sparse.functions,
        tests: sparse.tests,
        notes: sparse.notes,
        artifacts: sparse.artifacts,
        stats: sparse.stats,
    })
}

fn relocate_constants(
    source: &Program,
    parent: &ImportedCandidate,
    sparse: &mut Compiled,
    budget: &mut Budget,
) -> Result<(BTreeMap<sley_id::EntityId, sley_id::EntityId>, NameMap)> {
    let mut ordinal = parent
        .record
        .operations
        .iter()
        .filter(|operation| matches!(operation.payload, MutationPayload::CreateEntity(_)))
        .count() as u64;
    let mut identities = BTreeMap::new();
    let mut allocated = BTreeSet::new();
    let mut names = NameMap::default();
    // Old create ordinals are immutable. Only the lens's new integer constants
    // receive ordinals after them. Fresh sparse-compiler IDs are never published.
    for operation in &mut sparse.ops {
        budget.checkpoint()?;
        if matches!(
            operation.payload,
            MutationPayload::CreateEntity(EntityBodyValue::Constant(_))
        ) {
            let next = candidate::created_id(
                source,
                parent.record.candidate_nonce,
                operation.kind,
                ordinal,
            );
            ordinal += 1;
            if source.contains(&next) || !allocated.insert(next) {
                return Err(preserve(
                    "appended constant identity collides with the source graph",
                ));
            }
            if let Some(name) = sparse.names.get(operation.target.as_bytes()) {
                names.insert(*next.as_bytes(), name);
            }
            identities.insert(operation.target, next);
            operation.target = next;
        }
    }
    Ok((identities, names))
}
