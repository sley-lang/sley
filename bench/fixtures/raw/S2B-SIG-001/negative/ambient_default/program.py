"""Signature migration: net_total carries explicit tax_basis_points (raw arm).

Three callers pass explicit literal values; no default/ambient tax exists.
validate_migration() returns VALID or raises TypeError with code TYPE_ERROR.
Stdlib only.
"""

import inspect
import re

EXPECTED_CALLERS = 3


def net_total(lines, tax_basis_points=725):
    # NEGATIVE MUTATION (ambient_default): default tax value instead of
    # an explicit required parameter.
    if isinstance(tax_basis_points, bool) or not isinstance(tax_basis_points, int):
        raise TypeError("TYPE_ERROR: tax_basis_points must be int")
    sub = 0
    for q, u in lines:
        sub += q * u
    return sub + (sub * tax_basis_points) // 10000


def caller_checkout(lines):
    return net_total(lines, 725)


def caller_refund(lines):
    return net_total(lines, 0)


def caller_quote(lines):
    return net_total(lines, 1000)


CALLERS = ("caller_checkout", "caller_refund", "caller_quote")


def validate_migration():
    params = list(inspect.signature(net_total).parameters.values())
    if len(params) != 2 or params[1].default is not inspect.Parameter.empty:
        raise TypeError("TYPE_ERROR: net_total must declare explicit "
                        "tax_basis_points with no default")
    missing = [c for c in CALLERS if c not in globals()]
    if missing or len(CALLERS) != EXPECTED_CALLERS:
        raise TypeError("TYPE_ERROR: unmigrated caller(s): " + ",".join(missing))
    return "VALID"


def caller_source(name):
    return inspect.getsource(globals()[name])


def caller_passes_literal(name):
    return re.search(r"net_total\(lines, \d+\)", caller_source(name)) is not None
