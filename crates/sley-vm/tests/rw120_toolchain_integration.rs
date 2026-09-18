//! Cross-component RW-120 integration construction.

use sha2::{Digest, Sha256};

#[path = "rw080_checker_scaffold.rs"]
mod checker;
#[path = "rw080_codec_program_outer.rs"]
mod codec;
#[path = "rw120_toolchain_integration/component.rs"]
mod component;
#[path = "rw120_toolchain_integration/handoff.rs"]
mod handoff;
#[path = "rw080_lower_scaffold.rs"]
mod lower;

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;

    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").unwrap();
        output
    })
}

fn preserve_seed_artifact(environment: &str, bytes: &[u8]) {
    use std::io::Write as _;

    let Some(path) = std::env::var_os(environment) else {
        return;
    };
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap_or_else(|error| {
            panic!(
                "{environment} must name a new preservation path ({}): {error}",
                std::path::Path::new(&path).display()
            )
        });
    output
        .write_all(bytes)
        .unwrap_or_else(|error| panic!("failed to preserve {environment}: {error}"));
    output
        .sync_all()
        .unwrap_or_else(|error| panic!("failed to sync {environment}: {error}"));
}

fn reconstruction_case(
    reference: &component::DriverReference,
) -> (Vec<sley_ssmc::ConstValue>, sley_ssmc::ConstValue) {
    let reconstruction =
        lower::integration_toolchain_reconstruction_test(&reference.lowered, &reference.package);
    let (codec_inputs, expected_codec) = codec::integration_codec_test();
    let (checker_inputs, expected_checker) = checker::integration_checker_test();
    let mut inputs = codec_inputs;
    inputs.extend(checker_inputs);
    inputs.extend(reconstruction.lower_inputs);
    inputs.extend(reconstruction.builder_tail_inputs);
    let expected_values = vec![
        expected_codec,
        expected_checker,
        reconstruction.expected_lower,
        reconstruction.expected_builder,
    ];
    let expected = sley_ssmc::ConstValue {
        value_type: sley_ssmc::TypeExpr::Tuple(
            expected_values
                .iter()
                .map(|value| value.value_type.clone())
                .collect(),
        ),
        data: sley_ssmc::ConstData::Sequence(expected_values),
    };
    (inputs, expected)
}

#[test]
fn bounded_component_programs_are_available_to_one_integration_crate() {
    let codec = codec::integration_bounded_codec_program();
    let checker = checker::integration_checker_program();
    let (lowerer, builder) = lower::integration_lowerer_programs();
    assert_eq!(codec.functions.len(), 31);
    assert_eq!(checker.functions.len(), 8);
    assert_eq!(lowerer.functions.len(), 35);
    assert_eq!(builder.functions.len(), 26);
}

#[test]
fn bounded_programs_merge_without_semantic_identity_collisions() {
    let merged = component::bounded_merged_program();
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
fn bounded_merged_component_root_executes_all_four_programs() {
    component::validate_component_test();
    let merged = component::bounded_merged_program();
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
    let codec = codec::integration_bounded_codec_program();
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
        "RW120_BOUNDED_COMPONENT objects={} object_bytes={} object_sha256={} root={} root_bytes={} root_sha256={}",
        evidence.objects.len(),
        object_bytes,
        hex(&object_digest),
        hex(evidence.root.root.as_bytes()),
        evidence.root.stored_bytes.len(),
        hex(&root_digest),
    );
}

