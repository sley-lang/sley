//! Differential tests: Sley entity-object decoding against the native codec.
#![allow(clippy::too_many_lines)]

use sley_id::EntityId;
use sley_mutate::value::EntityBodyValue;
use sley_selfhost::canonical;
use sley_selfhost::native::{big_limits, execute};
use sley_selfhost::seed::{Program, member_id, t, type_id};
use sley_selfhost::toolchain::{self, TOOLCHAIN_SEED};
use sley_ssmc::{
    CaseKey, ConstData, ConstValue, FieldConst, Immediate, NamedType, RecordConst, ResultConst,
    SwitchArgument, Terminator, TypeDefForm, TypeExpr, ValueRef, VariantConst,
};
use sley_vm::ExecutionTermination;
use std::collections::HashMap;
use std::sync::OnceLock;

fn program() -> &'static (Program, HashMap<String, EntityId>) {
    static P: OnceLock<(Program, HashMap<String, EntityId>)> = OnceLock::new();
    P.get_or_init(|| toolchain::module().finish())
}

fn named(name: &str) -> TypeExpr {
    TypeExpr::Named(NamedType {
        definition: type_id(TOOLCHAIN_SEED, name),
        arguments: Vec::new(),
    })
}

fn record(name: &str, fields: Vec<(&str, ConstValue)>) -> ConstValue {
    ConstValue {
        value_type: named(name),
        data: ConstData::Record(RecordConst {
            definition: type_id(TOOLCHAIN_SEED, name),
            fields: fields
                .into_iter()
                .map(|(field, value)| FieldConst {
                    member_id: member_id(TOOLCHAIN_SEED, name, field),
                    value,
                })
                .collect(),
        }),
    }
}

fn variant(name: &str, case: &str, payload: Option<ConstValue>) -> ConstValue {
    ConstValue {
        value_type: named(name),
        data: ConstData::Variant(VariantConst {
            definition: type_id(TOOLCHAIN_SEED, name),
            member_id: member_id(TOOLCHAIN_SEED, name, case),
            payload: payload.map(Box::new),
        }),
    }
}

fn bytes(value: &[u8]) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bytes,
        data: ConstData::Bytes(value.to_vec()),
    }
}

fn u64v(value: u64) -> ConstValue {
    ConstValue {
        value_type: t::u64(),
        data: ConstData::UInt(u128::from(value)),
    }
}

fn boolv(value: bool) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    }
}

fn vec_of(element: TypeExpr, items: Vec<ConstValue>) -> ConstValue {
    ConstValue {
        value_type: t::vec(element),
        data: ConstData::Sequence(items),
    }
}

fn ids(values: &[EntityId]) -> ConstValue {
    vec_of(
        TypeExpr::Bytes,
        values.iter().map(|id| bytes(id.as_bytes())).collect(),
    )
}

/// Canonical SCB bytes of one `TypeExpr` (field 1 of a `ConstValue`).
fn type_bytes(ty: &TypeExpr) -> Vec<u8> {
    let encoded = sley_mutate::encode_const_value(&ConstValue {
        value_type: ty.clone(),
        data: ConstData::Unit,
    })
    .expect("encodes");
    let mut cursor = sley_scb1::ScbValueCursor::new(&encoded).unwrap();
    cursor.read_record_field_count().unwrap();
    cursor.read_uvar(32).unwrap();
    cursor.read_sized_payload().unwrap().to_vec()
}

fn types(values: &[TypeExpr]) -> ConstValue {
    vec_of(
        TypeExpr::Bytes,
        values.iter().map(|ty| bytes(&type_bytes(ty))).collect(),
    )
}

fn vref(value: &ValueRef) -> ConstValue {
    match value {
        ValueRef::Parameter(id) => record(
            "VRef",
            vec![
                ("kind", u64v(1)),
                ("id", bytes(id.as_bytes())),
                ("idx", u64v(0)),
            ],
        ),
        ValueRef::OperationResult(r) => record(
            "VRef",
            vec![
                ("kind", u64v(2)),
                ("id", bytes(r.operation.as_bytes())),
                ("idx", u64v(u64::from(r.result_index))),
            ],
        ),
    }
}

fn empty_vref() -> ConstValue {
    record(
        "VRef",
        vec![("kind", u64v(0)), ("id", bytes(&[])), ("idx", u64v(0))],
    )
}

fn edge(target: &EntityId, args: &[ValueRef]) -> ConstValue {
    record(
        "Edge",
        vec![
            ("target", bytes(target.as_bytes())),
            (
                "args",
                vec_of(named("VRef"), args.iter().map(vref).collect()),
            ),
        ],
    )
}

