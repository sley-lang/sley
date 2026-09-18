//! `PolicyBinding` (entity kind 17) body encoding construction.
//! Construction provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-policy-binding-encode.md`.

use super::*;
use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};

fn policy_binding_encode_image() -> Image {
    let mut assembler = Asm::new();
    let encode_ns = Ns {
        k: 210,
        p: 211,
        b: 212,
        o: 213,
    };
    let policy_ns = Ns {
        k: 214,
        p: 215,
        b: 216,
        o: 217,
    };
    let encode_fid = eid(11, 1);
    let policy_fid = eid(11, 2);
    let encode_graph = build_encode(&mut assembler, encode_ns, encode_fid);
    let policy_graph =
        build_policy_binding_encode(&mut assembler, policy_ns, policy_fid, encode_fid);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: policy_graph.clone(),
        functions: vec![policy_graph, encode_graph],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
        ],
        constants: assembler.constants,
    }
}

fn policy_binding_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_ns = Ns {
        k: 218,
        p: 219,
        b: 220,
        o: 221,
    };
    let policy_ns = Ns {
        k: 222,
        p: 223,
        b: 224,
        o: 225,
    };
    let decode_fid = eid(11, 3);
    let policy_fid = eid(11, 4);
    let (decode_graph, _) = build_decode(&mut assembler, decode_ns, decode_fid);
    let policy_graph =
        build_policy_binding_decode(&mut assembler, policy_ns, policy_fid, decode_fid);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: policy_graph.clone(),
        functions: vec![policy_graph, decode_graph],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
        ],
        constants: assembler.constants,
    }
}

fn policy_binding_encode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    subject: &[u8],
    requirements: &[u8],
) -> sley_vm::ExecutionOutcome {
    execute(
        package,
        approved,
        vec![
            bytes_input(subject),
            bytes_input(requirements),
            unit_input(),
        ],
    )
}

fn policy_binding_decode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    body: &[u8],
) -> sley_vm::ExecutionOutcome {
    execute(package, approved, vec![bytes_input(body), unit_input()])
}

fn policy_binding_body(subject: [u8; 32], requirements: &[[u8; 32]]) -> Vec<u8> {
    use sley_mutate::value::{EntityBodyValue, EntityIdSet, PolicyBindingBody};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes([1; 32]),
        body: EntityBodyValue::PolicyBinding(PolicyBindingBody {
            subject: sley_id::EntityId::from_bytes(subject),
            requirements: EntityIdSet::from_unsorted(
                requirements
                    .iter()
                    .copied()
                    .map(sley_id::EntityId::from_bytes)
                    .collect(),
            )
            .expect("fixture requirements are unique"),
        }),
        label: None,
        semantic_fingerprint: None,
    };
    let object = sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds policy binding fixture");
    ns_body_of(object.stored_bytes())
}

fn concat_ids(ids: &[[u8; 32]]) -> Vec<u8> {
    ids.iter().flatten().copied().collect()
}

fn raw_policy_binding_body(subject: Vec<u8>, requirements: &[Vec<u8>]) -> Vec<u8> {
    let requirements = sley_scb1::encode_list(requirements).expect("requirements list");
    let record = sley_scb1::encode_record(&[(1, subject), (2, requirements)])
        .expect("policy binding record");
    sley_scb1::encode_union(17, &record).expect("policy binding union")
}