#[test]
fn bounded_integrated_driver_calls_all_four_real_programs_in_one_execution() {
    let fixture = component::bounded_driver_fixture();
    assert_eq!(fixture.program.entry_points.len(), 5);
    assert_eq!(fixture.program.functions.len(), 101);
    assert_eq!(fixture.program.parameters.len(), 5_147);
    assert_eq!(fixture.program.blocks.len(), 1_847);
    assert_eq!(fixture.program.operations.len(), 4_054);
    assert_eq!(fixture.inputs.len(), 75);
    let evidence = component::component_evidence(&fixture.program);
    let execution = component::execute_driver(
        &fixture.program,
        fixture.entry,
        evidence.root.root,
        fixture.inputs,
    );
    assert_eq!(execution.value, fixture.expected);
    let object_bytes = evidence
        .objects
        .iter()
        .map(|object| object.stored_bytes().len())
        .sum::<usize>();
    let mut object_hasher = Sha256::new();
    for object in &evidence.objects {
        object_hasher.update(object.stored_bytes());
    }
    let object_digest: [u8; 32] = object_hasher.finalize().into();
    let root_digest: [u8; 32] = Sha256::digest(&evidence.root.stored_bytes).into();
    assert_eq!(evidence.objects.len(), 11_869);
    assert_eq!(object_bytes, 2_969_080);
    assert_eq!(
        hex(&object_digest),
        "a9f4aaa72edbb0382df8dd134e1495599bc20d1c1052f842db0bf230978d07a4"
    );
    assert_eq!(
        hex(evidence.root.root.as_bytes()),
        "65b567be0c60ab007d57e4fd990f5c83d5f4c6a13aea6ce5eed03d20da4362f7"
    );
    assert_eq!(evidence.root.stored_bytes.len(), 783_784);
    assert_eq!(
        hex(&root_digest),
        "396cb4bf7ddb1911f2c3f49a68c38735156f02714ae8bde0b04209ebf3ac5c45"
    );
    assert_eq!(execution.image_bytes, 490_920);
    assert_eq!(
        hex(&execution.package_digest),
        "4516219fab750bef20d30714a6c033318379af534dba55b297210ad27793ea4f"
    );
    assert_eq!(execution.gate_operation_count, 4_054);
    assert_eq!(execution.gate_bridge_uses, 147);
    eprintln!(
        "RW120_BOUNDED_DRIVER objects={} object_bytes={} object_sha256={} root={} root_bytes={} root_sha256={} image_bytes={} package_digest={} gate_operations={} gate_bridges={}",
        evidence.objects.len(),
        object_bytes,
        hex(&object_digest),
        hex(evidence.root.root.as_bytes()),
        evidence.root.stored_bytes.len(),
        hex(&root_digest),
        execution.image_bytes,
        hex(&execution.package_digest),
        execution.gate_operation_count,
        execution.gate_bridge_uses,
    );
}

#[test]
fn bounded_integrated_driver_hands_lowered_bytes_to_the_package_builder() {
    let fixture = handoff::bounded_fixture();
    assert_eq!(fixture.program.entry_points.len(), 5);
    assert_eq!(fixture.program.functions.len(), 101);
    assert_eq!(fixture.program.parameters.len(), 5_148);
    assert_eq!(fixture.program.blocks.len(), 1_849);
    assert_eq!(fixture.program.operations.len(), 4_058);
    assert_eq!(fixture.inputs.len(), 74);
    let builder_entry = fixture.program.entry_points[3];
    let builder_call = fixture
        .program
        .operations
        .iter()
        .find(|operation| {
            matches!(
                &operation.immediate,
                sley_ssmc::Immediate::Function(reference) if reference.function == builder_entry
            )
        })
        .expect("handoff driver directly calls the package builder");
    let sley_ssmc::ValueRef::Parameter(image_payload) = builder_call.operands[0] else {
        panic!("package-builder image operand is the lower-success block payload")
    };
    let payload = fixture
        .program
        .parameters
        .iter()
        .find(|parameter| parameter.entity_id == image_payload)
        .expect("lower-success payload is retained");
    assert_eq!(payload.role, sley_ssmc::ParameterRole::Block);
    assert_eq!(payload.value_type, sley_ssmc::TypeExpr::Bytes);
    assert_ne!(payload.owner, fixture.entry);
    let evidence = component::component_evidence(&fixture.program);
    let execution = component::execute_driver(
        &fixture.program,
        fixture.entry,
        evidence.root.root,
        fixture.inputs,
    );
    assert_eq!(execution.value, fixture.expected);
    let object_bytes = evidence
        .objects
        .iter()
        .map(|object| object.stored_bytes().len())
        .sum::<usize>();
    let mut object_hasher = Sha256::new();
    for object in &evidence.objects {
        object_hasher.update(object.stored_bytes());
    }
    let object_digest: [u8; 32] = object_hasher.finalize().into();
    let root_digest: [u8; 32] = Sha256::digest(&evidence.root.stored_bytes).into();
    assert_eq!(evidence.objects.len(), 11_876);
    assert_eq!(object_bytes, 2_971_077);
    assert_eq!(
        hex(&object_digest),
        "c40eab81641a843f4983c603cd3483bfcca7a3488658b053576e883deb0d336d"
    );
    assert_eq!(
        hex(evidence.root.root.as_bytes()),
        "4cbcd1eeea202d482895e70ec91f1e0d154c1b7599cc19d60cc6751e61ecfe47"
    );
    assert_eq!(evidence.root.stored_bytes.len(), 784_246);
    assert_eq!(
        hex(&root_digest),
        "85f94269365956705d3d7206ca2aa9a65099da1d55141fe06e3b60f0ec93bbde"
    );
    assert_eq!(execution.image_bytes, 491_378);
    assert_eq!(
        hex(&execution.package_digest),
        "8b45b177d8271d1b2fc1a2edbe78565ec1609b3d86fbe74eb5371bb0d2f13cdf"
    );
    assert_eq!(execution.gate_operation_count, 4_058);
    assert_eq!(execution.gate_bridge_uses, 147);
    eprintln!(
        "RW120_BOUNDED_HANDOFF objects={} object_bytes={} object_sha256={} root={} root_bytes={} root_sha256={} image_bytes={} package_digest={} gate_operations={} gate_bridges={}",
        evidence.objects.len(),
        object_bytes,
        hex(&object_digest),
        hex(evidence.root.root.as_bytes()),
        evidence.root.stored_bytes.len(),
        hex(&root_digest),
        execution.image_bytes,
        hex(&execution.package_digest),
        execution.gate_operation_count,
        execution.gate_bridge_uses,
    );
}

