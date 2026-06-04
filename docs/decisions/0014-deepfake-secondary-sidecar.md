# ADR 0014: Run the deepfake detection model as a separate sidecar process

- Status: Accepted (sub-plan 4)
- Context: deepfake detection (image + video + audio) needs a PyTorch model (~800 MB on disk, ~2 GB resident). The main sidecar `blackglass_sidecar` is meant to be small and fast — scapy, impacket, pyflipper, gophish. Bundling PyTorch in the same venv would (a) bloat the main .deb from ~50 MB to ~1 GB, (b) make every `uv pip install` of the main sidecar pull in torch + CUDA libraries, and (c) make it impossible to run the rest of blackglass on machines without a working torch wheel. The detection capability is also semantically distinct — it is read-only classification, not active auditing tooling — so it has a different security/operations profile.
- Decision: ship a **secondary sidecar** as a separate process. It has its own venv, its own systemd unit, and its own .deb. It exposes a REST endpoint on `http://127.0.0.1:8511/detect` (and `/detect/video`, `/detect/batch`). The main sidecar does **not** import it; the `mcp-detect` crate (and the chokepoint's `python_bridge`) call it over HTTP via the `requests` library. v1 may ship with a placeholder model that returns `"verdict": "unknown", "confidence": 0.0`; the wire format is the same so the real model can drop in without code changes elsewhere.
- Consequences:
  - The main .deb stays small (~50 MB). The optional `blackglass-detect` meta-package pulls in the secondary sidecar's .deb.
  - One more process to manage, but it follows the same AppArmor profile pattern as the main sidecar and is auto-started by a systemd user unit.
  - v1 users without a GPU get a working "unknown" classification; a real model ships in v1.1.
  - The HTTP boundary is auditable — the main sidecar logs every request to the detector in the audit chain as `PythonBridgeInvoked{module: "blackglass_sidecar.detect_bridge"}`, and the detector's own stderr is captured for the audit's `bridge.stderr` field.
  - The chokepoint's bridge allow-list (`is_safe_module`) includes `blackglass_sidecar.detect_bridge` so even if the secondary sidecar is unreachable, the wire call still produces a clean `BridgeError::Runtime` rather than a missing-module panic.
- Alternatives:
  - Bundle torch in the main sidecar (rejected: ~1 GB .deb, slow `uv pip install`, blocks dev/CI machines without torch wheels).
  - Use an out-of-process model server like Triton or vLLM (rejected: too heavy for v1, requires GPU drivers, complicates packaging).
  - Call the model directly via subprocess `python -m blackglass_detect` (rejected: cold-start latency per call; the model has to load every time, no in-process state).
