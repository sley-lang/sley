//! Existing frame index grammar; parameter references do not select result slots.

use super::{Definition, Inventory};
use crate::afx::Kind;

pub(super) fn check(
    report: &mut Inventory,
    text: &str,
    at: &str,
    definition: Option<&Definition>,
) -> &'static str {
    let index = match text.split_once('#') {
        Some((_, suffix)) => {
            if let Ok(index) = suffix.parse::<u32>() {
                index
            } else {
                report.conflicts.push((at.into(), format!("bad result index spelling in `{text}`: the ordinary frame requires an unsigned 32-bit result index")));
                return "conflict";
            }
        }
        None => 0,
    };
    let Some(def) = definition else {
        return "cardinality_deferred";
    };
    if def.kind != Kind::Op {
        return if index == 0 {
            "parameter_reference_checked"
        } else {
            "parameter_suffix_ignored_by_ordinary_compiler"
        };
    }
    let Some(count) = def.results else {
        report.deferred.push(at.into());
        return "cardinality_deferred";
    };
    if usize::try_from(index).is_ok_and(|index| index < count) {
        "operation_result_index_checked"
    } else {
        report.conflicts.push((at.into(), format!("result index {index} in `{text}` is outside the {count} results of its definition at {}", def.at)));
        "conflict"
    }
}
