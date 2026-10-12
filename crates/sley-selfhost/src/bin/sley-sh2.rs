//! `sley-sh2`: SH2 bootstrap staging and inspection (C0 side).
//!
//! ```text
//! sley-sh2 stage --out DIR        canonical S for the toolchain, plus C1 = C0.build(S, P)
//! sley-sh2 inspect PKG            package digests of one EXEC_PACKAGE_V3 envelope
//! sley-sh2 compare PKG PKG        byte and digest comparison of two packages
//! sley-sh2 mutate-constant --stage DIR --out DIR --from N --to N
//!                                 typed mutation of S: replace the UInt(32)
//!                                 constant N (a toolchain diagnostic code)
//! sley-sh2 c0 --stage DIR --out PKG
//!                                 C0.build of a staged S (recovery and parity)
//! sley-sh2 corrupt --stage DIR --out DIR
//!                                 a copy of S whose first object has a bad magic
//! ```
//!
//! `stage` writes `DIR/S/{epoch.bin, root.bin, objects/<entity-id-hex>.obj}`,
//! `DIR/C1.pkg`, `DIR/C1.digest` (the package identity, hex), and
//! `DIR/manifest.json`. `S` is exactly the build request the pinned host
//! stages for every later stage.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sley_selfhost::{canonical, native, toolchain};

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    hex(&Sha256::digest(bytes))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// Canonical S for the toolchain program, pruned to the driver closure.
fn toolchain_s() -> canonical::CanonicalS {
    let (program, names) = toolchain::module().finish();
    let entry = names[toolchain::driver::ENTRY];
    let pruned = canonical::prune_to_closure(&program, entry);
    canonical::build(&pruned, entry, toolchain::S_SEED)
}

fn stage(out: &Path) -> Result<(), String> {
    let started = std::time::Instant::now();
    let s = toolchain_s();
    let built = std::time::Instant::now();
    let dir = out.join("S");
    let objects = dir.join("objects");
    std::fs::create_dir_all(&objects).map_err(|e| e.to_string())?;
    write(&dir.join("epoch.bin"), s.epoch.as_bytes())?;
    write(&dir.join("root.bin"), &s.root.stored_bytes)?;
    let mut object_bytes = 0_usize;
    for object in &s.objects {
        let name = format!("{}.obj", hex(object.record().entity_id.as_bytes()));
        write(&objects.join(name), object.stored_bytes())?;
        object_bytes += object.stored_bytes().len();
    }
    let c1 = native::c0_build(&s, native::toolchain_limits())?;
    let c0_done = std::time::Instant::now();
    write(&out.join("C1.pkg"), &c1.envelope)?;
    write(&out.join("C1.digest"), hex(&c1.digests.package_digest).as_bytes())?;
    let manifest = format!(
        "{{\n  \"schema\": \"sley2.sh2.stage.v1\",\n  \"state_root\": \"{}\",\n  \"epoch\": \"{}\",\n  \"objects\": {},\n  \"object_bytes\": {},\n  \"root_bytes\": {},\n  \"c1_package_digest\": \"{}\",\n  \"c1_envelope_sha256\": \"{}\",\n  \"c1_envelope_bytes\": {},\n  \"c1_image_bytes\": {},\n  \"seed_ms\": {},\n  \"c0_build_ms\": {}\n}}\n",
        hex(s.root.root.as_bytes()),
        hex(s.epoch.as_bytes()),
        s.objects.len(),
        object_bytes,
        s.root.stored_bytes.len(),
        hex(&c1.digests.package_digest),
        sha256(&c1.envelope),
        c1.envelope.len(),
        c1.package.image_bytes.len(),
        built.duration_since(started).as_millis(),
        c0_done.duration_since(built).as_millis(),
    );
    write(&out.join("manifest.json"), manifest.as_bytes())?;
    print!("{manifest}");
    Ok(())
}

fn digests(path: &Path) -> Result<(Vec<u8>, sley_vm::PackageDigests), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let hydrated = sley_vm::hydrate_package_envelope_v3(&bytes)
        .map_err(|e| format!("{}: not a v3 package: {e:?}", path.display()))?;
    Ok((bytes, hydrated.digests))
}

fn inspect(path: &Path) -> Result<(), String> {
    let (bytes, d) = digests(path)?;
    println!(
        "{{\"package_digest\": \"{}\", \"image_digest\": \"{}\", \"envelope_sha256\": \"{}\", \"bytes\": {}}}",
        hex(&d.package_digest),
        hex(&d.image_digest),
        sha256(&bytes),
        bytes.len()
    );
    Ok(())
}

