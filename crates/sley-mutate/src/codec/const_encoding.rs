//! One borrowed wire schema for constant sizing, encoding and encoded-key order.
//!
//! Measurement validates the complete canonical value before applying a caller
//! cap. It borrows payloads and uses fixed scratch for integer encodings and
//! depth-bounded cursor stacks. It is not an O(cap) work guarantee. Cursors
//! remeasure child lengths without rechecking already-validated map order, so
//! nested keys do not recursively repeat canonical-order validation.

use super::{
    BuiltinFailureValue, ConstData, ConstValue, ConstValueByteMeasure, EntityId, FieldConst,
    FunctionRefValue, FunctionType, MAX_NESTING_DEPTH, MAX_STANDALONE_BYTES, MapEntryConst,
    NamedType, RecordConst, Result, ResultConst, ScbError, ScbErrorCode, TypeExpr, VariantConst,
    check_container_depth, check_depth, encode_f32_bits, encode_f64_bits,
    validate_entity_id_set_order,
};
use core::cmp::Ordering;
use sley_scb1::{
    MAX_BYTE_PAYLOAD, MAX_COLLECTION_ELEMENTS, encode_sint128_into, encode_uvar128_into,
};

#[derive(Clone, Copy)]
pub(super) enum Node<'a> {
    Value(&'a ConstValue),
    Data(&'a ConstData),
    Type(&'a TypeExpr),
    Named(&'a NamedType),
    Function(&'a FunctionType),
    FunctionRef(&'a FunctionRefValue),
    Failure(&'a BuiltinFailureValue),
    Field(&'a FieldConst),
    Record(&'a RecordConst),
    Variant(&'a VariantConst),
    Entry(&'a MapEntryConst),
    Result(&'a ResultConst),
    TypePair(&'a TypeExpr, &'a TypeExpr),
    Option(Option<&'a ConstValue>),
    Sequence(Sequence<'a>),
    UInt(u128),
    SInt(i128),
    Bool(bool),
    F32(u32),
    F64(u64),
    Raw(&'a [u8]),
    Bytes(&'a [u8]),
}

#[derive(Clone, Copy)]
pub(super) enum Sequence<'a> {
    Types(&'a [TypeExpr]),
    Values(&'a [ConstValue]),
    Fields(&'a [FieldConst]),
    Entries(&'a [MapEntryConst]),
    Ids(&'a [EntityId]),
}

impl<'a> Sequence<'a> {
    fn len(self) -> usize {
        match self {
            Self::Types(v) => v.len(),
            Self::Values(v) => v.len(),
            Self::Fields(v) => v.len(),
            Self::Entries(v) => v.len(),
            Self::Ids(v) => v.len(),
        }
    }

    fn get(self, index: usize) -> Node<'a> {
        match self {
            Self::Types(v) => Node::Type(&v[index]),
            Self::Values(v) => Node::Value(&v[index]),
            Self::Fields(v) => Node::Field(&v[index]),
            Self::Entries(v) => Node::Entry(&v[index]),
            Self::Ids(v) => Node::Raw(v[index].as_bytes()),
        }
    }
}

#[derive(Clone, Copy)]
enum Chunk<'a> {
    Borrowed(&'a [u8]),
    Inline { bytes: [u8; 32], len: usize },
}

impl Chunk<'_> {
    fn inline(value: &[u8]) -> Self {
        // Only SCB scalar encodings (<=19 bytes) and two u64 prefixes enter here.
        let mut bytes = [0; 32];
        bytes[..value.len()].copy_from_slice(value);
        Self::Inline {
            bytes,
            len: value.len(),
        }
    }

    fn uint(value: usize) -> Result<Self> {
        let value = u64::try_from(value).map_err(|_| limit())?;
        Ok(Self::unsigned(value.into()))
    }

    fn unsigned(value: u128) -> Self {
        let mut bytes = [0; 19];
        let length = encode_uvar128_into(value, &mut bytes);
        Self::inline(&bytes[..length])
    }

    fn signed(value: i128) -> Self {
        let mut bytes = [0; 19];
        let length = encode_sint128_into(value, &mut bytes);
        Self::inline(&bytes[..length])
    }

    fn bytes(&self) -> &[u8] {
        match self {
            Self::Borrowed(bytes) => bytes,
            Self::Inline { bytes, len } => &bytes[..*len],
        }
    }
}

#[derive(Clone, Copy)]
enum Layout<'a> {
    Atom(Chunk<'a>),
    Bytes(&'a [u8]),
    Union(u32, Option<Node<'a>>),
    Record(RecordFields<'a>),
    List(Sequence<'a>),
}

/// Borrow the record owner instead of materializing every child at each visit.
/// Measurement and the byte cursor share this field order and framing.
#[derive(Clone, Copy)]
enum RecordFields<'a> {
    Value(&'a ConstValue),
    Named(&'a NamedType),
    TypePair(&'a TypeExpr, &'a TypeExpr),
    Function(&'a FunctionType),
    FunctionRef(&'a FunctionRefValue),
    Failure(&'a BuiltinFailureValue),
    Field(&'a FieldConst),
    Record(&'a RecordConst),
    Variant(&'a VariantConst),
    Entry(&'a MapEntryConst),
}

impl<'a> RecordFields<'a> {
    fn len(self) -> usize {
        match self {
            Self::Function(_) | Self::Variant(_) => 3,
            _ => 2,
        }
    }

    fn get(self, index: usize) -> Node<'a> {
        match (self, index) {
            (Self::Value(v), 0) => Node::Type(&v.value_type),
            (Self::Value(v), 1) => Node::Data(&v.data),
            (Self::Named(v), 0) => Node::Raw(v.definition.as_bytes()),
            (Self::Named(v), 1) => Node::Sequence(Sequence::Types(&v.arguments)),
            (Self::TypePair(a, _), 0) => Node::Type(a),
            (Self::TypePair(_, b), 1) => Node::Type(b),
            (Self::Function(v), 0) => Node::Sequence(Sequence::Types(&v.parameters)),
            (Self::Function(v), 1) => Node::Type(&v.result),
            (Self::Function(v), 2) => Node::Sequence(Sequence::Ids(&v.effects)),
            (Self::FunctionRef(v), 0) => Node::Raw(v.function.as_bytes()),
            (Self::FunctionRef(v), 1) => Node::Sequence(Sequence::Types(&v.type_arguments)),
            (Self::Failure(v), 0) => Node::UInt(v.kind.tag().into()),
            (Self::Failure(v), 1) => Node::UInt(v.code.into()),
            (Self::Field(v), 0) => Node::Raw(v.member_id.as_bytes()),
            (Self::Field(v), 1) => Node::Value(&v.value),
            (Self::Record(v), 0) => Node::Raw(v.definition.as_bytes()),
            (Self::Record(v), 1) => Node::Sequence(Sequence::Fields(&v.fields)),
            (Self::Variant(v), 0) => Node::Raw(v.definition.as_bytes()),
            (Self::Variant(v), 1) => Node::Raw(v.member_id.as_bytes()),
            (Self::Variant(v), 2) => Node::Option(v.payload.as_deref()),
            (Self::Entry(v), 0) => Node::Value(&v.key),
            (Self::Entry(v), 1) => Node::Value(&v.value),
            _ => unreachable!("record field index is bounded by its schema"),
        }
    }
}

/// Two consumers share one wire schema: the cursor needs a layout, while
/// measurement can fold child sizes directly without constructing that layout.
trait WireConsumer<'a> {
    type Output;

    fn atom(self, chunk: Chunk<'a>) -> Result<Self::Output>;
    fn bytes(self, bytes: &'a [u8]) -> Result<Self::Output>;
    fn union(self, tag: u32, payload: Option<Node<'a>>) -> Result<Self::Output>;
    fn record(self, fields: RecordFields<'a>) -> Result<Self::Output>;
    fn list(self, values: Sequence<'a>) -> Result<Self::Output>;
}

struct BuildLayout;

impl<'a> WireConsumer<'a> for BuildLayout {
    type Output = Layout<'a>;

    fn atom(self, chunk: Chunk<'a>) -> Result<Self::Output> {
        Ok(Layout::Atom(chunk))
    }

    fn bytes(self, bytes: &'a [u8]) -> Result<Self::Output> {
        Ok(Layout::Bytes(bytes))
    }

    fn union(self, tag: u32, payload: Option<Node<'a>>) -> Result<Self::Output> {
        Ok(Layout::Union(tag, payload))
    }

    fn record(self, fields: RecordFields<'a>) -> Result<Self::Output> {
        Ok(Layout::Record(fields))
    }

    fn list(self, values: Sequence<'a>) -> Result<Self::Output> {
        Ok(Layout::List(values))
    }
}

impl<'a> Node<'a> {
    fn layout(self, depth: usize) -> Result<Layout<'a>> {
        self.visit(depth, BuildLayout)
    }

    #[inline]
    #[allow(clippy::too_many_lines)]
    fn visit<C: WireConsumer<'a>>(self, depth: usize, consumer: C) -> Result<C::Output> {
        check_depth(depth)?;
        if !matches!(
            self,
            Self::UInt(_)
                | Self::SInt(_)
                | Self::Bool(_)
                | Self::F32(_)
                | Self::F64(_)
                | Self::Raw(_)
                | Self::Bytes(_)
        ) {
            check_container_depth(depth)?;
        }
        match self {
            Self::Value(v) => consumer.record(RecordFields::Value(v)),
            Self::Type(v) => {
                let payload = match v {
                    TypeExpr::Unit
                    | TypeExpr::Bool
                    | TypeExpr::F32
                    | TypeExpr::F64
                    | TypeExpr::Bytes
                    | TypeExpr::Text => None,
                    TypeExpr::SInt(w) | TypeExpr::UInt(w) => Some(Self::UInt(w.bits().into())),
                    TypeExpr::Tuple(v) => Some(Self::Sequence(Sequence::Types(v))),
                    TypeExpr::Named(v) => Some(Self::Named(v)),
                    TypeExpr::Vector(v) | TypeExpr::Option(v) | TypeExpr::LocalCell(v) => {
                        Some(Self::Type(v))
                    }
                    TypeExpr::OrderedMap { key, value } => Some(Self::TypePair(key, value)),
                    TypeExpr::Result { ok, error } => Some(Self::TypePair(ok, error)),
                    TypeExpr::FunctionRef(v) => Some(Self::Function(v)),
                    TypeExpr::AdapterHandle(v) | TypeExpr::CapabilityToken(v) => {
                        Some(Self::Raw(v.as_bytes()))
                    }
                    TypeExpr::TypeParameter(v) => Some(Self::UInt((*v).into())),
                    TypeExpr::BuiltinFailure(v) => Some(Self::UInt(v.tag().into())),
                };
                consumer.union(v.tag(), payload)
            }
            Self::Data(v) => {
                let payload = match v {
                    ConstData::Unit => None,
                    ConstData::Bool(v) => Some(Self::Bool(*v)),
                    ConstData::SInt(v) => Some(Self::SInt(*v)),
                    ConstData::UInt(v) => Some(Self::UInt(*v)),
                    ConstData::F32Bits(v) => Some(Self::F32(*v)),
                    ConstData::F64Bits(v) => Some(Self::F64(*v)),
                    ConstData::Bytes(v) => Some(Self::Bytes(v)),
                    ConstData::Text(v) => Some(Self::Bytes(v.as_bytes())),
                    ConstData::Sequence(v) => Some(Self::Sequence(Sequence::Values(v))),
                    ConstData::Record(v) => Some(Self::Record(v)),
                    ConstData::Variant(v) => Some(Self::Variant(v)),
                    ConstData::Map(v) => Some(Self::Sequence(Sequence::Entries(v))),
                    ConstData::Option(v) => Some(Self::Option(v.as_deref())),
                    ConstData::Result(v) => Some(Self::Result(v)),
                    ConstData::FunctionRef(v) => Some(Self::FunctionRef(v)),
                    ConstData::BuiltinFailure(v) => Some(Self::Failure(v)),
                };
                consumer.union(v.tag(), payload)
            }
            Self::Named(v) => consumer.record(RecordFields::Named(v)),
            Self::TypePair(a, b) => consumer.record(RecordFields::TypePair(a, b)),
            Self::Function(v) => consumer.record(RecordFields::Function(v)),
            Self::FunctionRef(v) => consumer.record(RecordFields::FunctionRef(v)),
            Self::Failure(v) => consumer.record(RecordFields::Failure(v)),
            Self::Field(v) => consumer.record(RecordFields::Field(v)),
            Self::Record(v) => consumer.record(RecordFields::Record(v)),
            Self::Variant(v) => consumer.record(RecordFields::Variant(v)),
            Self::Entry(v) => consumer.record(RecordFields::Entry(v)),
            Self::Result(v) => {
                let (ResultConst::Ok(value) | ResultConst::Err(value)) = v;
                consumer.union(v.tag(), Some(Self::Value(value)))
            }
            Self::Option(v) => consumer.union(u32::from(v.is_some()), v.map(Self::Value)),
            Self::Sequence(v) => consumer.list(v),
            Self::UInt(v) => consumer.atom(Chunk::unsigned(v)),
            Self::SInt(v) => consumer.atom(Chunk::signed(v)),
            Self::Bool(v) => consumer.atom(Chunk::inline(&[u8::from(v)])),
            Self::F32(v) => consumer.atom(Chunk::inline(&encode_f32_bits(v)?)),
            Self::F64(v) => consumer.atom(Chunk::inline(&encode_f64_bits(v)?)),
            Self::Raw(v) => consumer.atom(Chunk::Borrowed(v)),
            Self::Bytes(v) => {
                if v.len() > MAX_BYTE_PAYLOAD {
                    return Err(limit());
                }
                consumer.bytes(v)
            }
        }
    }
}

fn limit() -> ScbError {
    ScbError::new(ScbErrorCode::ResourceLimit)
}

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or_else(limit)
}

fn uvar_len(value: usize) -> usize {
    let bits = usize::BITS - value.leading_zeros();
    usize::try_from(bits.max(1).div_ceil(7)).expect("uvar length fits usize")
}

fn sized(length: usize) -> Result<usize> {
    add(uvar_len(length), length)
}

fn checked_length(length: usize) -> Result<usize> {
    if length > MAX_STANDALONE_BYTES {
        Err(limit())
    } else {
        Ok(length)
    }
}

/// `canonical` is false only for immutable subtrees already validated by the
/// enclosing measurement. This avoids recursively validating nested map keys
/// again during a cursor's prefix-length calculation.
fn measure(node: Node<'_>, depth: usize, canonical: bool) -> Result<usize> {
    checked_length(node.visit(depth, Measure { depth, canonical })?)
}

struct Measure {
    depth: usize,
    canonical: bool,
}

impl<'a> WireConsumer<'a> for Measure {
    type Output = usize;

    fn atom(self, chunk: Chunk<'a>) -> Result<usize> {
        Ok(chunk.bytes().len())
    }

    fn bytes(self, bytes: &'a [u8]) -> Result<usize> {
        sized(bytes.len())
    }

    fn union(self, tag: u32, payload: Option<Node<'a>>) -> Result<usize> {
        let length = payload.map_or(Ok(0), |node| measure(node, self.depth + 1, self.canonical))?;
        add(uvar_len(tag as usize), sized(length)?)
    }

    fn record(self, fields: RecordFields<'a>) -> Result<usize> {
        measure_record(fields, self.depth, self.canonical, None)
    }

    fn list(self, values: Sequence<'a>) -> Result<usize> {
        let Self { depth, canonical } = self;
        if canonical && let Sequence::Ids(ids) = values {
            validate_entity_id_set_order(ids)?;
        }
        let mut length = uvar_len(values.len());
        for index in 0..values.len() {
            let child_length = if canonical && let Sequence::Entries(entries) = values {
                let entry = &entries[index];
                let key_length = measure(Node::Value(&entry.key), depth + 2, true)?;
                if index > 0 {
                    ensure_increasing(compare_validated(
                        Node::Value(&entries[index - 1].key),
                        Node::Value(&entry.key),
                        depth + 2,
                    )?)?;
                }
                let Layout::Record(fields) = Node::Entry(entry).layout(depth + 1)? else {
                    unreachable!()
                };
                measure_record(fields, depth + 1, true, Some(key_length))
                    .and_then(checked_length)?
            } else {
                measure(values.get(index), depth + 1, canonical)?
            };
            length = add(length, sized(child_length)?)?;
        }
        // The old codec checked collection/aggregate limits after children.
        if u64::try_from(values.len()).map_err(|_| limit())? > MAX_COLLECTION_ELEMENTS {
            return Err(limit());
        }
        Ok(length)
    }
}

fn measure_record(
    fields: RecordFields<'_>,
    depth: usize,
    canonical: bool,
    first_length: Option<usize>,
) -> Result<usize> {
    let mut length = uvar_len(fields.len());
    for index in 0..fields.len() {
        let child_length = if index == 0
            && let Some(length) = first_length
        {
            length
        } else {
            measure(fields.get(index), depth + 1, canonical)?
        };
        length = add(length, add(uvar_len(index + 1), sized(child_length)?)?)?;
    }
    Ok(length)
}

fn ensure_increasing(order: Ordering) -> Result<()> {
    match order {
        Ordering::Less => Ok(()),
        Ordering::Equal => Err(ScbError::new(ScbErrorCode::MapDuplicate)),
        Ordering::Greater => Err(ScbError::new(ScbErrorCode::MapOrder)),
    }
}

#[derive(Clone, Copy)]
enum Frame<'a> {
    Node(Node<'a>, usize),
    Chunk(Chunk<'a>),
    Children {
        layout: Layout<'a>,
        index: usize,
        depth: usize,
    },
}

// Each container retains one sibling iterator and at most two header frames;
// wide lists never put all children on this stack.
const MAX_CURSOR_FRAMES: usize = 3 * (MAX_NESTING_DEPTH + 1) + 1;
struct WireCursor<'a> {
    stack: Vec<Frame<'a>>,
}

impl<'a> WireCursor<'a> {
    fn new(node: Node<'a>, depth: usize) -> Result<Self> {
        let mut stack = Vec::new();
        stack
            .try_reserve_exact(MAX_CURSOR_FRAMES)
            .map_err(|_| limit())?;
        stack.push(Frame::Node(node, depth));
        Ok(Self { stack })
    }

    fn push(&mut self, frame: Frame<'a>) -> Result<()> {
        if self.stack.len() == MAX_CURSOR_FRAMES {
            return Err(limit());
        }
        self.stack.push(frame);
        Ok(())
    }

    fn next(&mut self) -> Result<Option<Chunk<'a>>> {
        while let Some(frame) = self.stack.pop() {
            match frame {
                Frame::Chunk(chunk) => return Ok(Some(chunk)),
                Frame::Node(node, depth) => match node.layout(depth)? {
                    Layout::Atom(chunk) => return Ok(Some(chunk)),
                    Layout::Bytes(bytes) => {
                        self.push(Frame::Chunk(Chunk::Borrowed(bytes)))?;
                        return Ok(Some(Chunk::uint(bytes.len())?));
                    }
                    layout => {
                        let header = match layout {
                            Layout::Union(tag, _) => tag as usize,
                            Layout::Record(fields) => fields.len(),
                            Layout::List(values) => values.len(),
                            _ => unreachable!(),
                        };
                        self.push(Frame::Children {
                            layout,
                            index: 0,
                            depth,
                        })?;
                        return Ok(Some(Chunk::uint(header)?));
                    }
                },
                Frame::Children {
                    layout,
                    index,
                    depth,
                } => {
                    let child = match layout {
                        Layout::Union(_, payload) => {
                            if index != 0 {
                                continue;
                            }
                            payload
                        }
                        Layout::Record(fields) => {
                            if index == fields.len() {
                                continue;
                            }
                            Some(fields.get(index))
                        }
                        Layout::List(values) => {
                            if index == values.len() {
                                continue;
                            }
                            Some(values.get(index))
                        }
                        _ => unreachable!(),
                    };
                    let length = child.map_or(Ok(0), |n| measure(n, depth + 1, false))?;
                    self.push(Frame::Children {
                        layout,
                        index: index + 1,
                        depth,
                    })?;
                    if let Some(child) = child {
                        self.push(Frame::Node(child, depth + 1))?;
                    }
                    if matches!(layout, Layout::Record(_)) {
                        self.push(Frame::Chunk(Chunk::uint(length)?))?;
                        return Ok(Some(Chunk::uint(index + 1)?));
                    }
                    return Ok(Some(Chunk::uint(length)?));
                }
            }
        }
        Ok(None)
    }
}

fn compare_validated(left: Node<'_>, right: Node<'_>, depth: usize) -> Result<Ordering> {
    let mut left = WireCursor::new(left, depth)?;
    let mut right = WireCursor::new(right, depth)?;
    let mut a = left.next()?;
    let mut b = right.next()?;
    let (mut ai, mut bi) = (0, 0);
    loop {
        if let Some(chunk) = &a
            && ai == chunk.bytes().len()
        {
            a = left.next()?;
            ai = 0;
            continue;
        }
        if let Some(chunk) = &b
            && bi == chunk.bytes().len()
        {
            b = right.next()?;
            bi = 0;
            continue;
        }
        match (&a, &b) {
            (None, None) => return Ok(Ordering::Equal),
            (None, Some(_)) => return Ok(Ordering::Less),
            (Some(_), None) => return Ok(Ordering::Greater),
            (Some(a), Some(b)) => {
                let common = (a.bytes().len() - ai).min(b.bytes().len() - bi);
                let order = a.bytes()[ai..ai + common].cmp(&b.bytes()[bi..bi + common]);
                if order != Ordering::Equal {
                    return Ok(order);
                }
                ai += common;
                bi += common;
            }
        }
    }
}

pub(super) fn measure_value(value: &ConstValue, cap: u64) -> Result<ConstValueByteMeasure> {
    let length = u64::try_from(measure(Node::Value(value), 0, true)?).map_err(|_| limit())?;
    Ok(if length <= cap {
        ConstValueByteMeasure::Exact(length)
    } else {
        ConstValueByteMeasure::OverLimit
    })
}

pub(super) fn encode(node: Node<'_>, depth: usize) -> Result<Vec<u8>> {
    let length = measure(node, depth, true)?;
    let mut out = Vec::new();
    out.try_reserve_exact(length).map_err(|_| limit())?;
    let mut cursor = WireCursor::new(node, depth)?;
    while let Some(chunk) = cursor.next()? {
        out.extend_from_slice(chunk.bytes());
    }
    if out.len() != length {
        return Err(limit());
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
