# ADR 0010: Gate 3 wire format

- Status: Accepted (sub-plan 3)
- Context: Gate 3 needs chokepoint → Tauri-app confirmation with up to 15s wait.
- Decision: server-pushed `confirm.request` event on the operator socket. Tauri app responds with `confirm.resolve` JSON-RPC. UUID v4 confirmation id. 15s default timeout (200ms in test mode). 6-value `decision` field: `allow | allow_and_remember | deny | timeout | disconnected | deny_late`. Default on timeout/disconnect: deny. See spec §4.3 + §6.2.
- Consequences: chokepoint's `.await` is the single source of truth. Late `confirm.resolve` → second `OperatorConfirmationResolved{decision: "deny_late"}` event but no follow-up `ActionDenied`.
- Alternatives: short-string ids (rejected: collision risk), absolute timestamps (rejected: clock skew).
