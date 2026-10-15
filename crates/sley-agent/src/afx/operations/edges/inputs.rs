//! Known terminator input types and complete case sets, without a CFG proof.

use super::{Context, Inventory};
use crate::error::Result;
use serde_json::{Value, json};
use sley_ssmc::{BuiltinCase, CaseKey, TypeDefForm, TypeExpr};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Key {
    Builtin(&'static str),
    Member(String),
    ForeignMember(String),
}

impl Key {
    pub(super) fn authored(value: &Value) -> Option<Self> {
        Some(match value.as_str()? {
            word @ ("Some" | "None" | "Ok" | "Err") => Self::Builtin(match word {
                "Some" => "Some",
                "None" => "None",
                "Ok" => "Ok",
                _ => "Err",
            }),
            word => Self::Member(word.rsplit('.').next().unwrap_or(word).into()),
        })
    }

    pub(super) fn retained(cx: &Context<'_>, key: CaseKey, ty: Option<&TypeExpr>) -> Option<Self> {
        Some(match key {
            CaseKey::Builtin(case) => Self::Builtin(match case {
                BuiltinCase::Some => "Some",
                BuiltinCase::None => "None",
                BuiltinCase::Ok => "Ok",
                BuiltinCase::Err => "Err",
            }),
            CaseKey::Member(member) => {
                // Names are only read under the current definition. A member
                // from another definition cannot acquire its identity by leaf.
                if let TypeExpr::Named(named) = ty? {
                    let leaf = cx.names.member_leaf(&named.definition, &member);
                    if cx.names.resolve_member_leaf(&named.definition, &leaf) == Some(member) {
                        return Some(Self::Member(leaf));
                    }
                }
                Self::ForeignMember(crate::hex::encode(member.as_bytes()))
            }
        })
    }

    fn label(&self) -> String {
        match self {
            Self::Builtin(name) => format!("builtin {name}"),
            Self::Member(name) => format!("member {name}"),
            Self::ForeignMember(id) => format!("member identity {id}"),
        }
    }
}

impl Inventory {
    pub(super) fn condition(&mut self, cx: &Context<'_>, ty: Option<&TypeExpr>, at: &str) {
        if let Some(ty) = ty {
            if *ty != TypeExpr::Bool {
                self.conflicts.push((
                    at.into(),
                    format!(
                        "branch condition has type {}, but requires bool",
                        cx.render(ty)
                    ),
                ));
            }
        } else {
            self.deferred.push(at.into());
        }
        self.inputs.push(json!({"at":at,"kind":"condition","actual_type":ty.as_ref().map(|ty|cx.render(ty)),"required_type":"bool","type_connection":if ty.is_some(){"checked"}else{"deferred"}}));
    }

    pub(super) fn selector(
        &mut self,
        cx: &Context<'_>,
        ty: Option<&TypeExpr>,
        keys: &[(Option<Key>, String)],
        at: &str,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        checkpoint()?;
        let Some(ty) = ty else {
            self.deferred.push(at.into());
            return Ok(());
        };
        let expected = match ty {
            TypeExpr::Option(_) => vec![Key::Builtin("Some"), Key::Builtin("None")],
            TypeExpr::Result { .. } => vec![Key::Builtin("Ok"), Key::Builtin("Err")],
            TypeExpr::Named(named) => {
                let Some(definition) = cx.trait_definition(&named.definition) else {
                    self.deferred.push(at.into());
                    return Ok(());
                };
                if !matches!(definition.form, TypeDefForm::Variant(_)) {
                    self.invalid_selector(cx, ty, at);
                    return Ok(());
                }
                let Some((_, members)) = cx.members(&named.definition) else {
                    self.deferred.push(at.into());
                    return Ok(());
                };
                let mut expected = Vec::new();
                for (leaf, _) in members {
                    checkpoint()?;
                    expected.push(Key::Member(leaf));
                }
                expected
            }
            _ => {
                self.invalid_selector(cx, ty, at);
                return Ok(());
            }
        };
        let mut expected_set = BTreeSet::new();
        let mut labels = Vec::new();
        for key in expected {
            checkpoint()?;
            labels.push(key.label());
            expected_set.insert(key);
        }
        let expected = expected_set;
        let mut actual = BTreeSet::new();
        let mut complete = true;
        for (key, pointer) in keys {
            checkpoint()?;
            let Some(key) = key else {
                self.deferred.push(pointer.clone());
                complete = false;
                continue;
            };
            if !expected.contains(key) {
                self.conflicts.push((
                    pointer.clone(),
                    format!(
                        "switch case {} is not a case of {}",
                        key.label(),
                        cx.render(ty)
                    ),
                ));
            } else if !actual.insert(key.clone()) {
                self.conflicts.push((
                    pointer.clone(),
                    format!("duplicate switch case {}", key.label()),
                ));
            }
        }
        if complete {
            let mut missing = Vec::new();
            for key in expected.difference(&actual) {
                checkpoint()?;
                missing.push(key.label());
            }
            if !missing.is_empty() {
                self.conflicts.push((
                    at.into(),
                    format!(
                        "switch on {} is missing cases: {}",
                        cx.render(ty),
                        missing.join(", ")
                    ),
                ));
            }
        } else {
            self.deferred.push(at.into());
        }
        self.inputs.push(json!({"at":at,"kind":"switch","actual_type":cx.render(ty),"case_coverage":if complete{"checked"}else{"deferred"},"expected_cases":labels }));
        checkpoint()
    }

    fn invalid_selector(&mut self, cx: &Context<'_>, ty: &TypeExpr, at: &str) {
        self.conflicts.push((
            at.into(),
            format!(
                "switch selector has type {}, but requires Option, Result or a nominal variant",
                cx.render(ty)
            ),
        ));
    }
}
