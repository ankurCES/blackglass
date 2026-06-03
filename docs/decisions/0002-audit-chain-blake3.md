# ADR 0002: Audit chain hash = blake3

- Status: Accepted (sub-plan 1)
- Context: spec §1.3 says "hash-chained JSONL audit log"; algorithm not specified.
- Decision: blake3 (32-byte digest). Each line: `{event, hash}` where `hash = blake3(canonical_json(event))` and `event.prev_hash` references the previous line's `hash`.
- Consequences:
  - Pure-Rust, no OpenSSL dep for hashing.
  - 64-char hex fits in a column.
  - Not a "FIPS-approved" hash; acceptable for an operator's own box per spec §8.5 risk 19.
- Alternatives: SHA-256 (kept as the hash for captured *output* per spec §1.3 success #3; not the chain hash), SHA-3 (slower, no benefit).
