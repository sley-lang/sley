"""Positional Rust source scanning shared by evidence checkers.

This lexer distinguishes code from comments and literals; it is not a Rust
parser. Test-module partitioning only removes exact cfg(test) inline modules.
Unknown conditional forms remain production for conservative hygiene checks.

The lexical mask mirrors the frozen S20-530 checker. That checker retains its
own copy because its isolated launch and frozen trust inventory forbid importing
this helper; regression tests compare the two scanners on representative input.
"""
from __future__ import annotations

import re

RUST_RAW_STRING_PREFIX = re.compile(r'(?:br|cr|r)(?P<hashes>#{0,255})"')

def rust_char_literal_end(source: str, start: int) -> int | None:
    byte_literal = source.startswith("b'", start)
    quote = start + 1 if byte_literal else start
    if quote >= len(source) or source[quote] != "'":
        return None
    cursor = quote + 1
    if cursor >= len(source):
        return None
    if source[cursor] == "\\":
        cursor += 1
        if cursor >= len(source):
            return None
        escape = source[cursor]
        if escape in "nrt\\0'\"":
            cursor += 1
        elif escape == "x":
            digits = source[cursor + 1 : cursor + 3]
            if len(digits) != 2 or re.fullmatch(r"[0-9A-Fa-f]{2}", digits) is None:
                return None
            cursor += 3
        elif escape == "u" and not byte_literal:
            if cursor + 1 >= len(source) or source[cursor + 1] != "{":
                return None
            closing = source.find("}", cursor + 2)
            if closing < 0:
                return None
            digits = source[cursor + 2 : closing]
            hexadecimal = digits.replace("_", "")
            if (
                not 1 <= len(hexadecimal) <= 6
                or re.fullmatch(r"[0-9A-Fa-f_]+", digits) is None
                or re.fullmatch(r"[0-9A-Fa-f]+", hexadecimal) is None
            ):
                return None
            cursor = closing + 1
        else:
            return None
    else:
        if source[cursor] in "'\r\n":
            return None
        cursor += 1
    if cursor >= len(source) or source[cursor] != "'":
        return None
    return cursor + 1


def rust_code_mask(source: str) -> list[bool]:
    """Return positions that are Rust code rather than comments or literals."""
    mask = [True] * len(source)
    index = 0
    while index < len(source):
        if source.startswith("//", index):
            end = source.find("\n", index)
            end = len(source) if end < 0 else end
            mask[index:end] = [False] * (end - index)
            index = end
            continue
        if source.startswith("/*", index):
            depth = 1
            cursor = index + 2
            while cursor < len(source) and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            mask[index:cursor] = [False] * (cursor - index)
            index = cursor
            continue
        prefix_boundary = index == 0 or not (
            source[index - 1].isalnum() or source[index - 1] == "_"
        )
        raw = RUST_RAW_STRING_PREFIX.match(source, index) if prefix_boundary else None
        if raw is not None:
            delimiter = '"' + raw["hashes"]
            cursor = raw.end()
            end = source.find(delimiter, cursor)
            end = len(source) if end < 0 else end + len(delimiter)
            mask[index:end] = [False] * (end - index)
            index = end
            continue
        quote_index = None
        if source[index] == '"':
            quote_index = index
        elif prefix_boundary and source.startswith(('b"', 'c"'), index):
            quote_index = index + 1
        if quote_index is not None:
            cursor = quote_index + 1
            while cursor < len(source):
                if source[cursor] == "\\":
                    cursor += 2
                elif source[cursor] == '"':
                    cursor += 1
                    break
                else:
                    cursor += 1
            mask[index:cursor] = [False] * (cursor - index)
            index = cursor
            continue
        char_end = None
        if source[index] == "'":
            char_end = rust_char_literal_end(source, index)
        elif prefix_boundary and source.startswith("b'", index):
            char_end = rust_char_literal_end(source, index)
        if char_end is not None:
            mask[index:char_end] = [False] * (char_end - index)
            index = char_end
            continue
        index += 1
    return mask



TEST_MODULE = re.compile(
    r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*"
    r"(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{"
)


def test_module_ranges(source: str) -> list[tuple[int, int]]:
    """Return complete inline test modules, respecting literal/comment braces."""
    mask = rust_code_mask(source)
    projection = ''.join(c if mask[i] else ' ' for i, c in enumerate(source))
    ranges = []
    for match in TEST_MODULE.finditer(projection):
        if ranges and match.start() < ranges[-1][1]:
            continue
        depth = 1
        end = match.end()
        while end < len(source) and depth:
            if mask[end]:
                if source[end] == '{':
                    depth += 1
                elif source[end] == '}':
                    depth -= 1
            end += 1
        if depth:
            raise ValueError('unterminated Rust test module')
        ranges.append((match.start(), end))
    return ranges


def rust_test_text(source: str) -> str:
    """Text within complete cfg(test) inline modules, never the trailing file."""
    return '\n'.join(source[start:end] for start, end in test_module_ranges(source))


def rust_production_text(source: str) -> str:
    """Preserve positions and every source byte outside known test modules."""
    output = list(source)
    for start, end in test_module_ranges(source):
        for i in range(start, end):
            if output[i] != '\n':
                output[i] = ' '
    return ''.join(output)
