#!/usr/bin/env python3
"""In-sandbox loopback forwarder for the confined live provider.

Stdlib only. Bound read-only into every provider sandbox and run as the
sandbox's first process:

    python3 egress_forwarder.py SOCKET PORT AGENT [ARGS...]

It listens on 127.0.0.1:PORT (the sandbox network namespace has no other
interface), then forks: the child relays each loopback connection to the
runner-side egress proxy's unix SOCKET; the parent replaces itself with
the agent command, so the agent keeps the sandbox's stdin, stdout, exit
status, and process identity. The proxy, not this relay, decides which
hosts are reachable. When the agent exits the sandbox init tears the
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


def _relay(listener: socket.socket, target: str) -> None:
    while True:
        try:
            client, _ = listener.accept()
        except OSError:
            return
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
        _relay(listener, target)
        os._exit(0)
    listener.close()
    os.execv(argv[3], argv[3:])
    return 97  # unreachable: execv raises on failure


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
