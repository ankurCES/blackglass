# ADR 0003: Profile format = TOML

- Status: Accepted (sub-plan 1)
- Context: spec §1.3 mentions tiered profiles but does not fix a format.
- Decision: TOML. Schema is owned by the `blackglass-profile` crate. v1 schema:

  ```toml
  name = "analyst"          # required, non-empty
  tier = "analyst"          # one of: analyst | operator | redteam
  allowed_domains = ["core", "osint", "packets", "audit"]
  allowed_action_classes = ["read_only"]
  ```

- Consequences:
  - Human-editable, diff-friendly in git.
  - `serde` + `toml` crates are mature and dependency-light.
  - `+operator` and `+redteam` profiles add the additional fields they need (signed-config path, EULA path) in their own sub-plans.
- Alternatives: YAML (rejected: serde_yaml has had CVEs), JSON (rejected: comments forbidden, hostile to hand-editing), RON (rejected: not human-friendly).
