//! Make previously unstated constants explicit using the validated source
//! graph as side information. This asserts preservation, not historical authorship.

use serde_json::{Value, json};
use sley_mutate::value::EntityBodyValue;

use crate::error::Result;
use crate::names::{NameMap, Names};
use crate::residual::frontier::Budget;
use crate::workspace::{Head, Program};

use super::super::authoring::{self, Retained};

pub(crate) fn complete(
    head: &Head,
    source: &Program,
    composed: &Program,
    map: &NameMap,
    retained: &mut Retained,
    budget: &mut Budget,
) -> Result<Value> {
    let mut checked = super::inspect(head, composed, map, &retained.frame, budget)?;
    if checked.unrepresented.is_empty() {
        return Ok(checked.report);
    }
    let names = Names::build(composed, map);
    let types = crate::values::ProgramTypes {
        program: composed,
        names: &names,
    };
    let mut declared = std::collections::BTreeSet::new();
    while !checked.unrepresented.is_empty() {
        let mut declarations = Vec::new();
        for id in checked.unrepresented {
            budget.charge(1)?;
            if !declared.insert(id) {
                return Err(super::preserve(
                    "inherited constant declaration did not resolve coverage",
                ));
            }
            let object = source.object(&id).ok_or_else(|| {
                super::preserve("unrepresented constant has no validated source object")
            })?;
            if composed
                .object(&id)
                .is_none_or(|next| next.stored_bytes() != object.stored_bytes())
            {
                return Err(super::preserve(
                    "inherited constant changed during composition",
                ));
            }
            let EntityBodyValue::Constant(constant) = &object.record().body else {
                return Err(super::preserve("inherited declaration is not a constant"));
            };
            let value = crate::values::to_json(&constant.value, &names);
            let roundtrip =
                crate::values::read(&value, &constant.value.value_type, &types, "/consts")
                    .map_err(|error| {
                        super::preserve(&format!("cannot retain constant: {}", error.detail()))
                    })?;
            if roundtrip != constant.value {
                return Err(super::preserve(
                    "inherited constant cannot be represented exactly in AF1",
                ));
            }
            let declaration = json!({"name":names.name(&id),
            "type":crate::types::render(&constant.value.value_type, &names),"value":value});
            let origin = json!({"class":"INHERITED", "source":"validated_source_graph",
            "entity":crate::hex::encode(id.as_bytes()),
            "object":crate::hex::encode(object.object_id().as_bytes()),
            "historical_authorship":"not_asserted"});
            declarations.push((declaration, origin));
        }
        authoring::inherit_constants(
            retained,
            head.program(),
            &Names::build(head.program(), map),
            declarations,
            budget,
        )?;
        // Making one equal constant explicit can redirect the compiler's inline
        // alias choice. Account for any newly unrepresented identity as well.
        checked = super::inspect(head, composed, map, &retained.frame, budget)?;
    }
    let mut report = checked.report;
    report["inherited_constant_declarations"] = json!(retained.inherited_constants.len());
    Ok(report)
}
