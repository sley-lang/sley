//! Operand ownership in the authored fragment region. Generated block names
//! deliberately avoid every authored reference, so qualified block operands
//! cannot name another region. Opcode immediates do not pass through this path.

use super::super::BindingKind;
use super::{Check, Known, Result, conflict, json};

impl Check<'_, '_> {
    pub(super) fn binding(&mut self, text: &str, at: &str) -> Result<Option<Known>> {
        let (name, index) = if let Some((name, index)) = text.split_once('#') {
            let index = index
                .parse::<u32>()
                .map_err(|_| conflict(at, "invalid local result index"))?;
            (name, index)
        } else {
            (text, 0)
        };
        if !crate::names::is_identifier(name) {
            return Err(conflict(
                at,
                "value operand must name a local fragment binding; generated blocks are private, and qualified/special references require explicit AF1-X",
            ));
        }
        if let Some(binding) = self
            .scope
            .unresolved
            .and_then(|bindings| bindings.get(name))
        {
            if index != 0 && binding.kind == Some(BindingKind::Operation) {
                return Err(conflict(
                    at,
                    &format!(
                        "binding `{text}` selects result {index}, but the authored operation at {} has only result 0",
                        binding.source
                    ),
                ));
            }
            self.deferred.insert(at.into());
            self.value_bindings.insert(json!({"at":at,"binding":text,
                "scope":"current_region","definition":binding.source,
                "type":"deferred","rewritten":false}));
            return Ok(None);
        }
        let value = self
            .scope
            .inputs
            .get(name)
            .cloned()
            .ok_or_else(|| conflict(at, "value is not a binding in this fragment interface"))?;
        if index != 0 && value.kind == BindingKind::Operation {
            // These are newly authored region operations. frame::Builder emits
            // exactly one result type for each such operation; an indexed read
            // cannot select a tuple member or another result. Checked operations
            // instead bind continuation parameters and are handled below.
            return Err(conflict(
                at,
                &format!(
                    "binding `{text}` selects result {index}, but the authored operation at {} has only result 0",
                    value.source
                ),
            ));
        }
        // frame::resolve_value changes an OperationResult index, but retains
        // a Parameter reference unchanged for every parsed u32 suffix. Its
        // declared type (including an unwrapped continuation parameter's)
        // therefore remains authoritative even when AF1-X's
        // local inference drops the optional type hint for a nonzero suffix.
        self.value_bindings.insert(json!({"at":at,"binding":text,
            "scope":"current_region","definition":value.source,
            "type":self.cx.render(&value.ty),"rewritten":false,
            "binding_kind":match value.kind {
                BindingKind::Parameter => "parameter",
                BindingKind::Operation => "operation",
                BindingKind::Continuation => "continuation_parameter",
            },
            "result_index":index}));
        Ok(Some(value.known()))
    }
}