fn term(value: &Terminator) -> ConstValue {
    let (kind, val, has_val, code, edges, cases) = match value {
        Terminator::Return(r) => (1, vref(&r.value), true, 0, vec![], vec![]),
        Terminator::Branch(b) => (
            2,
            empty_vref(),
            false,
            0,
            vec![edge(&b.edge.target, &b.edge.arguments)],
            vec![],
        ),
        Terminator::CondBranch(c) => (
            3,
            vref(&c.condition),
            true,
            0,
            vec![
                edge(&c.if_true.target, &c.if_true.arguments),
                edge(&c.if_false.target, &c.if_false.arguments),
            ],
            vec![],
        ),
        Terminator::VariantSwitch(sw) => (
            4,
            vref(&sw.value),
            true,
            0,
            vec![],
            sw.cases
                .iter()
                .map(|case| {
                    let (member, key, builtin) = match case.case_key {
                        CaseKey::Member(m) => (true, m.as_bytes().to_vec(), 0),
                        CaseKey::Builtin(b) => (false, vec![], u64::from(b.tag())),
                    };
                    record(
                        "SCase",
                        vec![
                            ("member", boolv(member)),
                            ("key", bytes(&key)),
                            ("builtin", u64v(builtin)),
                            ("target", bytes(case.edge.target.as_bytes())),
                            (
                                "args",
                                vec_of(
                                    named("SArg"),
                                    case.edge
                                        .arguments
                                        .iter()
                                        .map(|a| match a {
                                            SwitchArgument::Value(v) => record(
                                                "SArg",
                                                vec![("pay", boolv(false)), ("val", vref(v))],
                                            ),
                                            SwitchArgument::CasePayload => record(
                                                "SArg",
                                                vec![("pay", boolv(true)), ("val", empty_vref())],
                                            ),
                                        })
                                        .collect(),
                                ),
                            ),
                        ],
                    )
                })
                .collect(),
        ),
        Terminator::Trap(trap) => (
            5,
            trap.payload.as_ref().map_or_else(empty_vref, vref),
            trap.payload.is_some(),
            u64::from(trap.code.tag()),
            vec![],
            vec![],
        ),
    };
    record(
        "Term",
        vec![
            ("kind", u64v(kind)),
            ("val", val),
            ("has_val", boolv(has_val)),
            ("code", u64v(code)),
            ("edges", vec_of(named("Edge"), edges)),
            ("cases", vec_of(named("SCase"), cases)),
        ],
    )
}

fn imm(value: &Immediate) -> ConstValue {
    let (tag, id, member, index, targs): (u64, Vec<u8>, Vec<u8>, u64, Vec<TypeExpr>) = match value {
        Immediate::None => (1, vec![], vec![], 0, vec![]),
        Immediate::Entity(id) => (2, id.as_bytes().to_vec(), vec![], 0, vec![]),
        Immediate::Index(i) => (3, vec![], vec![], u64::from(*i), vec![]),
        Immediate::Field(m) => (4, m.as_bytes().to_vec(), vec![], 0, vec![]),
        Immediate::Variant(v) => (
            5,
            v.definition.as_bytes().to_vec(),
            v.member_id.as_bytes().to_vec(),
            0,
            vec![],
        ),
        Immediate::Observation(o) => (6, o.to_vec(), vec![], 0, vec![]),
        Immediate::Function(f) => (
            7,
            f.function.as_bytes().to_vec(),
            vec![],
            0,
            f.type_arguments.clone(),
        ),
    };
    record(
        "Imm",
        vec![
            ("tag", u64v(tag)),
            ("id", bytes(&id)),
            ("member", bytes(&member)),
            ("index", u64v(index)),
            ("targs", types(&targs)),
        ],
    )
}

fn tparams(values: &[sley_ssmc::TypeParameterDef]) -> ConstValue {
    vec_of(
        t::u64(),
        values.iter().map(|p| u64v(u64::from(p.ordinal))).collect(),
    )
}