#[test]
#[ignore = "qualification: complete fact-fed reconstruction is intentionally measured separately"]
fn bounded_integrated_driver_reconstructs_its_complete_executable_package() {
    let fixture = handoff::bounded_fixture();
    let evidence = component::component_evidence(&fixture.program);
    let limits = component::reconstruction_limits();
    let reference = component::reference_driver_package_with_limits(
        &fixture.program,
        fixture.entry,
        evidence.root.root,
        limits,
    );
    let (inputs, expected) = reconstruction_case(&reference);
    assert_eq!(inputs.len(), 74);
    let execution = component::execute_driver_with_limits(
        &fixture.program,
        fixture.entry,
        evidence.root.root,
        inputs,
        limits,
    );
    assert_eq!(execution.value, expected);
    let sley_ssmc::ConstData::Sequence(results) = &execution.value.data else {
        unreachable!("integrated driver result is a tuple")
    };
    let sley_ssmc::ConstData::Result(sley_ssmc::ResultConst::Ok(envelope)) = &results[3].data
    else {
        unreachable!("toolchain package reconstruction succeeds")
    };
    let sley_ssmc::ConstData::Bytes(envelope) = &envelope.data else {
        unreachable!("package builder returns envelope bytes")
    };
    assert_eq!(
        envelope,
        &sley_vm::encode_package_envelope_v2(&reference.package).unwrap()
    );
    let decoded = sley_vm::decode_package_envelope_v2(envelope).unwrap();
    assert_eq!(decoded.image_bytes, reference.lowered.bytes);
    assert_eq!(decoded.entry, fixture.entry);
    assert_eq!(decoded.state_root, evidence.root.root);
    let loaded = sley_vm::host_abi::load_image(&decoded.image_bytes).unwrap();
    assert_eq!(loaded.entry.function, fixture.entry);
    assert_eq!(loaded.callees.len(), 100);
    let envelope_digest: [u8; 32] = Sha256::digest(envelope).into();
    let image_digest: [u8; 32] = Sha256::digest(&decoded.image_bytes).into();
    assert_eq!(decoded.image_bytes.len(), 491_378);
    assert_eq!(
        hex(&image_digest),
        "dffdfbbd96585d92a1088f46597ec34bf2271248062a3a84437ba844302552c5"
    );
    assert_eq!(envelope.len(), 533_671);
    assert_eq!(
        hex(&envelope_digest),
        "a73fe4cf621826f490adbb50862d3104a99fba56ba06e1613fd52e1672ec865c"
    );
    assert_eq!(
        hex(&decoded.digests.package_digest),
        "d53bde8d226da2aee57bddeaf093464ef7cbd5f857916dee71aa085c5dcd60e1"
    );
    assert_eq!(execution.instruction_count, 15_480_658);
    assert_eq!(execution.fuel_used, 74_670_072);
    assert_eq!(execution.peak_value_units, 6_027_165_516_738);
    preserve_seed_artifact("SLEY_C1_OUTPUT", envelope);
    eprintln!(
        "RW120_RECONSTRUCTION image_bytes={} image_sha256={} callee_count={} envelope_bytes={} envelope_sha256={} package_digest={} instructions={} fuel={} peak_value_units={}",
        decoded.image_bytes.len(),
        hex(&image_digest),
        loaded.callees.len(),
        envelope.len(),
        hex(&envelope_digest),
        hex(&decoded.digests.package_digest),
        execution.instruction_count,
        execution.fuel_used,
        execution.peak_value_units,
    );
}

