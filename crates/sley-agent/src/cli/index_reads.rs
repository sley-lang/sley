//! Where a failing advisory test's trap came from, when it was a list or
//! vector read out of range: `try` lowers the frame's structured bodies
//! again with every trapping read checked first
//! ([`crate::structured::lower_probing`]), stages that frame in memory,
//! and runs the failing tests on it. The probed run traps at the same read,
//! with its index and length as the payload. The probed program is never
//! recorded, submitted or committed, and its fuel is never reported: the
//! test's verdict, fuel and trap code stay those of the real candidate.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sley_ssmc::{ConstData, ConstValue};
use sley_vm::ExecutionTermination;

use crate::afx::Context;
use crate::candidate::{self, Authority, StageInputs};
use crate::exec::{self, Executor, TestOutcome};
use crate::names::{NameMap, Names};
use crate::workspace::Head;

/// The VM's `unreachable` trap tag: what a missing index traps with.
const UNREACHABLE: u32 = 1;

/// One out-of-range read: where it is and what it read.
pub(super) struct IndexRead {
    /// The authored pointer of the read in the tried frame.
    pub(super) pointer: String,
    /// The index read (negative for a negative Python index).
    pub(super) index: i128,
    /// The length of the list or vector it read.
    pub(super) length: u128,
}

impl IndexRead {
    /// `index 3 is out of range for length 3`. A negative index Python
    /// would count from the end says that Sley does not.
    pub(super) fn text(&self) -> String {
        let text = format!(
            "index {} is out of range for length {}",
            self.index, self.length
        );
        if self.index < 0 && self.index.unsigned_abs() <= self.length {
            format!("{text} (a computed negative index does not count from the end)")
        } else {
            text
        }
    }

    /// The JSON form, with the text position when one is known.
    pub(super) fn json(&self, line: Option<(usize, usize)>) -> Value {
        let mut value = json!({
            "at": self.pointer,
            "index": self.index.to_string(),
            "length": self.length.to_string(),
        });
        if let Some((line, column)) = line {
            value["text"] = json!({"line": line, "column": column});
        }
        value
    }
}

/// The out-of-range read behind each failing test that trapped, by test
/// name. Empty when the frame has no structured bodies, no failing test
/// trapped, or the probed frame does not stage as a Valid candidate.
pub(super) fn find(
    head: &Head,
    authority: &Authority,
    names: &Names,
    map: &NameMap,
    frame: &Value,
    tests: &[TestOutcome],
    after: &Names,
) -> BTreeMap<String, IndexRead> {
    let trapped: BTreeSet<String> = tests
        .iter()
        .filter(|test| !test.passed() && test.actual == format!("trap({UNREACHABLE})"))
        .map(|test| after.name(&test.test))
        .collect();
    let Some(object) = frame.as_object() else {
        return BTreeMap::new();
    };
    if trapped.is_empty()
        || !crate::structured::uses_bodies(object)
        || object.contains_key("ripple")
    {
        return BTreeMap::new();
    }
    let context = Context::new(head.program(), names, object);
    let Ok((lowered, sites)) = crate::structured::lower_probing(&context, object) else {
        return BTreeMap::new();
    };
    if sites.is_empty() {
        return BTreeMap::new();
    }
    let Ok(nonce) = candidate::fresh_nonce() else {
        return BTreeMap::new();
    };
    let staged = candidate::stage(
        head,
        authority,
        names,
        &Value::Object(lowered.frame),
        StageInputs {
            nonce,
            random: &mut candidate::random32,
            now: &mut candidate::now_unix_millis,
        },
        |_| Ok(()),
    );
    let Ok((compiled, _, output)) = staged else {
        return BTreeMap::new();
    };
    let Some(program) = candidate::proposed_program(head, &output) else {
        return BTreeMap::new();
    };
    let mut probed_map = map.clone();
    probed_map.extend(&compiled.names);
    let probed_names = Names::build(&program, &probed_map);
    let Ok(mut executor) = Executor::new(&program) else {
        return BTreeMap::new();
    };
    let mut found = BTreeMap::new();
    for test in executor.tests().to_vec() {
        let name = probed_names.name(&test.entity_id);
        if !trapped.contains(&name) {
            continue;
        }
        // The probe's checks cost fuel the test's own limit does not
        // budget for; the outcome, not the count, is what is read.
        let Ok(outcome) = executor.run(&test.target, test.inputs.clone(), exec::call_limits())
        else {
            continue;
        };
        if let Some(read) = read_of(&outcome.termination, &sites) {
            found.insert(name, read);
        }
    }
    found
}

/// The read a probed trap reports: `(site, index, length)`.
fn read_of(termination: &ExecutionTermination, sites: &[String]) -> Option<IndexRead> {
    let ExecutionTermination::Trap {
        trap_tag: UNREACHABLE,
        payload: Some(payload),
    } = termination
    else {
        return None;
    };
    let ConstData::Sequence(items) = &payload.data else {
        return None;
    };
    let [site, index, length] = items.as_slice() else {
        return None;
    };
    let unsigned = |value: &ConstValue| match value.data {
        ConstData::UInt(number) => Some(number),
        _ => None,
    };
    let index = match index.data {
        ConstData::SInt(number) => number,
        ConstData::UInt(number) => i128::try_from(number).ok()?,
        _ => return None,
    };
    let pointer = sites.get(usize::try_from(unsigned(site)?).ok()?)?.clone();
    Some(IndexRead {
        pointer,
        index,
        length: unsigned(length)?,
    })
}
