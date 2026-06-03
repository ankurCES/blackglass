# ADR 0011: New audit event kinds for Gate 3

- Status: Accepted (sub-plan 3)
- Context: sub-plan 1 ships 5 EventKind variants; Gate 3 needs 2 more.
- Decision: add `OperatorConfirmationRequested` and `OperatorConfirmationResolved`. Both carry `id` (UUID), `request_id` (originating JSON-RPC id from runtime socket), and class-specific fields. See spec §6.4.
- Consequences: existing chokepoint test (3 events, read_only) stays. New test (5 events, destructive+allow) added.
- Alternatives: collapse into ActionAllowed/ActionDenied with confirmation field (rejected: less self-describing).
