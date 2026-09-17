"""Checked division boundary tests (raw arm, S2B-TEST-001).

Three canonical cases with exact failure codes; implementation root facts
(IMPL_DIGEST) unchanged except the test-set binding. Stdlib only.
"""

import hashlib

I64_MAX = 9223372036854775807
I64_MIN = -9223372036854775808
CODE_DIV_ZERO = "DIVIDE_BY_ZERO"
CODE_OVERFLOW = "ARITHMETIC_OVERFLOW"

IMPL_RECORD = ("checked_div:v1:floor:zero->DIVIDE_BY_ZERO:"
               "min/-1->ARITHMETIC_OVERFLOW")
IMPL_DIGEST = hashlib.sha256(IMPL_RECORD.encode()).hexdigest()
TEST_BINDING = {"version": 1,
                # NEGATIVE MUTATION (case_missing): signed_overflow dropped.
                "cases": ["success", "divide_by_zero"]}
REQUIRED_CASES = ["success", "divide_by_zero", "signed_overflow"]


class SignedOverflowError(ArithmeticError):
    def __init__(self, detail="signed overflow"):
        super().__init__(detail)
        self.code = CODE_OVERFLOW


def checked_div(a, b):
    if b == 0:
        raise ZeroDivisionError("DIVIDE_BY_ZERO: division by zero")
    if a == I64_MIN and b == -1:
        raise SignedOverflowError("signed overflow: min / -1")
    q = abs(a) // abs(b)
    if (a < 0) != (b < 0):
        q = -q
    if q < I64_MIN or q > I64_MAX:
        raise SignedOverflowError("quotient out of range")
    return q
