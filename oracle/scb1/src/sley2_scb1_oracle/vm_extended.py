"""Independent conformance oracle for the S20-260/S20-270 extended bytecode.

This module decodes the `SLEYBC02` container and re-derives the bytecode cache
key from the frozen preimage, implemented from the contracts alone
(`docs/spec/VM_LOWERING_PROFILE_V1.md` sections 2.1 and 3, the extended
profile's section 1, and the SSMC1 epoch-1 schema manifest) and sharing no code
with the Rust implementation.

It is a codec and identity oracle, not a second semantic kernel: it never
judges an opcode signature, executes an instruction, or recomputes a value
hash or an observation identity, all of which stay the VM owner's.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

import blake3

CACHE_KEY_DOMAIN = b"sley2.vm-bytecode-cache-key.v1"
CACHE_PREIMAGE_MAGIC = b"SLEYBCK1"
EXTENDED_MAGIC = b"SLEYBC02"
RESTRICTED_MAGIC = b"SLEYBC01"
FORMAT_VERSION = 1
PROFILE_VERSION = 1
# `docs/spec/VM_LOWERING_PROFILE_V1.md` section 3.
FIELD_SCHEMA_HASH = bytes.fromhex(
    "1983bc8d6ad9ac3cb5390853f43959cf2c3dc0ae8e0ca18ca8264ca4960133ae"
)
DECODER_LIMITS_HASH = bytes.fromhex(
    "389791b170bc9d8575f7e6f338e4f9e9f2b75f35d7a2e52c7cb106cb2cd6136a"
)
# `CacheProfile::EXTENDED_V1` of the extended profile contract, section 1,
# revision 12: the E6 callee-table layout change bumped the lowerer to
# [2, 0, 0], and any future SLEYBC02 layout change bumps it again.
EXTENDED_PROFILE = {
    "vm_version": (1, 0, 0),
    "lowering_profile": 2,
    "lowerer_version": (2, 0, 0),
    "entry_type_arguments": 0,
    "adapter_abi_entries": 0,
    "execution_abi_flags": 0,
}
# SSMC1 epoch-1 type tags with a payload beyond the tag itself.
TYPE_TAGS = {
    1: "Unit",
    2: "Bool",
    3: "SInt",
    4: "UInt",
    5: "F32",
    6: "F64",
    7: "Bytes",
    8: "Text",
    9: "Tuple",
    10: "Named",
    11: "Vector",
    12: "OrderedMap",
    13: "Option",
    14: "Result",
    15: "FunctionRef",
    16: "AdapterHandle",
    17: "CapabilityToken",
    18: "LocalCell",
    19: "TypeParameter",
    20: "BuiltinFailure",
}
IMMEDIATE_TAGS = {
    1: "None",
    2: "Entity",
    3: "Index",
    4: "Field",
    5: "Variant",
    6: "Observation",
    7: "Function",
}
TERMINATOR_TAGS = {1: "Return", 2: "Branch", 3: "CondBranch", 4: "Switch", 5: "Trap"}


# The opcodes the landed profile families may lower: E1 through E6 and slice
# E7a. An artifact naming any other opcode is refused, so a lowerer that
# quietly admitted an excluded E7 opcode would fail here independently.
PROFILE_OPCODES = frozenset(
    # The three restricted-profile Booleans, which the extended profile keeps.
    [102, 103, 104]
    + [1, 16, 17, 32, 33, 34, 35, 96, 97, 98, 99, 100, 101, 128, 129, 130, 131]
    + [64, 65, 66, 67, 68, 69, 70, 71]
    + [80, 81, 82, 83, 84, 85]
    + [18, 19, 20, 21, 36, 37, 38, 39, 40]
    + [176, 177, 178, 192, 193, 194]
    + [112]
    + [144]
)


class BytecodeError(Exception):
    """One exact independent decoding failure."""


class _Cursor:
    """A strict big-endian reader over the bytecode artifact."""

    def __init__(self, data: bytes) -> None:
        self.data = data
        self.offset = 0

    def take(self, count: int) -> bytes:
        end = self.offset + count
        if count < 0 or end > len(self.data):
            raise BytecodeError("bytecode ended inside a field")
        chunk = self.data[self.offset : end]
        self.offset = end
        return chunk

    def u16(self) -> int:
        return int.from_bytes(self.take(2), "big")

    def u32(self) -> int:
        return int.from_bytes(self.take(4), "big")

    def u64(self) -> int:
        return int.from_bytes(self.take(8), "big")

    def fixed32(self) -> bytes:
        return self.take(32)

    def count(self) -> int:
        value = self.u64()
        if value > len(self.data):
            raise BytecodeError("list count exceeds the artifact length")
        return value

    def entity_reference(self) -> bytes:
        # Bytecode has no self-reference abbreviation: every reference is
        # `u32be(1) || EntityId` (lowering profile section 2.1).
        tag = self.u32()
        if tag != 1:
            raise BytecodeError(f"entity reference tag {tag} is not the external form")
        return self.fixed32()

    def type_expr(self) -> dict[str, Any]:
        tag = self.u32()
        name = TYPE_TAGS.get(tag)
        if name is None:
            raise BytecodeError(f"unknown type tag {tag}")
        if name in ("Unit", "Bool", "F32", "F64", "Bytes", "Text"):
            return {"type": name}
        if name in ("SInt", "UInt"):
            return {"type": name, "width": self.u16()}
        if name == "Tuple":
            return {"type": name, "elements": [self.type_expr() for _ in range(self.count())]}
        if name == "Named":
            definition = self.entity_reference()
            return {
                "type": name,
                "definition": definition.hex(),
                "arguments": [self.type_expr() for _ in range(self.count())],
            }
        if name in ("Vector", "Option", "LocalCell"):
            return {"type": name, "element": self.type_expr()}
        if name == "OrderedMap":
            return {"type": name, "key": self.type_expr(), "value": self.type_expr()}
        if name == "Result":
            return {"type": name, "ok": self.type_expr(), "error": self.type_expr()}
        if name == "FunctionRef":
            parameters = [self.type_expr() for _ in range(self.count())]
            result = self.type_expr()
            effects = [self.entity_reference().hex() for _ in range(self.count())]
            return {
                "type": name,
                "parameters": parameters,
                "result": result,
                "effects": effects,
            }
        if name in ("AdapterHandle", "CapabilityToken"):
            return {"type": name, "entity": self.entity_reference().hex()}
        if name == "TypeParameter":
            return {"type": name, "ordinal": self.u32()}
        return {"type": name, "kind": self.u16()}

    def immediate(self) -> dict[str, Any]:
        tag = self.u32()
        name = IMMEDIATE_TAGS.get(tag)
        if name is None:
            raise BytecodeError(f"unknown immediate tag {tag}")
        if name == "None":
            return {"immediate": name}
        if name == "Entity":
            return {"immediate": name, "entity": self.entity_reference().hex()}
        if name == "Index":
            return {"immediate": name, "index": self.u32()}
        if name == "Field":
            return {"immediate": name, "member": self.fixed32().hex()}
        if name == "Variant":
            return {
                "immediate": name,
                "definition": self.entity_reference().hex(),
                "member": self.fixed32().hex(),
            }
        if name == "Observation":
            return {"immediate": name, "observation": self.fixed32().hex()}
        function = self.entity_reference().hex()
        return {
            "immediate": name,
            "function": function,
            "type_arguments": [self.type_expr() for _ in range(self.count())],
        }

    def registers(self) -> list[int]:
        return [self.u32() for _ in range(self.count())]

    def target_edge(self) -> dict[str, Any]:
        return {"block": self.u32(), "arguments": self.registers()}

    def terminator(self) -> dict[str, Any]:
        tag = self.u32()
        name = TERMINATOR_TAGS.get(tag)
        if name is None:
            raise BytecodeError(f"unknown terminator tag {tag}")
        if name == "Return":
            return {"terminator": name, "value": self.u32()}
        if name == "Branch":
            return {"terminator": name, "target": self.target_edge()}
        if name == "CondBranch":
            return {
                "terminator": name,
                "condition": self.u32(),
                "true": self.target_edge(),
                "false": self.target_edge(),
            }
        if name == "Switch":
            scrutinee = self.u32()
            cases = []
            for _ in range(self.count()):
                key_tag = self.u32()
                if key_tag == 1:
                    key: Any = {"member": self.fixed32().hex()}
                elif key_tag == 2:
                    key = {"builtin": self.u32()}
                else:
                    raise BytecodeError(f"unknown case key tag {key_tag}")
                target = self.u32()
                arguments = []
                for _ in range(self.count()):
                    argument_tag = self.u32()
                    if argument_tag == 1:
                        arguments.append({"value": self.u32()})
                    elif argument_tag == 2:
                        arguments.append({"case_payload": True})
                    else:
                        raise BytecodeError(
                            f"unknown switch argument tag {argument_tag}"
                        )
                cases.append({"key": key, "target": target, "arguments": arguments})
            default_tag = self.u32()
            if default_tag == 1:
                default = None
            elif default_tag == 2:
                default = self.target_edge()
            else:
                raise BytecodeError(f"unknown switch default option tag {default_tag}")
            return {
                "terminator": name,
                "scrutinee": scrutinee,
                "cases": cases,
                "default": default,
            }
        code = self.u32()
        payload_tag = self.u32()
        if payload_tag == 1:
            payload = None
        elif payload_tag == 2:
            payload = self.u32()
        else:
            raise BytecodeError(f"unknown trap payload option tag {payload_tag}")
        return {"terminator": name, "code": code, "payload": payload}


def decode_bytecode(data: bytes) -> dict[str, Any]:
    """Decodes one `SLEYBC02` artifact, including its callee table."""
    cursor = _Cursor(data)
    magic = cursor.take(8)
    if magic != EXTENDED_MAGIC:
        raise BytecodeError(f"magic {magic!r} is not the extended container")
    version = cursor.u32()
    if version != FORMAT_VERSION:
        raise BytecodeError(f"format version {version} is not 1")
    entry = _decode_body(cursor)
    callees = [_decode_body(cursor) for _ in range(cursor.count())]
    if cursor.offset != len(data):
        raise BytecodeError("trailing bytes after the callee table")
    return {"magic": magic.decode("ascii"), "entry": entry, "callees": callees}


def _decode_body(cursor: _Cursor) -> dict[str, Any]:
    function = cursor.fixed32()
    parameter_registers = cursor.registers()
    register_types = [cursor.type_expr() for _ in range(cursor.count())]
    result_type = cursor.type_expr()
    entry_block = cursor.u32()
    blocks = []
    for _ in range(cursor.count()):
        slot = cursor.u32()
        block_parameters = cursor.registers()
        instructions = []
        for _ in range(cursor.count()):
            opcode = cursor.u32()
            if opcode not in PROFILE_OPCODES:
                raise BytecodeError(f"opcode {opcode} is outside the landed profile families")
            operands = cursor.registers()
            results = cursor.registers()
            immediate = cursor.immediate()
            instructions.append(
                {
                    "opcode": opcode,
                    "operands": operands,
                    "results": results,
                    **immediate,
                }
            )
        terminator = cursor.terminator()
        reachability = cursor.u32()
        blocks.append(
            {
                "slot": slot,
                "parameters": block_parameters,
                "instructions": instructions,
                "terminator": terminator,
                "reachability": reachability,
            }
        )
    return {
        "function": function.hex(),
        "parameter_registers": parameter_registers,
        "register_types": register_types,
        "result_type": result_type,
        "entry_block": entry_block,
        "blocks": blocks,
    }


def cache_key(schema_epoch: bytes, state_root: bytes, function: bytes) -> bytes:
    """Re-derives the extended-profile bytecode cache key from the contract."""
    profile = EXTENDED_PROFILE
    preimage = b"".join(
        [
            CACHE_PREIMAGE_MAGIC,
            PROFILE_VERSION.to_bytes(4, "big"),
            schema_epoch,
            FIELD_SCHEMA_HASH,
            DECODER_LIMITS_HASH,
            state_root,
            function,
            *(part.to_bytes(4, "big") for part in profile["vm_version"]),
            profile["lowering_profile"].to_bytes(4, "big"),
            *(part.to_bytes(4, "big") for part in profile["lowerer_version"]),
            profile["entry_type_arguments"].to_bytes(8, "big"),
            profile["adapter_abi_entries"].to_bytes(8, "big"),
            profile["execution_abi_flags"].to_bytes(8, "big"),
        ]
    )
    if len(preimage) != 224:
        raise BytecodeError(f"cache preimage is {len(preimage)} bytes, not 224")
    return blake3.blake3(CACHE_KEY_DOMAIN + preimage).digest()


def check_vm_extended(accepted_path: Path, rejected_path: Path | None = None) -> dict[str, Any]:
    """Checks the frozen extended-profile corpus with the independent decoder."""
    accepted = json.loads(accepted_path.read_text(encoding="utf-8"))
    problems: list[str] = []
    if accepted.get("contract") != "sley2-vm-extended-opcode-profile-v1":
        problems.append("contract drift")
    if accepted.get("bytecode_magic") != "SLEYBC02":
        problems.append("magic drift")
    if accepted.get("cache_profile") != "EXTENDED_V1":
        problems.append("profile drift")
    schema_epoch = bytes.fromhex(accepted.get("schema_epoch_hex", ""))
    state_root = bytes.fromhex(accepted.get("state_root_hex", ""))
    if len(schema_epoch) != 32 or len(state_root) != 32:
        problems.append("fixture identity drift")
        return _result(accepted, problems)

    for vector in accepted.get("vectors", []):
        label = vector.get("id", "?")
        data = bytes.fromhex(vector["bytecode_hex"])
        if hashlib.sha256(data).hexdigest() != vector["bytecode_sha256"]:
            problems.append(f"{label}: bytecode SHA-256 drift")
        try:
            decoded = decode_bytecode(data)
        except BytecodeError as error:
            problems.append(f"{label}: {error}")
            continue
        entry = decoded["entry"]
        derived = cache_key(schema_epoch, state_root, bytes.fromhex(entry["function"]))
        if derived.hex() != vector["cache_key_hex"]:
            problems.append(f"{label}: cache key drift")
        opcodes = [
            instruction["opcode"]
            for block in entry["blocks"]
            for instruction in block["instructions"]
        ]
        if not opcodes:
            problems.append(f"{label}: the entry body carries no instruction")
        elif opcodes[-1] != vector["opcode"]:
            problems.append(f"{label}: recorded opcode is not the entry's last")
        if vector["instruction_count"] < len(opcodes):
            problems.append(f"{label}: instruction count is below the static count")
        if entry["blocks"] and entry["blocks"][0]["slot"] != entry["entry_block"]:
            problems.append(f"{label}: the entry block slot is not first")
        if len(entry["register_types"]) < len(entry["parameter_registers"]):
            problems.append(f"{label}: fewer register types than parameter registers")

    rejected_count = 0
    if rejected_path is not None:
        rejected = json.loads(rejected_path.read_text(encoding="utf-8"))
        if rejected.get("contract") != "sley2-vm-extended-opcode-profile-v1":
            problems.append("rejected contract drift")
        for mutation in rejected.get("mutations", []):
            rejected_count += 1
            label = mutation.get("id", "?")
            data = bytes.fromhex(mutation["input_hex"])
            if hashlib.sha256(data).hexdigest() != mutation["input_sha256"]:
                problems.append(f"rejected {label}: input SHA-256 drift")
            try:
                decode_bytecode(data)
            except BytecodeError:
                continue
            problems.append(f"rejected {label}: the independent decoder accepted it")
    return _result(accepted, problems, rejected_count)


def _result(
    accepted: dict[str, Any], problems: list[str], rejected_vectors: int = 0
) -> dict[str, Any]:
    return {
        "accepted_vectors": len(accepted.get("vectors", [])),
        "rejected_vectors": rejected_vectors,
        "claim": accepted.get("claim"),
        "contract": "s20-260-270-independent-extended-bytecode-oracle-v1",
        "problems": problems,
        "result": "FAIL" if problems else "PASS",
        "scope": "SLEYBC02 CONTAINER AND CACHE IDENTITY ONLY; NO SEMANTIC JUDGMENT",
    }
