//! Accepted empty policy root for the RW-080 seed-assembled component.

use sley_id::{GenesisNonce, WorkspaceId};

const GENESIS_SEED: [u8; 32] = [0x80; 32];

fn rw080_construction_policy() -> sley_policy::AcceptedPolicyRoot {
    let workspace = WorkspaceId::derive(GenesisNonce::from_bytes(GENESIS_SEED));
    sley_policy::PolicyRootBuilder::new(workspace)
        .build(&sley_policy::conformance_registry().expect("policy registry is frozen"))
        .expect("empty RW-080 construction policy is accepted")
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;

    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
        output
    })
}

#[test]
fn rw080_construction_policy_is_registry_accepted_and_stable() {
    let policy = rw080_construction_policy();
    let registry = sley_policy::conformance_registry().expect("policy registry is frozen");
    assert_eq!(
        sley_policy::import_policy_root(&registry, policy.stored_bytes())
            .expect("policy reimports"),
        policy
    );
    assert_eq!(
        policy.record().workspace_id,
        WorkspaceId::derive(GenesisNonce::from_bytes(GENESIS_SEED))
    );
    assert_eq!(policy.stored_bytes().len(), 174);
    assert_eq!(
        policy.root().into_bytes(),
        [
            0x3b, 0x8e, 0xab, 0x80, 0xac, 0xdc, 0x87, 0x4b, 0xd3, 0xf3, 0x95, 0x89, 0x81, 0xd0,
            0xda, 0x81, 0xd2, 0xce, 0x23, 0x14, 0x07, 0x3f, 0xc9, 0x77, 0x3d, 0x75, 0xed, 0x90,
            0x1f, 0x0d, 0xc8, 0x9c,
        ]
    );
    println!(
        "rw080_policy root={} bytes={} stored_bytes={}",
        hex(policy.root().as_bytes()),
        policy.stored_bytes().len(),
        hex(policy.stored_bytes())
    );
}