#[test]
fn policy_binding_decode_matches_native_semantics() {
    let (package, approved) = admit(&policy_binding_decode_image());
    let mut nonuniform_subject = [0u8; 32];
    for (index, byte) in nonuniform_subject.iter_mut().enumerate() {
        *byte = u8::try_from(index).expect("subject index fits u8");
    }
    let cases = [
        ([5; 32], Vec::new()),
        ([5; 32], vec![[6; 32]]),
        (nonuniform_subject, vec![[6; 32], [7; 32], [8; 32]]),
    ];
    for (subject, requirements) in cases {
        let body = policy_binding_body(subject, &requirements);
        let outcome = policy_binding_decode_call(&package, &approved, &body);
        assert_entity_set_decode_ok(
            &outcome,
            &subject,
            &concat_ids(&requirements),
            u64::try_from(requirements.len()).expect("requirement count fits u64"),
        );
        eprintln!(
            "POLICY_BINDING_DEC body{}B reqs={} fuel={} instr={} peak={}",
            body.len(),
            requirements.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }
}

#[test]
fn policy_binding_decode_matches_native_rejections() {
    let (package, approved) = admit(&policy_binding_decode_image());
    let valid_requirement = vec![6; 32];
    let mut trailing = policy_binding_body([5; 32], &[[6; 32]]);
    trailing.push(0);
    let cases = [
        (
            "subject-short",
            raw_policy_binding_body(vec![5; 31], std::slice::from_ref(&valid_requirement)),
        ),
        (
            "subject-long",
            raw_policy_binding_body(vec![5; 33], std::slice::from_ref(&valid_requirement)),
        ),
        (
            "requirements-duplicate",
            raw_policy_binding_body(vec![5; 32], &[vec![6; 32], vec![6; 32]]),
        ),
        (
            "requirements-order",
            raw_policy_binding_body(vec![5; 32], &[vec![7; 32], vec![6; 32]]),
        ),
        (
            "subject-before-requirements",
            raw_policy_binding_body(vec![5; 33], &[vec![7; 32], vec![6; 32]]),
        ),
        ("body-trailing", trailing),
    ];
    for (name, body) in cases {
        let expected = program_native_code(&ns_wrap_body(1, &body));
        assert_ne!(expected, "OK", "native refuses {name}");
        let outcome = policy_binding_decode_call(&package, &approved, &body);
        assert_refusal(&outcome, &expected);
        eprintln!("POLICY_BINDING_DEC_REJECT {name}->{expected}");
    }

    let namespace_body = ns_body_of(&program_ns_stored(1, None, &[]));
    let scoped = policy_binding_decode_call(&package, &approved, &namespace_body);
    assert_refusal(&scoped, "SSMC_RESERVED_FIELD_PRESENT");
}

#[test]
fn policy_binding_encode_matches_native() {
    let (package, approved) = admit(&policy_binding_encode_image());
    let mut nonuniform_subject = [0u8; 32];
    for (index, byte) in nonuniform_subject.iter_mut().enumerate() {
        *byte = u8::try_from(index).expect("subject index fits u8");
    }
    let cases = [
        ([5; 32], Vec::new()),
        ([5; 32], vec![[6; 32]]),
        (nonuniform_subject, vec![[6; 32], [7; 32], [8; 32]]),
    ];
    for (subject, requirements) in cases {
        let expected = policy_binding_body(subject, &requirements);
        let requirement_bytes = concat_ids(&requirements);
        let outcome = policy_binding_encode_call(&package, &approved, &subject, &requirement_bytes);
        assert_encode_ok(&outcome, &expected);
        let again = policy_binding_encode_call(&package, &approved, &subject, &requirement_bytes);
        assert_encode_ok(&again, &expected);
        eprintln!(
            "POLICY_BINDING_ENC body{}B reqs={} fuel={} instr={} peak={}",
            expected.len(),
            requirements.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }
}

#[test]
fn policy_binding_encode_rejects_malformed_semantics() {
    let (package, approved) = admit(&policy_binding_encode_image());
    let valid_subject = [5; 32];
    let ordered = concat_ids(&[[6; 32], [7; 32]]);
    let cases: [(&str, Vec<u8>, Vec<u8>, &str); 7] = [
        (
            "subject-short",
            vec![5; 31],
            ordered.clone(),
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            "subject-long",
            vec![5; 33],
            ordered.clone(),
            "SCB_TRAILING_BYTES",
        ),
        (
            "requirements-short",
            valid_subject.to_vec(),
            vec![6; 31],
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            "requirements-partial-33",
            valid_subject.to_vec(),
            vec![6; 33],
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            "requirements-order",
            valid_subject.to_vec(),
            concat_ids(&[[7; 32], [6; 32]]),
            "SCB_MAP_ORDER",
        ),
        (
            "requirements-duplicate",
            valid_subject.to_vec(),
            concat_ids(&[[6; 32], [6; 32]]),
            "SCB_MAP_DUPLICATE",
        ),
        (
            "subject-before-requirements",
            vec![5; 33],
            concat_ids(&[[7; 32], [6; 32]]),
            "SCB_TRAILING_BYTES",
        ),
    ];
    for (name, subject, requirements, code) in cases {
        let outcome = policy_binding_encode_call(&package, &approved, &subject, &requirements);
        assert_refusal(&outcome, code);
        eprintln!("POLICY_BINDING_ENC_REJECT {name}->{code}");
    }
}