#[test]
#[ignore = "qualification: checker-package reconstruction is measured separately"]
fn integrated_driver_reconstructs_the_checker_package() {
    let fixture = handoff::fixture();
    let evidence = component::component_evidence(&fixture.program);
    let target = component::checker_program();
    let target_entry = target.entry_points[0];
    let limits = component::reconstruction_limits();
    let reference = component::reference_driver_package_with_limits(
        &target,
        target_entry,
        evidence.root.root,
        limits,
    );
    let (inputs, expected) = reconstruction_case(&reference);
    let execution = component::execute_driver_with_limits(
        &fixture.program,
        fixture.entry,
        evidence.root.root,
        inputs,
        limits,
    );
    assert_eq!(execution.value, expected);
    let sley_ssmc::ConstData::Sequence(results) = &execution.value.data else {
        unreachable!("integrated driver result is a tuple")
    };
    let sley_ssmc::ConstData::Result(sley_ssmc::ResultConst::Ok(envelope)) = &results[3].data
    else {
        unreachable!("checker package reconstruction succeeds")
    };
    let sley_ssmc::ConstData::Bytes(envelope) = &envelope.data else {
        unreachable!("package builder returns envelope bytes")
    };
    assert_eq!(
        envelope,
        &sley_vm::encode_package_envelope_v2(&reference.package).unwrap()
    );
    let decoded = sley_vm::decode_package_envelope_v2(envelope).unwrap();
    assert_eq!(decoded.image_bytes, reference.lowered.bytes);
    assert_eq!(decoded.entry, target_entry);
    assert_eq!(decoded.state_root, evidence.root.root);
    let loaded = sley_vm::host_abi::load_image(&decoded.image_bytes).unwrap();
    assert_eq!(loaded.entry.function, target_entry);
    assert_eq!(loaded.callees.len(), 7);
    let envelope_digest: [u8; 32] = Sha256::digest(envelope).into();
    let image_digest: [u8; 32] = Sha256::digest(&decoded.image_bytes).into();
    assert_eq!(decoded.image_bytes.len(), 35_032);
    assert_eq!(
        hex(&image_digest),
        "cea90d9e204e1f5f3f6fc0e7a6d7edd3a639e924d02e8c2c794c9500a9f09bd2"
    );
    assert_eq!(envelope.len(), 40_280);
    assert_eq!(
        hex(&envelope_digest),
        "08d9db121febb04bdf79e24e201d6e5d402ac8246f69e79c045f3d6e15e2bbed"
    );
    assert_eq!(
        hex(&decoded.digests.package_digest),
        "c3ec1743ed10de737d1eb1d594255783c6ffc9df76771d8fae3e99352778467c"
    );
    assert_eq!(execution.instruction_count, 1_146_281);
    assert_eq!(execution.fuel_used, 5_491_836);
    assert_eq!(execution.peak_value_units, 35_065_761_207);
    eprintln!(
        "RW120_CHECKER_RECONSTRUCTION image_bytes={} image_sha256={} callee_count={} envelope_bytes={} envelope_sha256={} package_digest={} instructions={} fuel={} peak_value_units={}",
        decoded.image_bytes.len(),
        hex(&image_digest),
        loaded.callees.len(),
        envelope.len(),
        hex(&envelope_digest),
        hex(&decoded.digests.package_digest),
        execution.instruction_count,
        execution.fuel_used,
        execution.peak_value_units,
    );
}

/// Digest summary of one component evidence set.
fn component_digests(evidence: &component::ComponentEvidence) -> (usize, String, String) {
    let object_bytes = evidence
        .objects
        .iter()
        .map(|object| object.stored_bytes().len())
        .sum::<usize>();
    let mut object_hasher = Sha256::new();
    for object in &evidence.objects {
        object_hasher.update(object.stored_bytes());
    }
    let object_digest: [u8; 32] = object_hasher.finalize().into();
    let root_digest: [u8; 32] = Sha256::digest(&evidence.root.stored_bytes).into();
    (object_bytes, hex(&object_digest), hex(&root_digest))
}

