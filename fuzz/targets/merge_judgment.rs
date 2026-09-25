#![allow(unsafe_code)]
#![no_main]

//! S20-700 Section 18.5 merge-engine surface, judgment lane: byte-driven
//! mutation scripts over one fixed valid base root drive
//! `sley_repo::judge_merge`. Every script must judge deterministically to a
//! well-formed outcome (a merged root or a decodable conflict) or to a coded
//! failure; a panic, a divergent repeat, or an undiagnosed error is a crash.

use core::slice;

use sley_id::{EntityId, ObjectId, PolicyRootId, SchemaEpochId, StateRoot, WorkspaceId};
use sley_mutate::{
    EntityObject, EntityObjectRecord, build_entity_object,
    value::{
        ConstantBody, DependencyBindingBody, EntityBodyValue, EntityIdSet, GlobalValueBody,
        NamespaceBody, PackageBody, PolicyBindingBody, TypeDefBody, WorkspaceBody,
    },
};
use sley_repo::{MergeErrorCode, MergeOutcome, MergeSide, decode_merge_conflict, judge_merge};
use sley_ssmc::{ConstData, ConstValue, TypeDefForm, TypeExpr, Visibility};

const MAX_OPS: usize = 16;
const SLOTS: [u8; 5] = [4, 6, 16, 18, 19];

fn id(byte: u8) -> EntityId {
    EntityId::from_bytes([byte; 32])
}

fn set(bytes: &[u8]) -> EntityIdSet {
    EntityIdSet::from_unsorted(bytes.iter().map(|byte| id(*byte)).collect()).unwrap()
}

fn workspace() -> EntityBodyValue {
    EntityBodyValue::Workspace(WorkspaceBody {
        packages: set(&[3]),
        root_namespace: id(2),
        capability_requirements: set(&[]),
        contracts: set(&[]),
        tests: set(&[]),
    })
}

fn package() -> EntityBodyValue {
    EntityBodyValue::Package(PackageBody {
        workspace: id(1),
        root_namespace: id(4),
        dependencies: set(&[17]),
        exports: set(&[6]),
    })
}

fn namespace(members: &[u8]) -> EntityBodyValue {
    EntityBodyValue::Namespace(NamespaceBody {
        parent: None,
        members: set(members),
    })
}

fn typedef(visibility: Visibility) -> EntityBodyValue {
    EntityBodyValue::TypeDef(TypeDefBody {
        type_parameters: Vec::new(),
        form: TypeDefForm::Record(Vec::new()),
        invariants: set(&[]),
        visibility,
    })
}

fn policy_binding(subject: u8) -> EntityBodyValue {
    EntityBodyValue::PolicyBinding(PolicyBindingBody {
        subject: id(subject),
        requirements: set(&[]),
    })
}

fn dependency_binding() -> EntityBodyValue {
    EntityBodyValue::DependencyBinding(DependencyBindingBody {
        dependency_root: StateRoot::from_bytes([9; 32]),
        external_package: id(99),
        local_namespace: id(4),
    })
}

fn constant(value: bool) -> EntityBodyValue {
    EntityBodyValue::Constant(ConstantBody {
        value: ConstValue {
            value_type: TypeExpr::Bool,
            data: ConstData::Bool(value),
        },
    })
}

fn global(visibility: Visibility) -> EntityBodyValue {
    EntityBodyValue::GlobalValue(GlobalValueBody {
        value_type: TypeExpr::Bool,
        initializer: id(18),
        visibility,
    })
}

fn base_bodies() -> Vec<(u8, EntityBodyValue)> {
    vec![
        (1, workspace()),
        (
            2,
            EntityBodyValue::Namespace(NamespaceBody {
                parent: None,
                members: set(&[]),
            }),
        ),
        (3, package()),
        (4, namespace(&[6, 16, 18, 19])),
        (6, typedef(Visibility::Private)),
        (16, policy_binding(6)),
        (17, dependency_binding()),
        (18, constant(true)),
        (19, global(Visibility::Private)),
    ]
}

fn cycle_visibility(current: Visibility) -> Visibility {
    match current {
        Visibility::Private => Visibility::Package,
        Visibility::Package => Visibility::Workspace,
        Visibility::Workspace => Visibility::Exported,
        Visibility::Exported => Visibility::Private,
    }
}

/// Applies one scripted mutation; unknown shapes are no-ops so every byte
/// string still defines two projectable sides.
fn mutate(bodies: &mut Vec<(u8, EntityBodyValue)>, labels: &mut Vec<(u8, String)>, slot: u8, mutation: u8) {
    let position = SLOTS.iter().position(|candidate| *candidate == slot);
    let Some(index) = position else {
        return;
    };
    let (_, body) = &mut bodies[index];
    match (slot, mutation % 4) {
        // Primary field toggle.
        (4, 0) => {
            if let EntityBodyValue::Namespace(namespace) = body {
                let mut members: Vec<EntityId> = namespace.members.as_slice().to_vec();
                if members.contains(&id(19)) {
                    members.retain(|member| *member != id(19));
                } else {
                    members.push(id(19));
                }
                namespace.members =
                    EntityIdSet::from_unsorted(members).expect("member toggle keeps canonicity");
            }
        }
        (6, 0) => {
            if let EntityBodyValue::TypeDef(typedef) = body {
                typedef.visibility = cycle_visibility(typedef.visibility);
            }
        }
        (16, 0) => {
            if let EntityBodyValue::PolicyBinding(binding) = body {
                binding.subject = if binding.subject == id(6) { id(18) } else { id(6) };
            }
        }
        (18, 0) => {
            if let EntityBodyValue::Constant(constant) = body {
                let flipped = !matches!(constant.value.data, ConstData::Bool(true));
                constant.value.data = ConstData::Bool(flipped);
            }
        }
        (19, 0) => {
            if let EntityBodyValue::GlobalValue(global) = body {
                global.visibility = cycle_visibility(global.visibility);
            }
        }
        // Label set / clear (metadata-only paths, including J6 and J8).
        (_, 1) => {
            labels.retain(|(entity, _)| *entity != slot);
            labels.push((slot, "fuzz".to_owned()));
        }
        (_, 2) => {
            labels.retain(|(entity, _)| *entity != slot);
        }
        // No-op: keeps dead script bytes meaningful for determinism.
        (_, _) => {}
    }
}

