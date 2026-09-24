#!/usr/bin/env python3
"""In-sandbox loopback forwarder for the confined live provider.

Stdlib only. Bound read-only into every provider sandbox and run as the
sandbox's first process:

    python3 egress_forwarder.py SOCKET PORT AGENT [ARGS...]

It listens on 127.0.0.1:PORT (the sandbox network namespace has no other
interface), then forks: the child relays each loopback connection to the
runner-side egress proxy's unix SOCKET; the parent replaces itself with
the agent command, so the agent keeps the sandbox's stdin, stdout, exit
status, and process identity. The relay admits only connections held by
that agent process (model-issued commands inherit the proxy variables but
are refused here); the proxy, not this relay, decides which hosts are
reachable. When the agent exits the sandbox init tears the
relay down with the rest of the namespace.
"""

from __future__ import annotations

import os
import select
import socket
import sys
import threading


def _pump(left: socket.socket, right: socket.socket) -> None:
    try:
        while True:
            ready, _, _ = select.select([left, right], [], [], 900)
            if not ready:
                return
            for source in ready:
                data = source.recv(65536)
                if not data:
                    return
                (right if source is left else left).sendall(data)
    except OSError:
        return
    finally:
        for end in (left, right):
            try:
                end.close()
            except OSError:
                pass


def _socket_inode(local_port: int, remote_port: int) -> str | None:
    """Inode of the loopback client socket 127.0.0.1:local -> :remote."""

    want_local = f"0100007F:{local_port:04X}"
    want_remote = f"0100007F:{remote_port:04X}"
    try:
        with open("/proc/net/tcp", encoding="ascii") as handle:
            next(handle, None)
            for line in handle:
                fields = line.split()
                if len(fields) > 9 and fields[1] == want_local and fields[2] == want_remote:
                    return fields[9]
    except OSError:
        return None
    return None


def _holders(inode: str) -> set[int]:
    marker = f"socket:[{inode}]"
    found: set[int] = set()
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        directory = f"/proc/{entry}/fd"
        try:
            names = os.listdir(directory)
        except OSError:
            continue
        for name in names:
            try:
                if os.readlink(f"{directory}/{name}") == marker:
                    found.add(int(entry))
                    break
            except OSError:
                continue
    return found


def _from_agent(client: socket.socket, port: int, agent: int) -> tuple[bool, str]:
    """Only the provider process itself may use the egress route: a
    connection held by any other process (a model-issued command, which
    inherits the proxy variables) is refused. Threads share the provider's
    descriptor table, so its own API connections are always admitted."""

    peer_port = client.getpeername()[1]
    inode = _socket_inode(peer_port, port)
    if inode is None:
        return False, "no-socket"
    holders = _holders(inode)
    if agent in holders:
        return True, ""
    return False, ",".join(str(pid) for pid in sorted(holders)) or "none"


def _refuse(target: str, client: socket.socket, detail: str) -> None:
    # Tell the runner-side proxy (its decision log is outside the sandbox),
    # then close the client with a refusal.
    try:
        note = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        note.settimeout(5)
        note.connect(target)
        note.sendall(f"LOCAL-DENY non-provider-process {detail} HTTP/1.1\r\n\r\n".encode())
        note.recv(64)
        note.close()
    except OSError:
        pass
    try:
        client.sendall(b"HTTP/1.1 403 Forbidden\r\n\r\n")
        client.close()
    except OSError:
        pass


def _relay(listener: socket.socket, target: str, port: int, agent: int | None) -> None:
    while True:
        try:
            client, _ = listener.accept()
        except OSError:
            return
        if agent is not None:
            admitted, detail = _from_agent(client, port, agent)
            if not admitted:
                _refuse(target, client, detail)
                continue
        upstream = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            upstream.connect(target)
        except OSError:
            client.close()
            upstream.close()
            continue
        threading.Thread(target=_pump, args=(client, upstream),
                         daemon=True).start()


def main(argv: list[str]) -> int:
    if len(argv) < 4 or not argv[1] or not argv[2].isdigit() or not argv[3]:
        sys.stderr.write("egress_forwarder: usage SOCKET PORT AGENT [ARGS...]\n")
        return 97
    target, port = argv[1], int(argv[2])
    listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        listener.bind(("127.0.0.1", port))
        listener.listen(64)
    except OSError as error:
        sys.stderr.write(f"egress_forwarder: listen: {error}\n")
        return 97
    child = os.fork()
    if child == 0:
        # The relay never holds the agent's stdio: the runner reads the
        # provider streams to EOF, so a stray holder would stall it.
        null = os.open(os.devnull, os.O_RDWR)
        for descriptor in (0, 1, 2):
            try:
                os.dup2(null, descriptor)
            except OSError:
                pass
        _relay(listener, target, port, os.getppid())
        os._exit(0)
    listener.close()
    os.execv(argv[3], argv[3:])
    return 97  # unreachable: execv raises on failure


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