/// The canonical union: the arbitrary codec component merged with the
/// retained checker, lowerer, and builder, with all four programs executing
/// from its root.
#[test]
fn merged_component_root_executes_all_four_canonical_programs() {
    let merged = component::merged_program();
    assert_eq!(merged.entry_points.len(), 4);
    let evidence = component::component_evidence(&merged);
    assert_eq!(
        evidence.root.record.entity_bindings.len(),
        evidence.objects.len()
    );
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
            .expect("arbitrary component object reimports"),
            *object
        );
    }
    assert_eq!(
        sley_state_root::import_state_root(
            &sley_state_root::conformance_registry().unwrap(),
            &evidence.root.stored_bytes,
        )
        .expect("arbitrary component root reimports"),
        evidence.root
    );
    let (object_bytes, object_digest, root_digest) = component_digests(&evidence);
    assert_eq!(merged.functions.len(), 201);
    assert_eq!(merged.parameters.len(), 9_687);
    assert_eq!(merged.blocks.len(), 3_233);
    assert_eq!(merged.operations.len(), 6_483);
    assert_eq!(merged.constants.len(), 669);
    assert_eq!(evidence.objects.len(), 20_293);
    assert_eq!(object_bytes, 5_109_711);
    assert_eq!(
        object_digest,
        "0c9703d091302c7d693b34032bf9b640079c928a81de8a1f495034a7e5da1248"
    );
    assert_eq!(
        hex(evidence.root.root.as_bytes()),
        "95f3ca026a3482a6d20ccac47eb41a3967b701eab7cd064c9b553716c691cf40"
    );
    assert_eq!(evidence.root.stored_bytes.len(), 1_339_736);
    assert_eq!(
        root_digest,
        "b51ba7ca953d63bd8cba9982b3cc0c88281ea52b1024e5f0afcd827bdef138f2"
    );
    eprintln!(
        "RW120_COMPONENT functions={} parameters={} blocks={} operations={} constants={} objects={} object_bytes={} object_sha256={} root={} root_bytes={} root_sha256={}",
        merged.functions.len(),
        merged.parameters.len(),
        merged.blocks.len(),
        merged.operations.len(),
        merged.constants.len(),
        evidence.objects.len(),
        object_bytes,
        object_digest,
        hex(evidence.root.root.as_bytes()),
        evidence.root.stored_bytes.len(),
        root_digest,
    );
}

/// The integrated driver over the canonical union: one execution calling
/// all four real programs.
#[test]
fn integrated_driver_calls_all_four_real_programs_in_one_execution() {
    let fixture = component::driver_fixture();
    assert_eq!(fixture.program.entry_points.len(), 5);
    let evidence = component::component_evidence(&fixture.program);
    let execution = component::execute_driver(
        &fixture.program,
        fixture.entry,
        evidence.root.root,
        fixture.inputs,
    );
    assert_eq!(execution.value, fixture.expected);
    let (object_bytes, object_digest, root_digest) = component_digests(&evidence);
    assert_eq!(fixture.program.functions.len(), 202);
    assert_eq!(fixture.program.parameters.len(), 9_762);
    assert_eq!(fixture.program.blocks.len(), 3_234);
    assert_eq!(fixture.program.operations.len(), 6_488);
    assert_eq!(evidence.objects.len(), 20_376);
    assert_eq!(object_bytes, 5_129_407);
    assert_eq!(
        object_digest,
        "c501299d61cd3354b45206a38b50fb2ce496011d7d7de5591280302ab1c927c8"
    );
    assert_eq!(
        hex(evidence.root.root.as_bytes()),
        "748312058f8a13f7a27c03e91cf1d7a1eaf0b9f8cefd56ed39ed88fab0394af5"
    );
    assert_eq!(evidence.root.stored_bytes.len(), 1_345_247);
    assert_eq!(
        root_digest,
        "d3a5b0a01d64b59ab85150a6fae6b50fe3280b8cc6764acb7dbb0442bb19d039"
    );
    assert_eq!(execution.image_bytes, 816_822);
    assert_eq!(
        hex(&execution.package_digest),
        "9514c3193c74af11e717ca548b05fe8cb58185cb96a2bed2e7987c9122763da0"
    );
    assert_eq!(execution.gate_operation_count, 6_488);
    assert_eq!(execution.gate_bridge_uses, 217);
    eprintln!(
        "RW120_DRIVER functions={} parameters={} blocks={} operations={} objects={} object_bytes={} object_sha256={} root={} root_bytes={} root_sha256={} image_bytes={} package_digest={} gate_operations={} gate_bridges={} instructions={} fuel={} peak={}",
        fixture.program.functions.len(),
        fixture.program.parameters.len(),
        fixture.program.blocks.len(),
        fixture.program.operations.len(),
        evidence.objects.len(),
        object_bytes,
        object_digest,
        hex(evidence.root.root.as_bytes()),
        evidence.root.stored_bytes.len(),
        root_digest,
        execution.image_bytes,
        hex(&execution.package_digest),
        execution.gate_operation_count,
        execution.gate_bridge_uses,
        execution.instruction_count,
        execution.fuel_used,
        execution.peak_value_units,
    );
}