fn compare(a: &Path, b: &Path) -> Result<(), String> {
    let (left, ld) = digests(a)?;
    let (right, rd) = digests(b)?;
    println!(
        "{{\"identical_bytes\": {}, \"left_package_digest\": \"{}\", \"right_package_digest\": \"{}\"}}",
        left == right,
        hex(&ld.package_digest),
        hex(&rd.package_digest)
    );
    if left == right {
        Ok(())
    } else {
        Err("packages differ".into())
    }
}

/// Loads a staged S.
fn load_stage(dir: &Path) -> Result<canonical::CanonicalS, String> {
    let read = |path: PathBuf| std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()));
    let epoch_bytes: [u8; 32] = read(dir.join("epoch.bin"))?
        .try_into()
        .map_err(|_| "epoch.bin must be 32 bytes".to_owned())?;
    let epoch = sley_id::SchemaEpochId::from_bytes(epoch_bytes);
    let root_bytes = read(dir.join("root.bin"))?;
    let registry = sley_state_root::conformance_registry().map_err(|e| format!("{e:?}"))?;
    let root = sley_state_root::import_state_root(&registry, &root_bytes)
        .map_err(|e| format!("root: {e:?}"))?;
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir.join("objects"))
        .map_err(|e| e.to_string())?
        .map(|entry| entry.map(|e| e.path()).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    names.sort();
    let mut objects = Vec::with_capacity(names.len());
    for name in names {
        objects.push(
            sley_mutate::import_entity_object(epoch, &read(name)?)
                .map_err(|e| format!("object: {e:?}"))?,
        );
    }
    let entry_point = *root.record.entry_points.first().ok_or("no entry point")?;
    let entry_function = objects
        .iter()
        .find_map(|object| match &object.record().body {
            sley_mutate::value::EntityBodyValue::EntryPoint(body)
                if object.record().entity_id == entry_point =>
            {
                Some(body.function)
            }
            _ => None,
        })
        .ok_or("entry point object missing")?;
    Ok(canonical::CanonicalS {
        epoch,
        objects,
        root,
        entry_point,
        entry_function,
    })
}

/// Writes a staged S.
fn write_stage(dir: &Path, s: &canonical::CanonicalS) -> Result<(), String> {
    let objects = dir.join("objects");
    std::fs::create_dir_all(&objects).map_err(|e| e.to_string())?;
    write(&dir.join("epoch.bin"), s.epoch.as_bytes())?;
    write(&dir.join("root.bin"), &s.root.stored_bytes)?;
    for object in &s.objects {
        let name = format!("{}.obj", hex(object.record().entity_id.as_bytes()));
        write(&objects.join(name), object.stored_bytes())?;
    }
    Ok(())
}

