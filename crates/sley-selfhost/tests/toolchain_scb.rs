//! Differential tests: the Sley SCB walker against the native codec.
#![allow(clippy::too_many_lines)]

use sley_selfhost::native::{big_limits, execute};
use sley_selfhost::seed::{Program, t};
use sley_selfhost::toolchain;
use sley_ssmc::{
    BuiltinFailureKind, BuiltinFailureValue, ConstData, ConstValue, FieldConst, FunctionRefValue,
    IntegerWidth, MapEntryConst, MemberId, NamedType, RecordConst, ResultConst, TypeExpr,
    VariantConst,
};
use sley_vm::ExecutionTermination;
use std::collections::HashMap;
use std::sync::OnceLock;

fn program() -> &'static (Program, HashMap<String, sley_id::EntityId>) {
    static P: OnceLock<(Program, HashMap<String, sley_id::EntityId>)> = OnceLock::new();
    P.get_or_init(|| toolchain::module().finish())
}

fn sley_check(bytes: &[u8]) -> Result<(), u32> {
    let (program, names) = program();
    let input = ConstValue {
        value_type: TypeExpr::Bytes,
        data: ConstData::Bytes(bytes.to_vec()),
    };
    let outcome =
        execute(program, names["scb_const_exact"], vec![input], big_limits()).expect("executes");
    match outcome.termination {
        ExecutionTermination::Success(value) => match value.data {
            ConstData::Result(ResultConst::Ok(_)) => Ok(()),
            ConstData::Result(ResultConst::Err(code)) => match code.data {
                ConstData::UInt(code) => Err(u32::try_from(code).unwrap()),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        },
        other => panic!("{other:?} on {bytes:02x?}"),
    }
}

fn native_check(bytes: &[u8]) -> Result<(), u32> {
    sley_mutate::decode_const_value(bytes)
        .map(|_| ())
        .map_err(|e| toolchain::codes::scb_code(e.code()))
}

fn u(width: u16, value: u128) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::UInt(IntegerWidth::from_bits(width)),
        data: ConstData::UInt(value),
    }
}

fn corpus() -> Vec<ConstValue> {
    let id = sley_id::EntityId::from_bytes([7; 32]);
    let named = TypeExpr::Named(NamedType {
        definition: id,
        arguments: vec![TypeExpr::Bool, t::vec(t::u8())],
    });
    let mut values = vec![
        ConstValue {
            value_type: TypeExpr::Unit,
            data: ConstData::Unit,
        },
        ConstValue {
            value_type: TypeExpr::Bool,
            data: ConstData::Bool(true),
        },
        u(8, 255),
        u(64, u128::from(u64::MAX)),
        u(128, u128::MAX),
        ConstValue {
            value_type: t::i64(),
            data: ConstData::SInt(-12345),
        },
        ConstValue {
            value_type: TypeExpr::SInt(IntegerWidth::from_bits(128)),
            data: ConstData::SInt(i128::MIN),
        },
        ConstValue {
            value_type: TypeExpr::F32,
            data: ConstData::F32Bits(0x7fc0_0000),
        },
        ConstValue {
            value_type: TypeExpr::F64,
            data: ConstData::F64Bits(0x3ff0_0000_0000_0000),
        },
        ConstValue {
            value_type: TypeExpr::Bytes,
            data: ConstData::Bytes(vec![0, 1, 2, 255]),
        },
        ConstValue {
            value_type: TypeExpr::Text,
            data: ConstData::Text("héllo ☃ 𝄞".to_owned()),
        },
        ConstValue {
            value_type: t::vec(t::u64()),
            data: ConstData::Sequence(vec![u(64, 1), u(64, 300)]),
        },
        ConstValue {
            value_type: named.clone(),
            data: ConstData::Record(RecordConst {
                definition: id,
                fields: vec![FieldConst {
                    member_id: MemberId::from_bytes([3; 32]),
                    value: u(8, 1),
                }],
            }),
        },
        ConstValue {
            value_type: named,
            data: ConstData::Variant(VariantConst {
                definition: id,
                member_id: MemberId::from_bytes([4; 32]),
                payload: Some(Box::new(u(16, 9))),
            }),
        },
        ConstValue {
            value_type: t::map(t::u64(), TypeExpr::Bool),
            data: ConstData::Map(vec![
                MapEntryConst {
                    key: u(64, 1),
                    value: ConstValue {
                        value_type: TypeExpr::Bool,
                        data: ConstData::Bool(false),
                    },
                },
                MapEntryConst {
                    key: u(64, 200),
                    value: ConstValue {
                        value_type: TypeExpr::Bool,
                        data: ConstData::Bool(true),
                    },
                },
            ]),
        },
        ConstValue {
            value_type: t::opt(t::u8()),
            data: ConstData::Option(Some(Box::new(u(8, 5)))),
        },
        ConstValue {
            value_type: t::opt(t::u8()),
            data: ConstData::Option(None),
        },
        ConstValue {
            value_type: t::res(t::u8(), TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)),
            data: ConstData::Result(ResultConst::Err(Box::new(ConstValue {
                value_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
                data: ConstData::BuiltinFailure(BuiltinFailureValue {
                    kind: BuiltinFailureKind::Index,
                    code: 1,
                }),
            }))),
        },
        ConstValue {
            value_type: TypeExpr::FunctionRef(sley_ssmc::FunctionType {
                parameters: vec![TypeExpr::Bool],
                result: Box::new(TypeExpr::Unit),
                effects: vec![id],
            }),
            data: ConstData::FunctionRef(FunctionRefValue {
                function: id,
                type_arguments: vec![t::tuple(vec![TypeExpr::Text, TypeExpr::TypeParameter(3)])],
            }),
        },
    ];
    // Deep nesting near the limits.
    let mut deep = TypeExpr::Bool;
    for _ in 0..30 {
        deep = t::opt(deep);
    }
    values.push(ConstValue {
        value_type: deep,
        data: ConstData::Unit,
    });
    values
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn walker_accepts_and_refuses_like_native() {
    let mut checked = 0;
    for value in corpus() {
        let bytes = sley_mutate::encode_const_value(&value).expect("encodes");
        assert_eq!(sley_check(&bytes), Ok(()), "{value:?}");
        // Truncations.
        for cut in 0..bytes.len() {
            let mutated = &bytes[..cut];
            assert_eq!(
                sley_check(mutated),
                native_check(mutated),
                "cut {cut} of {value:?}"
            );
            checked += 1;
        }
        // Single-byte mutations.
        for position in 0..bytes.len() {
            for replacement in [0x00_u8, 0x01, 0x7f, 0x80, 0xff, bytes[position] ^ 0x01] {
                let mut mutated = bytes.clone();
                mutated[position] = replacement;
                let native = native_check(&mutated);
                let sley = sley_check(&mutated);
                assert_eq!(
                    sley.is_ok(),
                    native.is_ok(),
                    "pos {position} -> {replacement:02x} of {value:?}: sley {sley:?} native {native:?}"
                );
                if sley != native {
                    eprintln!(
                        "code deviation: pos {position} -> {replacement:02x}: sley {sley:?} native {native:?}"
                    );
                }
                checked += 1;
            }
        }
        // Trailing byte.
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(sley_check(&longer), native_check(&longer));
    }
    eprintln!("checked {checked} mutated encodings");
}
