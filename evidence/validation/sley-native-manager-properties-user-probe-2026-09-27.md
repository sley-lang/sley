# Native manager property probe — unprivileged transient services

- Host: greyarch, systemd 261 (261.2-1-arch), 2026-09-27.
- The system manager's `org.freedesktop.systemd1.Unit` and `.Service`
  `GetAll` responses were read with `busctl --json=short`. They returned
  typed values; for example, `TimeoutStopUSec` was an unsigned 64-bit integer
  while `systemctl show` rendered it as `5s`.
- A live user transient service installed `LoadCredential=sley-input:<source>`,
  `BindReadOnlyPaths=/usr/bin/sleep`, and
  `TemporaryFileSystem=/tmp:ro`. D-Bus reported `LoadCredential` as
  `a(ss)` with the exact name/source pair; `BindReadOnlyPaths` as `a(ssbt)`
  with source and destination `/usr/bin/sleep`, ignore-missing `false`, and
  flags `16384`; and `TemporaryFileSystem` as `a(ss)` with `/tmp` and `ro`.
- Its `ExecStart` value was `a(sasbttttuii)`. Path, argv, and the ignore-failure
  boolean were stable, while timestamps and PID were populated at runtime.
  The verifier checks the stable launch fields and obtains `MainPID`
  separately.
- A separate user service with
  `TemporaryFileSystem=/run/sley-scratch:ro` failed with
  `status=226/NAMESPACE`: systemd reported that `/run/sley-scratch` did not
  exist. That unnecessary mount was removed from the rendered worker unit.

The probe used the user manager. It validates the local D-Bus value shapes and
the missing mount-target failure, but does not qualify the privileged system
service or native test admission. The root supervisor is still unwired.

## Reap-state follow-up

A short-lived user transient service disappeared from `GetUnit` after normal
exit; `ListUnitsByPatterns` returned an empty typed unit list. A second user
service killed with `systemctl kill --kill-whom=all --signal=SIGKILL` remained
loaded as `ActiveState=failed`, while its typed Service properties reported
`MainPID=0` and `ControlGroup=""`. The reap checker therefore accepts either
an absent unit or that verified terminal manager state, and additionally
requires the exact cgroup path to be absent. This remains a user-manager
probe, not a system-manager qualification.
