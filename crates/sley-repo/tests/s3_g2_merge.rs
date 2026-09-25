//! S2B-MERGE-001 (G2, sley2 arm): disjoint merge composes, overlap conflicts.
//!
//! Real machinery: `sley_repo::judge_merge` (merge judge J1-J8,
//! `crates/sley-repo/src/merge.rs`) over synthetic `MergeSide`s built from
//! owned canonical entity objects — the same construction the engine's own
//! merge tests use (no repository I/O; the merge itself never writes).
//! Pinned symbols: `MergeOutcome::Merged` / `MergeOutcome::Conflict` with the
//! canonical conflict object (`StoredMergeConflict`: `conflict_id`,
//! `stored_bytes`, `conflict.conflicts: Vec<ConflictEntry>`);
//! `ConflictReason::FieldEdit` (tag 3) for the overlapping change.
//!
//! Positive: disjoint branches (ours adds an invoice-rounding-style constant,
//! theirs edits the checksum-style typedef visibility... precisely: ours adds
//! constant 30 to the namespace, theirs flips constant 18) merge with both
//! changes preserved; combined root deterministic across repeats and
//! identical under left/right swap (commutative).
//! Negative `overlapping_change`: both branches change typedef 6 visibility
//! differently -> canonical conflict object with exactly 1 entry, never
//! silent. Arm-honest code `ORACLE_MERGE_CONFLICT` (a conflict is a
//! successful outcome, not an engine error symbol).
//!
//! Negative handshake: `s3_merge_neg_<name>` prints `S3_NEG_RESULT <CODE>`
//! then fails by design; `oracle.py` requires nonzero exit + the code line.

use sley_id::{EntityId, ObjectId, PolicyRootId, SchemaEpochId, StateRoot, WorkspaceId};
use sley_mutate::value::{
    EntityBodyValue, EntityIdSet, NamespaceBody, PackageBody, PolicyBindingBody, TypeDefBody,
    WorkspaceBody,
};
use sley_mutate::{EntityObject, EntityObjectRecord, build_entity_object};
use sley_repo::{ConflictReason, MergeOutcome, MergeSide, decode_merge_conflict, judge_merge};
use sley_ssmc::{ConstData, ConstValue, TypeDefForm, TypeExpr, Visibility};
use sley_state_root::conformance_epoch_id as state_epoch_id;

const NEG_OVERLAP_CODE: &str = "ORACLE_MERGE_CONFLICT";

fn id(byte: u8) -> EntityId {
    EntityId::from_bytes([byte; 32])
}

fn root(byte: u8) -> StateRoot {
    StateRoot::from_bytes([byte; 32])
}

fn set(ids: &[u8]) -> EntityIdSet {
    EntityIdSet::from_unsorted(ids.iter().map(|byte| id(*byte)).collect()).unwrap()
}

