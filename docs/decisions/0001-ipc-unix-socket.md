# ADR 0001: IPC mechanism = Unix domain socket

- Status: Accepted (sub-plan 1)
- Context: spec §2.2 says "talks to core via socket". TCP localhost was an option.
- Decision: Unix SOCK_STREAM at `~/.local/share/blackglass/runtime.sock`, length-prefixed JSON-RPC frames.
- Consequences:
  - AppArmor can confine socket creation in sub-plan 3.
  - No port-allocation surprise on multi-user boxes.
  - Windows-incompatible (acceptable; not in scope per spec §1.4).
- Alternatives considered: TCP 127.0.0.1:0 with discovery file (rejected: port conflicts in CI), named pipe (rejected: not Linux).