/// The canonical `S` construction: the handoff driver hands the lowered
/// bytes to the package builder from a root binding the complete closure.
/// `canonical-s-manifest.json` and `check_reweave_canonical_s.py` pin the
/// facts this test prints.
#[test]
fn integrated_driver_hands_lowered_bytes_to_the_package_builder() {
    let fixture = handoff::fixture();
    assert_eq!(fixture.program.entry_points.len(), 5);
    assert_eq!(fixture.program.functions.len(), 202);
    assert_eq!(fixture.program.parameters.len(), 9_763);
    assert_eq!(fixture.program.blocks.len(), 3_236);
    assert_eq!(fixture.program.operations.len(), 6_492);
    assert_eq!(fixture.program.constants.len(), 669);
    assert_eq!(fixture.inputs.len(), 74);
    let evidence = component::component_evidence(&fixture.program);
    let execution = component::execute_driver(
        &fixture.program,
        fixture.entry,
        evidence.root.root,
        fixture.inputs,
    );
    assert_eq!(execution.value, fixture.expected);
    let (object_bytes, object_digest, root_digest) = component_digests(&evidence);
    assert_eq!(evidence.objects.len(), 20_383);
    assert_eq!(object_bytes, 5_131_404);
    assert_eq!(
        object_digest,
        "d7d2fcb57d19676c269559bc2a938c04ada682a46b4c34f197b5126901ded25a"
    );
    assert_eq!(
        hex(evidence.root.root.as_bytes()),
        "48f8af2422b3e56b2924f56b11ca07f141b87541cdba6584cf1c6e87602577dd"
    );
    assert_eq!(evidence.root.stored_bytes.len(), 1_345_709);
    assert_eq!(
        root_digest,
        "76235656984ec59963b8b39a425cb6af721069a25cafca0b6988dfea3ffb2e8c"
    );
    assert_eq!(execution.image_bytes, 817_280);
    assert_eq!(
        hex(&execution.package_digest),
        "9e684e1c84c955e0451bfd9443436224cac5358e51e4cbe8b6f518bb9dc2503c"
    );
    assert_eq!(execution.gate_operation_count, 6_492);
    assert_eq!(execution.gate_bridge_uses, 217);
    eprintln!(
        "RW120_HANDOFF functions={} parameters={} blocks={} operations={} constants={} objects={} object_bytes={} object_sha256={} root={} root_bytes={} root_sha256={} image_bytes={} package_digest={} gate_operations={} gate_bridges={}",
        fixture.program.functions.len(),
        fixture.program.parameters.len(),
        fixture.program.blocks.len(),
        fixture.program.operations.len(),
        fixture.program.constants.len(),
        evidence.objects.len(),
        object_bytes,
        object_digest,
        hex(evidence.root.root.as_bytes()),
        evidence.root.stored_bytes.len(),
        root_digest,
        execution.image_bytes,
        hex(&execution.package_digest),
        execution.gate_operation_count,
        execution.gate_bridge_uses,
    );
}

