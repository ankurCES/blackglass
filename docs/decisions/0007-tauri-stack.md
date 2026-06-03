# ADR 0007: Tauri 2.x + Svelte 5 + Vite + Tailwind

- Status: Accepted (sub-plan 3)
- Context: spec §6.15 pins Tauri 2.x, GTK webview (WebKitGTK 6.0 on Ubuntu 24.04), Svelte 5 with SvelteKit, TypeScript strict, Tailwind, Svelte stores, Vite.
- Decision: Tauri 2.x, Svelte 5 (runes), Vite, TypeScript strict, Tailwind. **Drift from spec:** (a) WebKitGTK 4.1, not 6.0 — Tauri 2 default; Ubuntu 24.04 stock. (b) Svelte 5 runes, not Svelte 4 stores. (c) No SvelteKit — single-window app does not need its routing layer.
- Consequences: spec §6.15 amendment needed.
- Alternatives: SvelteKit (rejected: overkill), SolidJS/React (rejected: spec says Svelte).
