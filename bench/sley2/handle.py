"""The object an agent adapter receives, and the audit of what it reaches.

This module is deliberately tiny and imports nothing privileged. A bound
method carries its defining module's globals on `__func__.__globals__`, so an
adapter that reflects reaches whatever this module can see. Keeping the
runner's endpoint, subprocess, filesystem, and trace names out of here is the
part of the boundary that is actually enforceable in one process; the rest is
stated as trust rather than claimed as capability. See
`docs/spec/SLEY2_TRIAL_RUNNER_V1.md` section 2.
"""

from __future__ import annotations

from typing import Any, Callable, Mapping

HANDLE_SURFACE = frozenset({"exchange", "affordances"})

# Names whose reachability through `__globals__` would hand a reflecting
# adapter the runner itself rather than merely the conversation.
PRIVILEGED_NAMES = frozenset(
    {
        "Endpoint",
        "Trace",
        "Path",
        "subprocess",
        "Popen",
        "os",
        "sys",
        "tempfile",
        "append_trace_record",
        "open",
    }
)


class EndpointHandle:
    """The only object an agent receives (contract section 2)."""

    __slots__ = ("_exchange", "_affordances")

    def __init__(
        self,
        exchange: Callable[[Mapping[str, Any]], list[dict[str, Any]]],
        affordances: list[str],
    ):
        self._exchange = exchange
        self._affordances = tuple(affordances)

    def exchange(self, request: Mapping[str, Any]) -> list[dict[str, Any]]:
        """Send one method and body; receive the events and the response."""
        return self._exchange(request)

    def affordances(self) -> list[str]:
        """The method names the negotiated profile admits."""
        return list(self._affordances)


def handle_surface(handle: Any) -> set[str]:
    """The declared attributes of a handle.

    This sees declared attributes only. It cannot see what reflection reaches,
    which is why it is not by itself evidence of a capability boundary; pair it
    with `reflected_privileged_names`.
    """
    return {name for name in dir(handle) if not name.startswith("_")}


def reflected_privileged_names(handle: Any) -> set[str]:
    """Privileged names a reflecting adapter reaches from `handle`.

    A bound method exposes its defining module's globals, so this is the part
    of the reflection surface the runner can hold to zero and prove it. The
    exchange closure's own cells are a separate, irreducible residual: the
    callable must hold whatever it needs to answer, and section 2 states that
    rather than denying it.
    """
    method = getattr(type(handle), "exchange", None)
    globals_ = getattr(method, "__globals__", {})
    return {name for name in PRIVILEGED_NAMES if name in globals_}
