# ADR 0012: Dev build = `cargo tauri dev`; full .deb deferred

- Status: Accepted (sub-plan 3)
- Context: spec §7 describes a full .deb with AppArmor, polkit, udev, cosign, .desktop. That's a packaging sub-plan.
- Decision: Sub-plan 3 ships only `cargo tauri dev` and `cargo tauri build` (Tauri-only .AppImage + .deb). Full blackglass system package is a packaging sub-plan.
- Consequences: developers run the UI today; manual e2e (Task 16) is reproducible on any Linux box with cargo.
- Alternatives: do the full .deb now (rejected: ~2x the plan, requires Ubuntu 24.04 VM).
