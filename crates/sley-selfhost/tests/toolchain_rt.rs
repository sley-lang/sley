//! Runtime-library parity tests against the native SCB1 primitives.

use sley_selfhost::native::{big_limits, execute};
use sley_selfhost::seed::{Program, t};
use sley_selfhost::toolchain;
use sley_ssmc::{ConstData, ConstValue, ResultConst};
use sley_vm::ExecutionTermination;
use std::collections::HashMap;
use std::sync::OnceLock;

fn program() -> &'static (Program, HashMap<String, sley_id::EntityId>) {
    static P: OnceLock<(Program, HashMap<String, sley_id::EntityId>)> = OnceLock::new();
    P.get_or_init(|| toolchain::module().finish())
}

fn run(entry: &str, inputs: Vec<ConstValue>) -> ConstValue {
    let (program, names) = program();
    let outcome = execute(program, names[entry], inputs, big_limits()).expect("executes");
    match outcome.termination {
        ExecutionTermination::Success(value) => value,
        other => panic!("{entry}: {other:?}"),
    }
}

fn u8vec(bytes: &[u8]) -> ConstValue {
    ConstValue {
        value_type: t::vec(t::u8()),
        data: ConstData::Sequence(
            bytes
                .iter()
                .map(|b| ConstValue {
                    value_type: t::u8(),
                    data: ConstData::UInt(u128::from(*b)),
                })
                .collect(),
        ),
    }
}

fn u64v(x: u64) -> ConstValue {
    ConstValue {
        value_type: t::u64(),
        data: ConstData::UInt(u128::from(x)),
    }
}

fn u32v(x: u32) -> ConstValue {
    ConstValue {
        value_type: t::u32(),
        data: ConstData::UInt(u128::from(x)),
    }
}

/// Decodes `(value, pos)` or the numeric error.
fn reader_result(value: &ConstValue) -> Result<(u128, u128), u128> {
    match &value.data {
        ConstData::Result(ResultConst::Ok(inner)) => match &inner.data {
            ConstData::Sequence(items) => match (&items[0].data, &items[1].data) {
                (ConstData::UInt(a), ConstData::UInt(b)) => Ok((*a, *b)),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        },
        ConstData::Result(ResultConst::Err(inner)) => match inner.data {
            ConstData::UInt(code) => Err(code),
            _ => unreachable!(),
        },
        _ => unreachable!(),
    }
}

#[test]
fn uvar_reader_matches_native() {
    let cases: Vec<(Vec<u8>, u8)> = vec![
        (vec![0x00], 64),
        (vec![0x7f], 64),
        (vec![0x80, 0x01], 64),
        (vec![0xac, 0x02], 64),
        (vec![0x80, 0x00], 64),
        (vec![0xff, 0xff, 0xff, 0xff, 0x0f], 32),
        (vec![0xff, 0xff, 0xff, 0xff, 0x1f], 32),
        (vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01], 64),
        (vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02], 64),
        (vec![0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01], 64),
        (vec![0x80], 64),
        (vec![], 64),
        (vec![0xff, 0x01], 8),
        (vec![0x80, 0x02], 8),
    ];
    for (bytes, width) in cases {
        let native = sley_scb1::ScbValueCursor::new(&bytes)
            .and_then(|mut cursor| {
                let value = cursor.read_uvar(width)?;
                Ok((u128::from(value), cursor.position() as u128))
            })
            .map_err(|e| u128::from(toolchain::codes::scb_code(e.code())));
        let sley = reader_result(&run(
            "rt_rd_uvar",
            vec![
                u8vec(&bytes),
                u64v(0),
                u64v(bytes.len() as u64),
                u32v(u32::from(width)),
            ],
        ));
        assert_eq!(sley, native, "{bytes:02x?} width {width}");
    }
}

#[test]
fn writers_match_native() {
    for x in [0_u64, 1, 127, 128, 300, 16_383, 16_384, u64::from(u32::MAX), u64::MAX] {
        let out = run("uvarb", vec![u64v(x)]);
        assert_eq!(out.data, ConstData::Bytes(sley_scb1::encode_uvar(x)), "{x}");
        let be = run("be64", vec![u64v(x)]);
        assert_eq!(be.data, ConstData::Bytes(x.to_be_bytes().to_vec()), "{x}");
    }
    let be = run("be32", vec![u64v(0x0102_0304)]);
    assert_eq!(be.data, ConstData::Bytes(vec![1, 2, 3, 4]));
}
