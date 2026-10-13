//! State-root decoding against the native importer.
mod common;
use common::*;

#[test]
fn decodes_the_toolchain_root_and_refuses_corruption() {
    let s = toolchain_s("cd_object");
    let stored = s.root.stored_bytes.clone();
    let epoch = s.epoch;
    let (value, instructions) =
        run_counted("t_sr_root", vec![bytes(&stored), bytes(epoch.as_bytes())]);
    let facts = tuple_items(&result(value).expect("decodes"));
    assert_eq!(as_bytes(&facts[0]), s.root.root.as_bytes().to_vec());
    assert_eq!(as_u64(&facts[1]), s.objects.len() as u64);
    assert_eq!(as_u64(&facts[2]), 1);
    assert_eq!(
        as_bytes(&facts[3]),
        s.objects[0].object_id().as_bytes().to_vec()
    );
    eprintln!(
        "root {} bytes decoded in {instructions} instructions",
        stored.len()
    );
}

#[test]
fn refuses_corrupted_roots_like_native() {
    // A small program keeps each corrupted decode fast.
    let (program, names) = program();
    let small = sley_selfhost::canonical::prune_to_closure(program, names["rt_rd_uvar"]);
    let s = sley_selfhost::canonical::build(&small, names["rt_rd_uvar"], "root-corruption");
    let stored = s.root.stored_bytes.clone();
    let epoch = s.epoch;
    let registry = sley_state_root::conformance_registry().unwrap();
    let redigest = |mut b: Vec<u8>| {
        let n = b.len();
        let d = blake3::Hasher::new()
            .update(b"sley2.state-root.v1")
            .update(&b[..n - 32])
            .finalize();
        b[n - 32..].copy_from_slice(d.as_bytes());
        b
    };
    let mut checked = 0;
    let positions: Vec<usize> = (0..300)
        .chain((300..stored.len() - 32).step_by(97))
        .collect();
    for position in positions {
        for replacement in [0x00_u8, 0x01, 0x7f, 0x80, stored[position] ^ 0x10] {
            let mut mutated = stored.clone();
            mutated[position] = replacement;
            let mutated = redigest(mutated);
            let native = sley_state_root::import_state_root(&registry, &mutated).is_ok();
            let sley = result(run(
                "t_sr_root",
                vec![bytes(&mutated), bytes(epoch.as_bytes())],
            ))
            .is_ok();
            assert_eq!(sley, native, "position {position} -> {replacement:02x}");
            checked += 1;
        }
    }
    eprintln!("checked {checked} corrupted roots");
}
