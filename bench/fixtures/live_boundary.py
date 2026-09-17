"""Narrow path admission shared by raw fixture oracles and live campaigns."""

from __future__ import annotations

import os
from pathlib import Path


LIVE_CANDIDATE_ENV = "SLEY2_LIVE_ORACLE_CANDIDATE"


class BoundaryError(ValueError):
    """The requested oracle candidate is outside its admitted boundary."""


def _fail(detail: str) -> None:
    raise BoundaryError(f"LIVE_ORACLE_PATH_REFUSED: {detail}")


def resolve_candidate(task_directory: Path, argument: str | Path) -> tuple[Path, str, str | None]:
    """Resolve a frozen control or the exact live candidate named by the runner.

    The environment value is an exact directory capability, not a parent
    directory. Children, siblings, lexical aliases, and symlinks are refused.
    """

    task = Path(task_directory).resolve()
    requested = Path(argument)
    if requested.is_absolute():
        local_source = requested
    else:
        cwd_candidate = Path.cwd() / requested
        local_source = cwd_candidate if cwd_candidate.exists() else task / requested
    local = local_source.resolve()
    try:
        relative = local.relative_to(task)
    except ValueError:
        relative = None
    if relative is not None:
        if relative.parts == ("fixture",) and local.is_dir():
            return local, "positive", None
        if len(relative.parts) == 2 and relative.parts[0] == "negative" and local.is_dir():
            return local, "negative", relative.parts[1]
        _fail("local role")

    admitted_text = os.environ.get(LIVE_CANDIDATE_ENV)
    if not admitted_text or "\x00" in admitted_text:
        _fail("not admitted")
    admitted_lexical = Path(admitted_text)
    if not admitted_lexical.is_absolute():
        _fail("capability is not absolute")
    admitted_absolute = Path(os.path.abspath(admitted_lexical))
    try:
        admitted = admitted_lexical.resolve(strict=True)
    except OSError as error:
        raise BoundaryError(f"LIVE_ORACLE_PATH_REFUSED: {error}") from error
    if admitted_absolute != admitted or not admitted.is_dir():
        _fail("capability aliases a path")
    requested_absolute = Path(os.path.abspath(Path.cwd() / requested if not requested.is_absolute() else requested))
    if requested_absolute != admitted:
        _fail("candidate does not equal capability")
    try:
        if requested_absolute.is_symlink() or requested_absolute.resolve(strict=True) != admitted:
            _fail("candidate aliases a path")
    except OSError as error:
        raise BoundaryError(f"LIVE_ORACLE_PATH_REFUSED: {error}") from error
    return admitted, "positive", None