fn constant(value: bool) -> EntityBodyValue {
    EntityBodyValue::Constant(sley_mutate::value::ConstantBody {
        value: ConstValue {
            value_type: TypeExpr::Bool,
            data: ConstData::Bool(value),
        },
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

fn namespace(members: &[u8]) -> EntityBodyValue {
    EntityBodyValue::Namespace(NamespaceBody {
        parent: None,
        members: set(members),
    })
}

/// Base seven-entity complete root plus a constant (18) in the package
/// namespace — mirrors the engine's own merge-test scaffold.
fn base_bodies() -> Vec<(u8, EntityBodyValue)> {
    vec![
        (
            1,
            EntityBodyValue::Workspace(WorkspaceBody {
                packages: set(&[3]),
                root_namespace: id(2),
                capability_requirements: set(&[]),
                contracts: set(&[]),
                tests: set(&[]),
            }),
        ),
        (
            2,
            EntityBodyValue::Namespace(NamespaceBody {
                parent: None,
                members: set(&[]),
            }),
        ),
        (
            3,
            EntityBodyValue::Package(PackageBody {
                workspace: id(1),
                root_namespace: id(4),
                dependencies: set(&[]),
                exports: set(&[6]),
            }),
        ),
        (
            4,
            EntityBodyValue::Namespace(NamespaceBody {
                parent: None,
                members: set(&[6, 16, 18]),
            }),
        ),
        (
            6,
            EntityBodyValue::TypeDef(TypeDefBody {
                type_parameters: Vec::new(),
                form: TypeDefForm::Record(Vec::new()),
                invariants: set(&[]),
                visibility: Visibility::Private,
            }),
        ),
        (
            16,
            EntityBodyValue::PolicyBinding(PolicyBindingBody {
                subject: id(6),
                requirements: set(&[]),
            }),
        ),
        (
            18,
            EntityBodyValue::Constant(sley_mutate::value::ConstantBody {
                value: ConstValue {
                    value_type: TypeExpr::Bool,
                    data: ConstData::Bool(true),
                },
            }),
        ),
    ]
}

fn with(
    bodies: &[(u8, EntityBodyValue)],
    byte: u8,
    body: EntityBodyValue,
) -> Vec<(u8, EntityBodyValue)> {
    let mut out: Vec<(u8, EntityBodyValue)> = bodies
        .iter()
        .filter(|(existing, _)| *existing != byte)
        .cloned()
        .collect();
    out.push((byte, body));
    out
}

/// A synthetic merge side over owned objects (no repository), exactly as the
/// engine's merge tests construct it.
fn synthetic(root_byte: u8, bodies: &[(u8, EntityBodyValue)]) -> MergeSide {
    let epoch: SchemaEpochId = state_epoch_id().unwrap();
    let mut objects: Vec<EntityObject> = bodies
        .iter()
        .map(|(byte, body)| {
            build_entity_object(
                epoch,
                &EntityObjectRecord {
                    entity_id: id(*byte),
                    body: body.clone(),
                    label: None,
                    semantic_fingerprint: None,
                },
            )
            .unwrap()
        })
        .collect();
    objects.sort_by_key(|object| object.record().entity_id);
    MergeSide {
        transaction_id: None,
        objects,
        root: root(root_byte),
        workspace_id: WorkspaceId::from_bytes([1; 32]),
        schema_epoch_id: epoch,
        entry_points: Vec::new(),
        dependency_roots: Vec::new(),
        contract_root: ObjectId::from_bytes([20; 32]),
        test_root: ObjectId::from_bytes([21; 32]),
        policy_root: PolicyRootId::from_bytes([22; 32]),
    }
}

fn object_body(side_objects: &[EntityObject], byte: u8) -> EntityBodyValue {
    side_objects
        .iter()
        .find(|object| object.record().entity_id == id(byte))
        .expect("entity present in merged root")
        .record()
        .body
        .clone()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

struct MergeEvidence {
    ancestor_root: String,
    ours_root: String,
    theirs_root: String,
    merged_root: String,
    merged_objects: usize,
    conflicts: usize,
}

fn run_disjoint_merge() -> MergeEvidence {
    let base = base_bodies();
    let ancestor = synthetic(50, &base);
    // Ours (invoice-rounding-style tests): add constant 30 to the namespace.
    let mut ours_bodies = with(&base, 4, namespace(&[6, 16, 18, 30]));
    ours_bodies.push((30, constant(false)));
    let ours = synthetic(51, &ours_bodies);
    // Theirs (checksum-style impl): flip constant 18.
    let theirs = synthetic(52, &with(&base, 18, constant(false)));
    let outcome = judge_merge(&ancestor, &ours, &theirs).expect("disjoint merge judges");
    let merged = match outcome {
        MergeOutcome::Merged(merged) => *merged,
        MergeOutcome::Conflict(conflict) => {
            panic!(
                "disjoint sides must merge, got {:?}",
                conflict.conflict.conflicts
            )
        }
    };
    // Both changes preserved (J1: one side only -> taken).
    assert_eq!(object_body(&merged.objects, 30), constant(false));
    assert_eq!(object_body(&merged.objects, 4), namespace(&[6, 16, 18, 30]));
    assert_eq!(object_body(&merged.objects, 18), constant(false));
    assert_eq!(merged.objects.len(), 8);
    // Deterministic across repeats.
    for _ in 0..8 {
        let again = match judge_merge(&ancestor, &ours, &theirs).unwrap() {
            MergeOutcome::Merged(merged) => *merged,
            MergeOutcome::Conflict(_) => panic!("repeat must merge"),
        };
        assert_eq!(again.state_root.root, merged.state_root.root);
        assert_eq!(again.objects, merged.objects);
    }
    // Commutative: left/right swap yields the same semantic result.
    let swapped = match judge_merge(&ancestor, &theirs, &ours).unwrap() {
        MergeOutcome::Merged(merged) => *merged,
        MergeOutcome::Conflict(_) => panic!("swap must merge"),
    };
    assert_eq!(swapped.state_root.root, merged.state_root.root);
    assert_eq!(swapped.objects, merged.objects);
    MergeEvidence {
        ancestor_root: hex(ancestor.root.as_bytes()),
        ours_root: hex(ours.root.as_bytes()),
        theirs_root: hex(theirs.root.as_bytes()),
        merged_root: hex(merged.state_root.root.as_bytes()),
        merged_objects: merged.objects.len(),
        conflicts: 0,
    }
}

struct OverlapEvidence {
    entity_byte: u8,
    reason_tag: u32,
    field: u32,
    kind: u32,
    conflict_id: String,
    stored_bytes_len: usize,
}

/// Both branches touch typedef 6 incompatibly -> canonical conflict object
/// with exactly 1 entry (J7 field-level disagreement), never silent.
fn run_overlapping_change() -> (OverlapEvidence, &'static str) {
    let base = base_bodies();
    let ancestor = synthetic(50, &base);
    let ours = synthetic(51, &with(&base, 6, typedef(Visibility::Exported)));
    let theirs = synthetic(52, &with(&base, 6, typedef(Visibility::Workspace)));
    let outcome = judge_merge(&ancestor, &ours, &theirs).expect("overlap judges");
    let stored = match outcome {
        MergeOutcome::Conflict(stored) => *stored,
        MergeOutcome::Merged(_) => panic!("overlapping change must conflict, never merge"),
    };
    assert_eq!(stored.conflict.conflicts.len(), 1, "expected_conflicts 1");
    let entry = &stored.conflict.conflicts[0];
    assert_eq!(entry.entity_id, id(6));
    assert_eq!(entry.reason, ConflictReason::FieldEdit);
    assert!(!stored.stored_bytes.is_empty(), "conflict object has bytes");
    // The conflict object round-trips through the canonical codec.
    let decoded = decode_merge_conflict(&stored.stored_bytes).expect("conflict decodes");
    assert_eq!(decoded.conflict_id, stored.conflict_id);
    assert_eq!(decoded.conflict, stored.conflict);
    // Deterministic: same inputs, same conflict identity.
    let again = match judge_merge(&ancestor, &ours, &theirs).unwrap() {
        MergeOutcome::Conflict(stored) => *stored,
        MergeOutcome::Merged(_) => panic!("repeat must conflict"),
    };
    assert_eq!(again.conflict_id, stored.conflict_id);
    (
        OverlapEvidence {
            entity_byte: 6,
            reason_tag: entry.reason.tag(),
            field: entry.field,
            kind: entry.kind,
            conflict_id: hex(stored.conflict_id.as_bytes()),
            stored_bytes_len: stored.stored_bytes.len(),
        },
        NEG_OVERLAP_CODE,
    )
}

#[test]
fn s3_merge_fixture_conformance() {
    let positive = run_disjoint_merge();
    assert_eq!(positive.merged_objects, 8);
    assert_eq!(positive.conflicts, 0);
    assert_ne!(positive.ours_root, positive.theirs_root);
    let (negative, code) = run_overlapping_change();
    assert_eq!(code, "ORACLE_MERGE_CONFLICT");
    assert_eq!(negative.entity_byte, 6);
    assert_eq!(negative.reason_tag, ConflictReason::FieldEdit.tag());
    eprintln!(
        "S3_EVIDENCE task=S2B-MERGE-001 merged_root={} objects={}",
        positive.merged_root, positive.merged_objects
    );
    eprintln!(
        "S3_EVIDENCE overlap_conflict={} reason_tag={} field={}",
        negative.conflict_id, negative.reason_tag, negative.field
    );
}

#[test]
#[ignore = "negative oracle handshake; fixture oracle invokes it explicitly"]
fn s3_merge_neg_overlapping_change() {
    let (evidence, code) = run_overlapping_change();
    println!("S3_NEG_RESULT {code}");
    eprintln!("S3_NEG_DETAIL entity={} conflicts=1", evidence.entity_byte);
    panic!(
        "negative control demonstrated: overlap conflicts with {code}; failing by design for the oracle handshake"
    );
}

/// Ignored emitter for the coordinator regen driver.
#[test]
#[ignore = "fixture refresh emitter; run through the generator script"]
fn emit_s3_merge_fixture() {
    let positive = run_disjoint_merge();
    let (negative, _) = run_overlapping_change();
    println!("S3_FIXTURE|task|S2B-MERGE-001");
    println!("S3_FIXTURE|arm|sley2");
    println!("S3_FIXTURE|ancestor_root|{}", positive.ancestor_root);
    println!("S3_FIXTURE|ours_root|{}", positive.ours_root);
    println!("S3_FIXTURE|theirs_root|{}", positive.theirs_root);
    println!("S3_FIXTURE|merged_root|{}", positive.merged_root);
    println!("S3_FIXTURE|merged_objects|{}", positive.merged_objects);
    println!("S3_FIXTURE|conflicts|{}", positive.conflicts);
    println!(
        "S3_FIXTURE|neg_overlapping_conflict|{}",
        negative.conflict_id
    );
    println!(
        "S3_FIXTURE|neg_overlapping_reason_tag|{}",
        negative.reason_tag
    );
    println!("S3_FIXTURE|neg_overlapping_field|{}", negative.field);
    println!("S3_FIXTURE|neg_overlapping_kind|{}", negative.kind);
    println!(
        "S3_FIXTURE|neg_overlapping_bytes|{}",
        negative.stored_bytes_len
    );
    println!("S3_FIXTURE|neg_overlapping_change|{NEG_OVERLAP_CODE}");
}
