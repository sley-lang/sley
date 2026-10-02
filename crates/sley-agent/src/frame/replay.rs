//! In-memory compiler replay with identities from an independently validated
//! graph. Only identities are borrowed: values and inherited fields come from
//! the accepted base and frame. Return expected bodies, not assembly operations:
//! the replay's create order need not derive the supplied identities.

use std::collections::BTreeMap;

use sley_id::{CandidateNonce, EntityId};
use sley_mutate::{MutationPayload, value::EntityBodyValue};
use sley_policy::PolicyResourceCeilings;
use sley_ssmc::MemberId;

use crate::error::Result;
use crate::names::{NameMap, Names, Scope};
use crate::workspace::Program;

pub(super) struct Identities<'a> {
    program: &'a Program,
    names: &'a Names,
}

impl Identities<'_> {
    // Generated constant names depend on allocation order and may collide
    // across widths. Match the author's complete typed value, never a generated
    // leaf. This selects identity only; the replay body still uses that value.
    pub(super) fn constant(
        &self,
        value: &sley_ssmc::ConstValue,
        accepted: &Program,
    ) -> Result<EntityId> {
        self.program
            .objects()
            .iter()
            .find_map(|object| {
                let id = object.record().entity_id;
                match &object.record().body {
                    sley_mutate::value::EntityBodyValue::Constant(body)
                        if !accepted.contains(&id) && body.value == *value =>
                    {
                        Some(id)
                    }
                    _ => None,
                }
            })
            .ok_or_else(|| super::frame("", "replay has no matching created constant identity"))
    }

    pub(super) fn entity(&self, kind: u16, leaf: &str, scope: Scope) -> Result<EntityId> {
        let name = match scope {
            Scope::Top => leaf.to_owned(),
            Scope::Function(owner) | Scope::Block(owner) => {
                format!("{}.{leaf}", self.names.name(&owner))
            }
        };
        self.names
            .resolve(&name)
            .filter(|id| self.names.scope(id) == scope)
            .filter(|id| {
                self.program
                    .body(id)
                    .is_some_and(|body| body.kind_tag() == kind)
            })
            .ok_or_else(|| {
                super::frame("", format!("replay has no matching identity for `{name}`"))
            })
    }

    pub(super) fn member(&self, definition: EntityId, leaf: &str) -> Result<MemberId> {
        self.names
            .resolve_member_leaf(&definition, leaf)
            .ok_or_else(|| {
                super::frame(
                    "",
                    format!("replay has no matching member identity for `{leaf}`"),
                )
            })
    }
}

/// Reconstruct changed bodies and deletions from the accepted base. This result
/// has no assembly operations: source creates may have a different order.
pub(crate) fn changes(
    accepted: &Program,
    source: &Program,
    map: &NameMap,
    ceilings: &PolicyResourceCeilings,
    frame: &serde_json::Value,
) -> Result<BTreeMap<EntityId, Option<EntityBodyValue>>> {
    let accepted_names = Names::build(accepted, map);
    let source_names = Names::build(source, map);
    let compiled = super::compile_inner(
        accepted,
        &accepted_names,
        ceilings,
        frame,
        CandidateNonce::from_bytes([0x6d; 32]),
        &mut || {
            Err(super::frame(
                "",
                "replay cannot create fresh member identities",
            ))
        },
        Some(Identities {
            program: source,
            names: &source_names,
        }),
    )?;
    let mut changes = BTreeMap::new();
    for operation in compiled.ops {
        let body = match operation.payload {
            MutationPayload::CreateEntity(body) | MutationPayload::ReplaceEntityVersion(body) => {
                Some(body)
            }
            MutationPayload::DeleteEntityBinding => None,
            _ => return Err(super::frame("", "unexpected mutation in frame replay")),
        };
        if changes.insert(operation.target, body).is_some() {
            return Err(super::frame("", "duplicate target in frame replay"));
        }
    }
    Ok(changes)
}
