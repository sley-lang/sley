//! Cross-component RW-120 integration construction.

use sha2::{Digest, Sha256};

#[path = "rw080_checker_scaffold.rs"]
mod checker;
#[path = "rw080_codec_program_outer.rs"]
mod codec;
#[path = "rw120_toolchain_integration/component.rs"]
mod component;
#[path = "rw080_lower_scaffold.rs"]
mod lower;

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;

    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").unwrap();
        output
    })
}

#[test]
fn canonical_component_programs_are_available_to_one_integration_crate() {
    let codec = codec::integration_codec_program();
    let checker = checker::integration_checker_program();
    let (lowerer, builder) = lower::integration_lowerer_programs();
    assert_eq!(codec.functions.len(), 31);
    assert_eq!(checker.functions.len(), 8);
    assert_eq!(lowerer.functions.len(), 35);
    assert_eq!(builder.functions.len(), 26);
}

#[test]
fn canonical_programs_merge_without_semantic_identity_collisions() {
    let merged = component::merged_program();
    assert_eq!(merged.entry_points.len(), 4);
    assert_eq!(merged.functions.len(), 100);
    assert_eq!(merged.parameters.len(), 5_072);
    assert_eq!(merged.blocks.len(), 1_846);
    assert_eq!(merged.operations.len(), 4_049);
    assert_eq!(merged.constants.len(), 699);
    assert_eq!(merged.adapters.len(), 4);
    eprintln!(
        "RW120_MERGED functions={} parameters={} blocks={} operations={} constants={} adapters={}",
        merged.functions.len(),
        merged.parameters.len(),
        merged.blocks.len(),
        merged.operations.len(),
        merged.constants.len(),
        merged.adapters.len(),
    );
}

#[test]
fn merged_component_root_executes_all_four_canonical_programs() {
    component::validate_component_test();
    let merged = component::merged_program();
    let evidence = component::component_evidence(&merged);
    assert_eq!(
        evidence.root.record.entity_bindings.len(),
        evidence.objects.len()
    );
    assert_eq!(evidence.root.record.entry_points.len(), 4);
    let checker = checker::integration_checker_program();
    let actual = checker::integration_execute_checker(
        &checker,
        evidence.root.root,
        evidence.test.inputs.clone(),
    );
    let sley_ssmc::ExpectedOutcome::Value(expected) = &evidence.test.expected else {
        unreachable!("integrated checker test has an exact value expectation")
    };
    assert_eq!(&actual, expected);
    let codec = codec::integration_codec_program();
    assert_eq!(
        codec::integration_execute_codec(&codec, evidence.root.root),
        codec::integration_codec_expected()
    );
    let (lowerer, builder) = lower::integration_lowerer_programs();
    let (actual_lowered, expected_lowered) =
        lower::integration_execute_lowerer(&lowerer, evidence.root.root);
    assert_eq!(actual_lowered, expected_lowered);
    let (actual_package, expected_package) =
        lower::integration_execute_builder(&builder, evidence.root.root);
    assert_eq!(actual_package, expected_package);
    for object in &evidence.objects {
        assert_eq!(
            sley_mutate::import_entity_object(
                sley_state_root::conformance_epoch_id().unwrap(),
                object.stored_bytes(),
            )
            .expect("integrated component object reimports"),
            *object
        );
    }
    assert_eq!(
        sley_state_root::import_state_root(
            &sley_state_root::conformance_registry().unwrap(),
            &evidence.root.stored_bytes,
        )
        .expect("integrated component root reimports"),
        evidence.root
    );
    let mut object_hasher = Sha256::new();
    for object in &evidence.objects {
        object_hasher.update(object.stored_bytes());
    }
    let object_digest: [u8; 32] = object_hasher.finalize().into();
    let root_digest: [u8; 32] = Sha256::digest(&evidence.root.stored_bytes).into();
    let object_bytes = evidence
        .objects
        .iter()
        .map(|object| object.stored_bytes().len())
        .sum::<usize>();
    assert_eq!(evidence.objects.len(), 11_786);
    assert_eq!(object_bytes, 2_949_384);
    assert_eq!(
        hex(&object_digest),
        "a9e6d19aa46527453219627e33c0694db1b540525a4b83f5b705f6a304422b97"
    );
    assert_eq!(
        hex(evidence.root.root.as_bytes()),
        "ca4287c72bf8ec1a5c0405554603cd0e4c6afe3ed7434b59236026b1c2b13df9"
    );
    assert_eq!(evidence.root.stored_bytes.len(), 778_273);
    assert_eq!(
        hex(&root_digest),
        "b28ab1c1d237d3f4cf6694cba67e23c07101065d01828ffc7e28060a63daba9c"
    );
    eprintln!(
        "RW120_COMPONENT objects={} object_bytes={} object_sha256={} root={} root_bytes={} root_sha256={}",
        evidence.objects.len(),
        object_bytes,
        hex(&object_digest),
        hex(evidence.root.root.as_bytes()),
        evidence.root.stored_bytes.len(),
        hex(&root_digest),
    );
}