fn side(
    epoch: SchemaEpochId,
    root_byte: u8,
    bodies: &[(u8, EntityBodyValue)],
    labels: &[(u8, String)],
) -> MergeSide {
    let mut objects: Vec<EntityObject> = bodies
        .iter()
        .map(|(byte, body)| {
            build_entity_object(
                epoch,
                &EntityObjectRecord {
                    entity_id: id(*byte),
                    body: body.clone(),
                    label: labels
                        .iter()
                        .find(|(entity, _)| *entity == *byte)
                        .map(|(_, label)| label.clone()),
                    semantic_fingerprint: None,
                },
            )
            .expect("scripted bodies stay encodable")
        })
        .collect();
    objects.sort_by_key(|object| object.record().entity_id);
    let entry_points = objects
        .iter()
        .filter(|object| matches!(object.record().body, EntityBodyValue::EntryPoint(_)))
        .map(|object| object.record().entity_id)
        .collect();
    MergeSide {
        transaction_id: None,
        objects,
        root: StateRoot::from_bytes([root_byte; 32]),
        workspace_id: WorkspaceId::from_bytes([1; 32]),
        schema_epoch_id: epoch,
        entry_points,
        dependency_roots: vec![StateRoot::from_bytes([9; 32])],
        contract_root: ObjectId::from_bytes([20; 32]),
        test_root: ObjectId::from_bytes([21; 32]),
        policy_root: PolicyRootId::from_bytes([22; 32]),
    }
}

#[unsafe(no_mangle)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn LLVMFuzzerTestOneInput(data: *const u8, len: usize) -> i32 {
    if len == 0 {
        return 0;
    }
    let input = unsafe { slice::from_raw_parts(data, len) };
    fuzz_one(input);
    0
}

fn fuzz_one(input: &[u8]) {
    // Gated lane byte so later lanes can share the corpus directory.
    let Some((&lane, script)) = input.split_first() else {
        return;
    };
    if lane != 0 {
        return;
    }
    let epoch = sley_state_root::conformance_epoch_id().expect("frozen epoch resolves");
    let base = base_bodies();
    let mut ours_bodies = base.clone();
    let mut theirs_bodies = base;
    let mut ours_labels = Vec::new();
    let mut theirs_labels = Vec::new();
    for pair in script.chunks(2).take(MAX_OPS) {
        let address = pair[0];
        let mutation = pair.get(1).copied().unwrap_or(0);
        let slot = SLOTS[(address as usize) % SLOTS.len()];
        if address & 0x80 == 0 {
            mutate(&mut ours_bodies, &mut ours_labels, slot, mutation);
        } else {
            mutate(&mut theirs_bodies, &mut theirs_labels, slot, mutation);
        }
    }
    let ancestor = side(epoch, 50, &base_bodies(), &[]);
    let ours = side(epoch, 51, &ours_bodies, &ours_labels);
    let theirs = side(epoch, 52, &theirs_bodies, &theirs_labels);
    let outcome = judge_merge(&ancestor, &ours, &theirs);
    match outcome {
        Ok(MergeOutcome::Merged(merged)) => {
            let again = judge_merge(&ancestor, &ours, &theirs).expect("judgment repeats");
            let MergeOutcome::Merged(repeat) = again else {
                panic!("a merged judgment must repeat as merged");
            };
            assert_eq!(
                repeat.state_root.root, merged.state_root.root,
                "a merged judgment must repeat its root"
            );
            assert_eq!(
                repeat.objects.len(),
                merged.objects.len(),
                "a merged judgment must repeat its entity set"
            );
            assert_eq!(
                repeat.metadata_overridden, merged.metadata_overridden,
                "a merged judgment must repeat its override report"
            );
        }
        Ok(MergeOutcome::Conflict(conflict)) => {
            let again = judge_merge(&ancestor, &ours, &theirs).expect("judgment repeats");
            let MergeOutcome::Conflict(repeat) = again else {
                panic!("a conflict judgment must repeat as conflict");
            };
            assert_eq!(
                repeat.stored_bytes, conflict.stored_bytes,
                "a conflict judgment must repeat its bytes"
            );
            let decoded =
                decode_merge_conflict(&conflict.stored_bytes).expect("a judged conflict decodes");
            assert_eq!(decoded, *conflict, "a judged conflict round-trips");
        }
        Err(error) => {
            assert!(
                error.code().is_some_and(|code| MergeErrorCode::ALL.contains(&code)),
                "every judgment failure carries a frozen code"
            );
        }
    }
}