fn expected(object: &sley_mutate::EntityObject) -> ConstValue {
    let record_ = object.record();
    let id = record_.entity_id;
    let idb = || ("id", bytes(id.as_bytes()));
    let (kind, ent) = match &record_.body {
        EntityBodyValue::Workspace(_) => (1, variant("Ent", "W", None)),
        EntityBodyValue::Package(_) => (2, variant("Ent", "K", None)),
        EntityBodyValue::Namespace(_) => (3, variant("Ent", "N", None)),
        EntityBodyValue::TypeDef(d) => {
            let (is_rec, members) = match &d.form {
                TypeDefForm::Record(fields) => (
                    true,
                    fields
                        .iter()
                        .map(|f| {
                            record(
                                "Mem",
                                vec![
                                    ("member", bytes(f.member_id.as_bytes())),
                                    ("has_ty", boolv(true)),
                                    ("ty", bytes(&type_bytes(&f.value_type))),
                                    ("vis", u64v(u64::from(f.visibility.tag()))),
                                ],
                            )
                        })
                        .collect(),
                ),
                TypeDefForm::Variant(cases) => (
                    false,
                    cases
                        .iter()
                        .map(|c| {
                            record(
                                "Mem",
                                vec![
                                    ("member", bytes(c.member_id.as_bytes())),
                                    ("has_ty", boolv(c.payload_type.is_some())),
                                    (
                                        "ty",
                                        bytes(
                                            &c.payload_type
                                                .as_ref()
                                                .map(type_bytes)
                                                .unwrap_or_default(),
                                        ),
                                    ),
                                    ("vis", u64v(0)),
                                ],
                            )
                        })
                        .collect(),
                ),
            };
            (
                4,
                variant(
                    "Ent",
                    "T",
                    Some(record(
                        "TDef",
                        vec![
                            idb(),
                            ("tparams", tparams(&d.type_parameters)),
                            ("is_rec", boolv(is_rec)),
                            ("members", vec_of(named("Mem"), members)),
                            ("invariants", ids(d.invariants.as_slice())),
                            ("vis", u64v(u64::from(d.visibility.tag()))),
                        ],
                    )),
                ),
            )
        }
        EntityBodyValue::Function(f) => (
            5,
            variant(
                "Ent",
                "F",
                Some(record(
                    "Fun",
                    vec![
                        idb(),
                        ("tparams", tparams(&f.type_parameters)),
                        ("params", ids(&f.parameters)),
                        ("result", bytes(&type_bytes(&f.result_type))),
                        ("effects", ids(f.effects.as_slice())),
                        ("entry", bytes(f.entry_block.as_bytes())),
                        ("blocks", ids(&f.blocks)),
                        ("contracts", ids(f.contracts.as_slice())),
                        ("vis", u64v(u64::from(f.visibility.tag()))),
                    ],
                )),
            ),
        ),
        EntityBodyValue::Parameter(p) => (
            6,
            variant(
                "Ent",
                "P",
                Some(record(
                    "Prm",
                    vec![
                        idb(),
                        ("owner", bytes(p.owner.as_bytes())),
                        ("role", u64v(u64::from(p.role.tag()))),
                        ("ordinal", u64v(u64::from(p.ordinal))),
                        ("ty", bytes(&type_bytes(&p.value_type))),
                    ],
                )),
            ),
        ),
        EntityBodyValue::Block(b) => (
            7,
            variant(
                "Ent",
                "B",
                Some(record(
                    "Blk",
                    vec![
                        idb(),
                        ("function", bytes(b.function.as_bytes())),
                        ("params", ids(&b.parameters)),
                        ("ops", ids(&b.operations)),
                        ("term", term(&b.terminator)),
                        ("reach", u64v(u64::from(b.reachability.tag()))),
                    ],
                )),
            ),
        ),
        EntityBodyValue::Operation(o) => (
            8,
            variant(
                "Ent",
                "O",
                Some(record(
                    "Op",
                    vec![
                        idb(),
                        ("block", bytes(o.block.as_bytes())),
                        ("ordinal", u64v(u64::from(o.ordinal))),
                        ("opcode", u64v(u64::from(o.opcode))),
                        (
                            "operands",
                            vec_of(named("VRef"), o.operands.iter().map(vref).collect()),
                        ),
                        ("results", types(&o.result_types)),
                        ("imm", imm(&o.immediate)),
                    ],
                )),
            ),
        ),
        EntityBodyValue::Constant(c) => {
            let value = sley_mutate::encode_const_value(&c.value).unwrap();
            (
                9,
                variant(
                    "Ent",
                    "C",
                    Some(record(
                        "Cst",
                        vec![
                            idb(),
                            ("value", bytes(&value)),
                            ("ty", bytes(&type_bytes(&c.value.value_type))),
                        ],
                    )),
                ),
            )
        }
        EntityBodyValue::AdapterImport(i) => (
            15,
            variant(
                "Ent",
                "I",
                Some(record(
                    "Imp",
                    vec![
                        idb(),
                        ("adapter", bytes(&i.adapter_id)),
                        ("abi", u64v(u64::from(i.abi_version))),
                        ("req", bytes(&type_bytes(&i.request_type))),
                        ("resp", bytes(&type_bytes(&i.response_type))),
                        ("fail", bytes(&type_bytes(&i.failure_type))),
                        ("effects", ids(i.effects.as_slice())),
                    ],
                )),
            ),
        ),
        EntityBodyValue::EntryPoint(e) => (
            16,
            variant(
                "Ent",
                "E",
                Some(record(
                    "EPt",
                    vec![
                        idb(),
                        ("function", bytes(e.function.as_bytes())),
                        ("exposure", u64v(u64::from(e.exposure.tag()))),
                    ],
                )),
            ),
        ),
        other => panic!("unexpected kind {other:?}"),
    };
    record(
        "Obj",
        vec![
            ("id", bytes(id.as_bytes())),
            ("oid", bytes(object.object_id().as_bytes())),
            ("kind", u64v(kind)),
            ("ent", ent),
        ],
    )
}

