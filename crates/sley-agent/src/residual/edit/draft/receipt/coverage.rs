//! Compare replayed AF1 changes with the whole proposed graph. Created constants
//! no longer mentioned after a literal repair are reported separately; their
//! historical provenance is not proven here.

use std::collections::BTreeSet;

use serde_json::{Value, json};
use sley_id::EntityId;
use sley_mutate::{
    MutationPayload,
    value::{EntityBodyValue, EntityIdSet},
};

use crate::candidate::Authority;
use crate::error::Result;
use crate::names::{NameMap, Names};
use crate::residual::frontier::Budget;
use crate::workspace::{Head, Program};

pub(super) struct Checked {
    pub report: Value,
    pub unrepresented: BTreeSet<EntityId>,
}

pub(super) fn check(
    head: &Head,
    source: &Program,
    map: &NameMap,
    frame: &Value,
    budget: &mut Budget,
) -> Result<Checked> {
    budget.checkpoint()?;
    let expected = crate::frame::replay::changes(
        head.program(),
        source,
        map,
        &Authority::of(head)?.ceilings,
        frame,
    )
    .map_err(|error| {
        super::preserve(&format!(
            "source frame cannot replay against the accepted head: {}",
            error.detail()
        ))
    })?;
    budget.checkpoint()?;
    for _ in expected.values() {
        budget.charge(1)?;
    }
    let mut surplus = BTreeSet::new();
    for object in source.objects() {
        budget.charge(1)?;
        let id = object.record().entity_id;
        if !head.program().contains(&id)
            && !expected.contains_key(&id)
            && matches!(object.record().body, EntityBodyValue::Constant(_))
        {
            surplus.insert(id);
        }
    }
    let names = Names::build(source, map);
    let mut aliases = 0;
    for object in source.objects() {
        budget.charge(1)?;
        let actual = object.record();
        let id = actual.entity_id;
        let accepted = head
            .program()
            .object(&id)
            .map(sley_mutate::EntityObject::record);
        if actual.label.as_ref() != accepted.and_then(|r| r.label.as_ref())
            || actual.semantic_fingerprint != accepted.and_then(|r| r.semantic_fingerprint)
        {
            return Err(super::preserve(
                "source object metadata differs from frame replay",
            ));
        }
        if surplus.contains(&id) {
            continue;
        }
        let body = expected
            .get(&id)
            .map_or_else(|| head.program().body(&id), Option::as_ref);
        let Some(body) = body else {
            return Err(disagreement(&names, id));
        };
        if &actual.body == body || namespace_matches(&actual.body, body, &surplus) {
            continue;
        }
        if super::equal_constant_alias(
            source,
            id,
            &MutationPayload::ReplaceEntityVersion(body.clone()),
        ) && super::inline_constant(source, &names, frame, id, budget)?
        {
            aliases += 1;
            continue;
        }
        return Err(disagreement(&names, id));
    }
    // The source walk alone cannot detect an omitted expected entity.
    for object in head.program().objects() {
        budget.charge(1)?;
        let id = object.record().entity_id;
        if !source.contains(&id) && expected.get(&id) != Some(&None) {
            return Err(disagreement(&names, id));
        }
    }
    for (id, body) in &expected {
        budget.charge(1)?;
        if body.is_some() && !source.contains(id) {
            return Err(disagreement(&names, *id));
        }
    }
    let report = json!({"basis":"accepted_head_with_source_identities_only",
        "compared_entities":source.objects().len(), "replayed_mutations":expected.len(),
        "equal_constant_aliases":aliases,
        "unrepresented_created_constants":surplus.len(),
        "complete_candidate_correspondence":if surplus.is_empty() {
            "verified_with_inline_constant_aliases"
        } else { "not_established" },
        "publication":"not_attempted"});
    Ok(Checked {
        report,
        unrepresented: surplus,
    })
}

fn namespace_matches(
    actual: &EntityBodyValue,
    expected: &EntityBodyValue,
    surplus: &BTreeSet<EntityId>,
) -> bool {
    let (EntityBodyValue::Namespace(actual), EntityBodyValue::Namespace(expected)) =
        (actual, expected)
    else {
        return false;
    };
    let members = EntityIdSet::from_unsorted(
        actual
            .members
            .as_slice()
            .iter()
            .filter(|id| !surplus.contains(id))
            .copied()
            .collect(),
    );
    actual.parent == expected.parent && members.as_ref() == Ok(&expected.members)
}

fn disagreement(names: &Names, id: EntityId) -> crate::error::AgentError {
    super::preserve(&format!(
        "source graph entity `{}` is not described by accepted-head frame replay",
        names.name(&id)
    ))
}
