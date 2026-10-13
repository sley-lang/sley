//! `sley-host`: the pinned native execution host for SH2.
//!
//! The host runs one approved `EXEC_PACKAGE_V3` toolchain package on staged
//! build inputs and writes the output bytes. It contains no compiler
//! service: it hydrates the package structurally, admits it only when its
//! package identity is in the pinned set given on the command line, and
//! executes it. Staging reads files; it never compiles, patches, or repairs
//! anything.
//!
//! ```text
//! sley-host build --package PKG --trust HEX[,HEX...] --stage DIR --out FILE [--receipt FILE]
//! ```
//!
//! `DIR` holds `epoch.bin` (32 bytes), `root.bin` (the stored state root),
//! and `objects/` (one stored entity object per file; file names sort in
//! entity-identity order). The toolchain entry takes `(objects, root,
//! epoch, limits)` and returns `Result<Bytes, UInt(32)>`; the limits passed
//! are the package's admitted limits.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sha2::{Digest as _, Sha256};
use sley_ssmc::{ConstData, ConstValue, IntegerWidth, ResultConst, TypeExpr};
use sley_vm::ExecutionTermination;

struct Args {
    package: PathBuf,
    trust: Vec<[u8; 32]>,
    stage: PathBuf,
    out: PathBuf,
    receipt: Option<PathBuf>,
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn unhex32(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64 {
        return Err(format!("trust digest must be 64 hex digits: {text}"));
    }
    let mut out = [0_u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16)
            .map_err(|_| format!("bad hex digest: {text}"))?;
    }
    Ok(out)
}

fn parse_args() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("build") {
        return Err("usage: sley-host build --package PKG --trust HEX[,HEX...] --stage DIR --out FILE [--receipt FILE]".into());
    }
    let (mut package, mut trust, mut stage, mut out, mut receipt) =
        (None, Vec::new(), None, None, None);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--package" => package = Some(PathBuf::from(value)),
            "--trust" => {
                for digest in value.split(',') {
                    trust.push(unhex32(digest)?);
                }
            }
            "--stage" => stage = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--receipt" => receipt = Some(PathBuf::from(value)),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(Args {
        package: package.ok_or("--package is required")?,
        trust,
        stage: stage.ok_or("--stage is required")?,
        out: out.ok_or("--out is required")?,
        receipt,
    })
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn bytes(value: Vec<u8>) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bytes,
        data: ConstData::Bytes(value),
    }
}

fn uint64(value: u64) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::UInt(IntegerWidth::from_bits(64)),
        data: ConstData::UInt(u128::from(value)),
    }
}

#[allow(clippy::too_many_lines)] // one linear construction; splitting it hides the order
fn run(args: &Args) -> Result<String, String> {
    let envelope = read(&args.package)?;
    let hydrated = sley_vm::hydrate_package_envelope_v3(&envelope)
        .map_err(|e| format!("package hydration refused: {e:?}"))?;
    let package = hydrated.package;
    let (digests, approved) =
        sley_vm::host_admission::admit_pinned_v3_package(&package, &args.trust)
            .map_err(|e| format!("admission refused: {e}"))?;
    // Stage the build request.
    let epoch = read(&args.stage.join("epoch.bin"))?;
    let root = read(&args.stage.join("root.bin"))?;
    let mut names: Vec<PathBuf> = std::fs::read_dir(args.stage.join("objects"))
        .map_err(|e| format!("objects: {e}"))?
        .map(|entry| entry.map(|e| e.path()).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    names.sort();
    let mut input_digest = Sha256::new();
    let mut objects = Vec::with_capacity(names.len());
    for name in &names {
        let object = read(name)?;
        input_digest.update((object.len() as u64).to_be_bytes());
        input_digest.update(&object);
        objects.push(bytes(object));
    }
    input_digest.update(&root);
    input_digest.update(&epoch);
    let limits = package.admitted_limits;
    let limit_tuple = ConstValue {
        value_type: TypeExpr::Tuple(vec![
            TypeExpr::UInt(IntegerWidth::from_bits(64)),
            TypeExpr::UInt(IntegerWidth::from_bits(64)),
            TypeExpr::UInt(IntegerWidth::from_bits(64)),
            TypeExpr::UInt(IntegerWidth::from_bits(64)),
            TypeExpr::Bool,
            TypeExpr::UInt(IntegerWidth::from_bits(64)),
        ]),
        data: ConstData::Sequence(vec![
            uint64(limits.max_instructions),
            uint64(limits.max_fuel),
            uint64(limits.max_value_units),
            uint64(limits.max_output_units),
            ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(limits.cancel_at_fuel.is_some()),
            },
            uint64(limits.cancel_at_fuel.unwrap_or(0)),
        ]),
    };
    let inputs = vec![
        ConstValue {
            value_type: TypeExpr::Vector(Box::new(TypeExpr::Bytes)),
            data: ConstData::Sequence(objects),
        },
        bytes(root),
        bytes(epoch),
        limit_tuple,
    ];
    let started = std::time::Instant::now();
    let outcome = sley_vm::execute_approved_package_v3(
        &package,
        &approved,
        sley_vm::ExecutionRequest { inputs, limits },
    )
    .map_err(|e| format!("execution refused: {e:?}"))?;
    let elapsed = started.elapsed();
    let (status, output) = match &outcome.termination {
        ExecutionTermination::Success(value) => match &value.data {
            ConstData::Result(ResultConst::Ok(inner)) => match &inner.data {
                ConstData::Bytes(out) => ("ok".to_owned(), Some(out.clone())),
                _ => ("malformed-result".to_owned(), None),
            },
            ConstData::Result(ResultConst::Err(code)) => match code.data {
                ConstData::UInt(code) => (format!("toolchain-error-{code}"), None),
                _ => ("malformed-error".to_owned(), None),
            },
            _ => ("malformed-result".to_owned(), None),
        },
        other => (format!("terminated: {other:?}"), None),
    };
    let output_digest = output.as_ref().map(|out| hex(&Sha256::digest(out)));
    if let Some(out) = &output {
        std::fs::write(&args.out, out).map_err(|e| format!("{}: {e}", args.out.display()))?;
    }
    let receipt = format!(
        "{{\n  \"schema\": \"sley2.sh2.host-build-receipt.v1\",\n  \"toolchain_package_digest\": \"{}\",\n  \"input_digest\": \"{}\",\n  \"objects\": {},\n  \"status\": \"{}\",\n  \"output_sha256\": {},\n  \"observation_id\": \"{}\",\n  \"instructions\": {},\n  \"fuel\": {},\n  \"peak_value_units\": {},\n  \"elapsed_ms\": {}\n}}\n",
        hex(&digests.package_digest),
        hex(&input_digest.finalize()),
        names.len(),
        status,
        output_digest.map_or_else(|| "null".to_owned(), |d| format!("\"{d}\"")),
        hex(outcome.observation_id.as_bytes()),
        outcome.instruction_count,
        outcome.fuel_used,
        outcome.peak_value_units,
        elapsed.as_millis(),
    );
    if let Some(path) = &args.receipt {
        std::fs::write(path, &receipt).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    if status == "ok" {
        Ok(receipt)
    } else {
        Err(format!("build failed: {status}\n{receipt}"))
    }
}

fn main() -> ExitCode {
    match parse_args().and_then(|args| run(&args)) {
        Ok(receipt) => {
            print!("{receipt}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("sley-host: {error}");
            ExitCode::FAILURE
        }
    }
}
