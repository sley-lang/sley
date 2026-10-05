//! Optional root-bound, function-scoped proposals. These are advisory
//! authoring constraints; the kernel still validates the ordinary candidate.

use std::collections::BTreeSet;

use serde_json::Value;
use sley_mutate::value::EntityBodyValue;

use super::{Head, Names, Program, owner_function};
use crate::error::{AgentError, AgentErrorCode, Result, usage};

pub(super) struct Guard {
    root: String,
    functions: Option<BTreeSet<String>>,
}

fn scope(detail: impl Into<String>) -> AgentError {
    AgentError::new(AgentErrorCode::ProposalScope, detail)
}

impl Guard {
    pub(super) fn parse(words: &super::Words, head: &Head) -> Result<Option<Self>> {
        let Some(root) = words.value("--base-root") else {
            return Ok(None);
        };
        let functions = words.value("--functions").or(words.value("--body"));
        if crate::hex::decode32(root).is_none() {
            return Err(usage(
                "--base-root needs the full 64 lowercase hex root from --json view",
            ));
        }
        let functions = functions
            .map(|value| {
                let mut names = BTreeSet::new();
                for name in value.split(',') {
                    if !crate::names::is_identifier(name) || !names.insert(name.to_owned()) {
                        return Err(scope(
                            "--functions needs unique, exact function names separated by commas",
                        ));
                    }
                }
                Ok(names)
            })
            .transpose()?;
        let guard = Self {
            root: root.to_owned(),
            functions,
        };
        guard.check_head(head)?;
        Ok(Some(guard))
    }

    pub(super) fn check_head(&self, head: &Head) -> Result<()> {
        if self.root != crate::hex::encode(head.state_root().root.as_bytes()) {
            return Err(AgentError::new(
                AgentErrorCode::ProposalStale,
                "accepted root changed; read the current context and author a new proposal",
            ));
        }
        Ok(())
    }

    pub(super) fn check_frame(&self, frame: &Value) -> Result<()> {
        let Some(allowed) = &self.functions else {
            return Ok(());
        };
        let object = frame
            .as_object()
            .ok_or_else(|| scope("scoped proposals need a function frame"))?;
        if object
            .keys()
            .any(|key| !["af1", "afx", "fns"].contains(&key.as_str()))
        {
            return Err(scope("scoped proposals may only contain af1, afx and fns"));
        }
        let functions = frame["fns"]
            .as_array()
            .filter(|fns| !fns.is_empty())
            .ok_or_else(|| scope("scoped proposals need a nonempty fns array"))?;
        let mut seen = BTreeSet::new();
        for function in functions {
            let name = function["fn"]
                .as_str()
                .ok_or_else(|| scope("each scoped function needs its exact name"))?;
            if !allowed.contains(name) || !seen.insert(name) {
                return Err(scope(format!(
                    "function `{name}` is outside the scope or repeated"
                )));
            }
        }
        Ok(())
    }

    pub(super) fn verify(
        &self,
        before: &Program,
        names: &Names,
        after: &Program,
        after_names: &Names,
    ) -> Result<()> {
        let Some(allowed) = &self.functions else {
            return Ok(());
        };
        // Compare object identities for every protected function, parameter,
        // block and operation, including ones omitted by bounded views.
        for object in before.objects() {
            let id = object.record().entity_id;
            if let Some(owner) = owner_function(before, names, &id)
                && !allowed.contains(&names.name(&owner))
                && after.object(&id).map(sley_mutate::EntityObject::object_id)
                    != Some(object.object_id())
            {
                return Err(scope(format!(
                    "protected function `{}` changed",
                    names.name(&owner)
                )));
            }
        }
        for object in after.objects() {
            let id = object.record().entity_id;
            if matches!(object.record().body, EntityBodyValue::Function(_))
                && !before.contains(&id)
                && !allowed.contains(&after_names.name(&id))
            {
                return Err(scope(format!(
                    "new function `{}` is outside the scope",
                    after_names.name(&id)
                )));
            }
        }
        Ok(())
    }
}

pub(super) fn check_options(words: &super::Words) -> Result<()> {
    if words.value("--functions").is_some() && words.value("--base-root").is_none() {
        return Err(usage(
            "--functions requires --base-root from the editing context",
        ));
    }
    if words.value("--base-root").is_some() && words.value("--on").is_some() {
        return Err(usage(
            "--base-root binds a fresh proposal; --on is not supported with it",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::names::NameMap;

    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn exact_guard_detects_changed_and_missing_interior_entities() {
        let path = std::env::temp_dir().join(format!(
            "sley-exact-scope-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _cleanup = Cleanup(path.clone());
        std::fs::create_dir_all(&path).unwrap();
        crate::genesis::init(&path, Some([7; 32]), crate::genesis::INIT_CEILINGS).unwrap();
        let run = |args: &[&str]| {
            let mut words = vec![
                "--workspace".to_owned(),
                path.display().to_string(),
                "--json".into(),
            ];
            words.extend(args.iter().map(|word| (*word).to_owned()));
            let mut output = Vec::new();
            assert_eq!(
                crate::cli::run(&words, &mut output),
                0,
                "{}",
                String::from_utf8_lossy(&output)
            );
        };
        run(&[
            "try",
            r#"{"af1":1,"fns":[{"fn":"held","params":[["x","i64"]],"returns":"i64","blocks":[{"name":"entry","ops":[["v","const",7]],"term":["return","v"]}]}]}"#,
        ]);
        run(&["commit", "c1"]);
        let ws = crate::workspace::Workspace::locate(&path).unwrap();
        let head = ws.read_head().unwrap();
        let before = head.program();
        let names = Names::build(
            before,
            &NameMap::read(&path.join(".sley/names.json")).unwrap(),
        );
        let guard = Guard {
            root: crate::hex::encode(before.root().as_bytes()),
            functions: Some(BTreeSet::from(["other".into()])),
        };
        guard.verify(before, &names, before, &names).unwrap();
        let mut kinds = BTreeSet::new();
        for (index, object) in before.objects().iter().enumerate() {
            if owner_function(before, &names, &object.record().entity_id).is_none() {
                continue;
            }
            kinds.insert(object.record().body.kind_tag());
            let mut record = object.record().clone();
            record.label = Some("changed_metadata".into());
            let mut objects = before.objects().to_vec();
            objects[index] =
                sley_mutate::build_entity_object(object.schema_epoch_id(), &record).unwrap();
            let after = Program::new(
                before.epoch(),
                before.root(),
                before.workspace(),
                objects.clone(),
            );
            assert_eq!(
                guard
                    .verify(before, &names, &after, &names)
                    .unwrap_err()
                    .code(),
                AgentErrorCode::ProposalScope
            );
            objects.remove(index);
            let after = Program::new(before.epoch(), before.root(), before.workspace(), objects);
            assert_eq!(
                guard
                    .verify(before, &names, &after, &names)
                    .unwrap_err()
                    .code(),
                AgentErrorCode::ProposalScope
            );
        }
        assert_eq!(kinds, BTreeSet::from([5, 6, 7, 8]));
    }
}