fn sley_decode(object: &[u8], epoch: &[u8]) -> Result<ConstValue, u32> {
    let (program, names) = program();
    let outcome = execute(
        program,
        names["cd_object"],
        vec![bytes(object), bytes(epoch)],
        big_limits(),
    )
    .expect("executes");
    match outcome.termination {
        ExecutionTermination::Success(value) => match value.data {
            ConstData::Result(ResultConst::Ok(inner)) => Ok(*inner),
            ConstData::Result(ResultConst::Err(code)) => match code.data {
                ConstData::UInt(code) => Err(u32::try_from(code).unwrap()),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        },
        other => panic!("{other:?}"),
    }
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn decodes_every_toolchain_object_exactly() {
    let (program, names) = program();
    let s = canonical::build(program, names["cd_object"], "codec-test");
    let epoch = s.epoch.as_bytes().to_vec();
    let mut by_kind: HashMap<u64, usize> = HashMap::new();
    // Every object of one or two of each kind, plus a stride sample.
    for (index, object) in s.objects.iter().enumerate() {
        let want = expected(object);
        let kind = match &want.data {
            ConstData::Record(r) => match r.fields[2].value.data {
                ConstData::UInt(k) => u64::try_from(k).unwrap(),
                _ => 0,
            },
            _ => 0,
        };
        let seen = by_kind.entry(kind).or_default();
        *seen += 1;
        if *seen > 25 && index % 97 != 0 {
            continue;
        }
        let got = sley_decode(object.stored_bytes(), &epoch).expect("decodes");
        assert_eq!(got, want, "object {index} kind {kind}");
    }
    eprintln!("objects {} by kind {by_kind:?}", s.objects.len());
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn refuses_corrupted_objects_like_native() {
    let (program, names) = program();
    let s = canonical::build(program, names["cd_object"], "codec-test");
    let epoch = s.epoch;
    let mut checked = 0;
    let mut deviations = 0;
    for object in s.objects.iter().step_by(211).take(40) {
        let stored = object.stored_bytes().to_vec();
        // Re-digest after each mutation so payload errors are reached.
        let redigest = |mut bytes: Vec<u8>| {
            let n = bytes.len();
            let digest = blake3::Hasher::new()
                .update(b"sley2.object.v1")
                .update(&bytes[..n - 32])
                .finalize();
            bytes[n - 32..].copy_from_slice(digest.as_bytes());
            bytes
        };
        for position in (0..stored.len() - 32).step_by(3) {
            for replacement in [0x00_u8, 0x01, 0x80, stored[position] ^ 0x02] {
                let mut mutated = stored.clone();
                mutated[position] = replacement;
                let mutated = redigest(mutated);
                let native = sley_mutate::import_entity_object(epoch, &mutated)
                    .map(|_| ())
                    .map_err(|e| toolchain::codes::scb_code(e.code()));
                let sley = sley_decode(&mutated, epoch.as_bytes()).map(|_| ());
                // The toolchain refuses kinds outside its domain.
                if native.is_ok() && sley == Err(toolchain::codes::TOOL_KIND_OUT_OF_DOMAIN) {
                    continue;
                }
                assert_eq!(
                    sley.is_ok(),
                    native.is_ok(),
                    "pos {position} -> {replacement:02x}: sley {sley:?} native {native:?}"
                );
                if sley != native {
                    deviations += 1;
                }
                checked += 1;
            }
        }
    }
    eprintln!("checked {checked} corrupted objects, {deviations} code deviations");
}
