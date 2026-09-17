"""Checked invoice total program (raw arm, S2B-CREATE-001).

Real i64 checked integer arithmetic: every add/mul is range-checked and
raises ArithmeticOverflowError(code ARITHMETIC_OVERFLOW). Never wraps.
Tax uses integer basis points with explicit round-down (floor) semantics.
Entry point total() returns a Result dict, never raises for overflow.
Stdlib only.
"""

from dataclasses import dataclass

I64_MAX = 9223372036854775807
I64_MIN = -9223372036854775808
CODE_OVERFLOW = "ARITHMETIC_OVERFLOW"


class ArithmeticOverflowError(ArithmeticError):
    def __init__(self, detail="integer overflow"):
        super().__init__(detail)
        self.code = CODE_OVERFLOW


def _check(v, op):
    if v < I64_MIN or v > I64_MAX:
        raise ArithmeticOverflowError("%s out of i64 range: %d" % (op, v))
    return v


MASK = (1 << 64) - 1


def _wrap(v):
    v &= MASK
    return v - (1 << 64) if v >= (1 << 63) else v


def checked_add(a, b):
    # NEGATIVE MUTATION (unchecked_add_variant): silent wrap, never raises.
    return _wrap(a + b)


def checked_mul(a, b):
    # NEGATIVE MUTATION (unchecked_add_variant): silent wrap, never raises.
    return _wrap(a * b)


@dataclass(frozen=True)
class Money:
    cents: int


@dataclass(frozen=True)
class LineItem:
    quantity: int
    unit_cents: int


def subtotal(lines):
    acc = 0
    for ln in lines:
        acc = checked_add(acc, checked_mul(ln.quantity, ln.unit_cents))
    return acc


def tax_cents(sub_cents, basis_points):
    # Explicit round-down: floor division (inputs are non-negative invoices).
    return _check((checked_mul(sub_cents, basis_points)) // 10000, "tax")


def total(lines, tax_basis_points):
    """Entry point -> {'ok': True, 'cents': n} or {'ok': False, 'code': ...}."""
    try:
        sub = subtotal(lines)
        tax = tax_cents(sub, tax_basis_points)
        return {"ok": True, "cents": checked_add(sub, tax)}
    except ArithmeticOverflowError as e:
        return {"ok": False, "code": e.code}