/// Replaces the `UInt(32)` constant `from` with `to` through one typed
/// `ReplaceEntityVersion` candidate applied to the staged snapshot.
fn mutate_constant(stage: &Path, out: &Path, from: u32, to: u32) -> Result<(), String> {
    use sley_mutate::value::{ConstantBody, EntityBodyValue};
    use sley_mutate::{
        BoundPrecondition, CandidateExpiry, CandidateRecord, ExactEntityVersion,
        MutationOperation, MutationPayload, PreconditionPayload,
    };
    use sley_ssmc::{ConstData, ConstValue, IntegerWidth, TypeExpr};
    let s = load_stage(stage)?;
    let wanted = ConstValue {
        value_type: TypeExpr::UInt(IntegerWidth::from_bits(32)),
        data: ConstData::UInt(u128::from(from)),
    };
    let matches: Vec<&sley_mutate::EntityObject> = s
        .objects
        .iter()
        .filter(|object| {
            matches!(&object.record().body, EntityBodyValue::Constant(body) if body.value == wanted)
        })
        .collect();
    let [target] = matches.as_slice() else {
        return Err(format!("expected exactly one UInt(32) {from} constant, found {}", matches.len()));
    };
    let target_id = target.record().entity_id;
    let payload = MutationPayload::ReplaceEntityVersion(EntityBodyValue::Constant(ConstantBody {
        value: ConstValue {
            value_type: TypeExpr::UInt(IntegerWidth::from_bits(32)),
            data: ConstData::UInt(u128::from(to)),
        },
    }));
    let precondition = PreconditionPayload::ExactEntityVersion(ExactEntityVersion {
        entity_id: target_id,
        object_id: target.object_id(),
    });
    let fill = |tag: u8| {
        let mut bytes = [tag; 32];
        bytes[..16].copy_from_slice(b"sley.sh2.mutate.");
        bytes
    };
    let record = CandidateRecord {
        format_version: 1,
        workspace_id: s.root.record.workspace_id,
        base_transaction_id: sley_id::TransactionId::from_bytes(fill(1)),
        base_root: s.root.root,
        schema_epoch_id: s.epoch,
        policy_root_id: s.root.record.policy_root,
        principal_id: sley_id::PrincipalId::from_bytes(fill(2)),
        capability_summary_digest: sley_id::CapabilitySummaryDigest::from_bytes(fill(3)),
        operations: vec![MutationOperation {
            ordinal: 0,
            class: payload.class(),
            target_kind: 9,
            target_entity: target_id,
            field_tag: None,
            payload,
            precondition_ordinal: 0,
        }],
        preconditions: vec![BoundPrecondition {
            operation_ordinal: 0,
            requirement: precondition.requirement(),
            payload: precondition,
        }],
        validation_profile_id: sley_mutate::full_validation_profile_id()
            .map_err(|e| e.to_string())?,
        candidate_nonce: sley_id::CandidateNonce::from_bytes(fill(4)),
        expiry: CandidateExpiry::unix_millis(u64::MAX),
    };
    let candidate = sley_mutate::build_candidate(&record).map_err(|e| e.to_string())?;
    let proposed = sley_mutate::apply_candidate_to_snapshot(
        s.epoch,
        &candidate.record,
        &s.objects,
        &s.root.record.entry_points,
    )
    .map_err(|e| format!("apply: {e:?}"))?;
    let mut builder = sley_state_root::StateRootBuilder::new(
        s.root.record.workspace_id,
        s.root.record.contract_root,
        s.root.record.test_root,
        s.root.record.policy_root,
    );
    for entry in proposed.entry_points() {
        builder = builder.entry_point(*entry);
    }
    for object in proposed.entities() {
        builder = builder.entity_binding(object.record().entity_id, object.object_id());
    }
    let registry = sley_state_root::conformance_registry().map_err(|e| format!("{e:?}"))?;
    let root = builder.build(&registry).map_err(|e| format!("root: {e:?}"))?;
    let mutated = canonical::CanonicalS {
        epoch: s.epoch,
        objects: proposed.entities().to_vec(),
        root,
        entry_point: s.entry_point,
        entry_function: s.entry_function,
    };
    write_stage(&out.join("S"), &mutated)?;
    write(&out.join("candidate.scb"), &candidate.stored_bytes)?;
    println!(
        "{{\"candidate_id\": \"{}\", \"target_constant\": \"{}\", \"from\": {from}, \"to\": {to}, \"base_root\": \"{}\", \"state_root\": \"{}\"}}",
        hex(candidate.candidate_id.as_bytes()),
        hex(target_id.as_bytes()),
        hex(s.root.root.as_bytes()),
        hex(mutated.root.root.as_bytes())
    );
    Ok(())
}

/// `C0.build` of a staged S.
fn c0(stage: &Path, out: &Path) -> Result<(), String> {
    let s = load_stage(stage)?;
    let built = native::c0_build(&s, native::toolchain_limits())?;
    write(out, &built.envelope)?;
    println!("{{\"package_digest\": \"{}\"}}", hex(&built.digests.package_digest));
    Ok(())
}

/// A copy of a staged S whose first object's magic is corrupted.
fn corrupt(stage: &Path, out: &Path) -> Result<(), String> {
    let s = load_stage(stage)?;
    write_stage(out, &s)?;
    let first = s.objects.first().ok_or("no objects")?;
    let name = format!("{}.obj", hex(first.record().entity_id.as_bytes()));
    let mut bytes = first.stored_bytes().to_vec();
    bytes[0] ^= 0xff;
    write(&out.join("objects").join(name), &bytes)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [cmd, flag, dir] if cmd == "stage" && flag == "--out" => stage(&PathBuf::from(dir)),
        [cmd, pkg] if cmd == "inspect" => inspect(Path::new(pkg)),
        [cmd, a, b] if cmd == "compare" => compare(Path::new(a), Path::new(b)),
        [cmd, f1, stage_dir, f2, out, f3, from, f4, to]
            if cmd == "mutate-constant"
                && f1 == "--stage"
                && f2 == "--out"
                && f3 == "--from"
                && f4 == "--to" =>
        {
            match (from.parse(), to.parse()) {
                (Ok(from), Ok(to)) => mutate_constant(Path::new(stage_dir), Path::new(out), from, to),
                _ => Err("--from/--to must be u32".into()),
            }
        }
        [cmd, f1, stage_dir, f2, out] if cmd == "c0" && f1 == "--stage" && f2 == "--out" => {
            c0(Path::new(stage_dir), Path::new(out))
        }
        [cmd, f1, stage_dir, f2, out] if cmd == "corrupt" && f1 == "--stage" && f2 == "--out" => {
            corrupt(Path::new(stage_dir), Path::new(out))
        }
        _ => Err("usage: sley-sh2 stage --out DIR | inspect PKG | compare PKG PKG | mutate-constant --stage DIR --out DIR --from N --to N | c0 --stage DIR --out PKG | corrupt --stage DIR --out DIR".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("sley-sh2: {error}");
            ExitCode::FAILURE
        }
    }
}