/// The C1 qualification: the Sley toolchain reconstructs its own complete
/// executable package over canonical `S`. Ignored because it is measured
/// separately; `SLEY_C1_OUTPUT` preserves the envelope.
#[test]
#[ignore = "qualification: complete fact-fed reconstruction is intentionally measured separately"]
fn integrated_driver_reconstructs_its_complete_executable_package() {
    let fixture = handoff::fixture();
    let evidence = component::component_evidence(&fixture.program);
    let limits = component::reconstruction_limits();
    let reference = component::reference_driver_package_with_limits(
        &fixture.program,
        fixture.entry,
        evidence.root.root,
        limits,
    );
    let (inputs, expected) = reconstruction_case(&reference);
    assert_eq!(inputs.len(), 74);
    let started = std::time::Instant::now();
    let execution = component::execute_driver_with_limits(
        &fixture.program,
        fixture.entry,
        evidence.root.root,
        inputs,
        limits,
    );
    let seconds = started.elapsed().as_secs_f64();
    assert_eq!(execution.value, expected);
    let sley_ssmc::ConstData::Sequence(results) = &execution.value.data else {
        unreachable!("integrated driver result is a tuple")
    };
    let sley_ssmc::ConstData::Result(sley_ssmc::ResultConst::Ok(envelope)) = &results[3].data
    else {
        unreachable!("toolchain package reconstruction succeeds")
    };
    let sley_ssmc::ConstData::Bytes(envelope) = &envelope.data else {
        unreachable!("package builder returns envelope bytes")
    };
    assert_eq!(
        envelope,
        &sley_vm::encode_package_envelope_v2(&reference.package).unwrap()
    );
    let decoded = sley_vm::decode_package_envelope_v2(envelope).unwrap();
    assert_eq!(decoded.image_bytes, reference.lowered.bytes);
    assert_eq!(decoded.entry, fixture.entry);
    assert_eq!(decoded.state_root, evidence.root.root);
    let loaded = sley_vm::host_abi::load_image(&decoded.image_bytes).unwrap();
    assert_eq!(loaded.entry.function, fixture.entry);
    assert_eq!(loaded.callees.len(), 201);
    let envelope_digest: [u8; 32] = Sha256::digest(envelope).into();
    let image_digest: [u8; 32] = Sha256::digest(&decoded.image_bytes).into();
    assert_eq!(decoded.image_bytes.len(), 817_280);
    assert_eq!(
        hex(&image_digest),
        "b9b18cabbb5609dfa52049ce0358c0f6d65c10ced0ef371882c35b4f85368d37"
    );
    assert_eq!(envelope.len(), 860_112);
    assert_eq!(
        hex(&envelope_digest),
        "2b35e6e76fe325b96869c0ca65ac0070a5e98683866042596891ab6a614a0210"
    );
    assert_eq!(
        hex(&decoded.digests.package_digest),
        "fade888f2037e7202d18f7ade5ef7e6f5a38f038028cf6fe6759ba6c98504d11"
    );
    assert_eq!(execution.instruction_count, 25_881_896);
    assert_eq!(execution.fuel_used, 123_242_409);
    assert_eq!(execution.peak_value_units, 15_797_684_686_117);
    eprintln!(
        "RW120_C1 root={} callees={} image_bytes={} image_sha256={} envelope_bytes={} envelope_sha256={} package_digest={} instructions={} fuel={} peak={} seconds={seconds:.2}",
        hex(evidence.root.root.as_bytes()),
        loaded.callees.len(),
        decoded.image_bytes.len(),
        hex(&image_digest),
        envelope.len(),
        hex(&envelope_digest),
        hex(&decoded.digests.package_digest),
        execution.instruction_count,
        execution.fuel_used,
        execution.peak_value_units,
    );
    preserve_seed_artifact("SLEY_C1_OUTPUT", envelope);
}

#[test]
fn canonical_component_programs_are_available_to_one_integration_crate() {
    let codec = codec::integration_codec_program();
    let checker = checker::integration_checker_program();
    let (lowerer, builder) = lower::integration_lowerer_programs();
    assert_eq!(codec.functions.len(), 132);
    assert_eq!(checker.functions.len(), 8);
    assert_eq!(lowerer.functions.len(), 35);
    assert_eq!(builder.functions.len(), 26);
}

#[test]
fn canonical_programs_merge_without_semantic_identity_collisions() {
    let merged = component::merged_program();
    assert_eq!(merged.entry_points.len(), 4);
    assert_eq!(merged.functions.len(), 201);
    assert_eq!(merged.parameters.len(), 9_687);
    assert_eq!(merged.blocks.len(), 3_233);
    assert_eq!(merged.operations.len(), 6_483);
    assert_eq!(merged.constants.len(), 669);
    assert_eq!(merged.adapters.len(), 4);
}
