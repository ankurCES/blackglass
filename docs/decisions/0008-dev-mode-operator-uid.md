# ADR 0008: Dev mode = operator UID for everything

- Status: Accepted (sub-plan 3)
- Context: spec §2.2 splits the process topology: Tauri app = operator UID, core = root via polkit + AppArmor. Sub-plans 1-2 ship core as whatever UID starts it.
- Decision: Sub-plan 3 keeps that model. Tauri app and core both run as operator UID. Polkit/AppArmor/root — deferred to a packaging sub-plan.
- Consequences: sub-plan 3's `cargo tauri dev` works on any Linux box with cargo. Spec §2.2 amendment needed.
- Alternatives: Implement polkit now (rejected: ~2x the plan, no UI value unlocked).
