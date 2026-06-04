"""Deepfake detection bridge — calls the secondary sidecar over loopback HTTP.

The secondary sidecar is a separate FastAPI process that owns the ML model
(MesoNet / FaceForensics++ / similar — placeholder v1 returns "unknown").
This module is the thin client: it takes the args the Rust bridge passes,
POSTs to the appropriate /detect/{image,video,batch} endpoint on
127.0.0.1:8511, and packages the JSON response as a DetectResult dict.

The URL is configurable via the BLACKGLASS_SECONDARY_URL env var so tests
and packaging can point at a different bind; the default matches the
launcher in crates/secondary-sidecar/.

If the secondary sidecar is not running we return a DetectResult with
verdict="unknown" and the error captured in `raw` — we do NOT raise, so
the chokepoint can still produce a graceful PythonBridgeFailed event
later. The point of this module is to be best-effort."""

from __future__ import annotations

import os
from typing import Any

import requests

from .audit_types import DetectResult


# 5s is generous for a placeholder v1; if v1.1 brings in torch+CUDA
# inference, bump this. The chokepoint's Gate 3 prompt is on top of
# this so the user is already in the loop before the call lands.
_DEFAULT_TIMEOUT_S = 5.0

# Loopback only by default — matches the AppArmor profile from
# sub-plan 3 Task 3.4.
_DEFAULT_BASE_URL = "http://127.0.0.1:8511"


def _base_url() -> str:
    return os.environ.get("BLACKGLASS_SECONDARY_URL", _DEFAULT_BASE_URL).rstrip("/")


def _post(endpoint: str, payload: dict[str, Any], op: str) -> dict[str, Any]:
    """POST to the secondary sidecar and shape the response.

    On any error (connection refused, timeout, non-200, malformed
    JSON) we return a DetectResult with verdict="unknown" and the
    error string in `raw.error`. The Rust side reads `result`
    directly so we always return a dict, never raise."""
    url = f"{_base_url()}{endpoint}"
    try:
        resp = requests.post(url, json=payload, timeout=_DEFAULT_TIMEOUT_S)
        resp.raise_for_status()
        body = resp.json()
    except requests.exceptions.ConnectionError as e:
        return DetectResult(
            op=op,
            verdict="unknown",
            confidence=0.0,
            raw={"error": f"secondary sidecar not reachable at {url}: {e}", "input": payload},
        ).to_dict()
    except requests.exceptions.Timeout as e:
        return DetectResult(
            op=op,
            verdict="unknown",
            confidence=0.0,
            raw={"error": f"secondary sidecar timed out after {_DEFAULT_TIMEOUT_S}s: {e}", "input": payload},
        ).to_dict()
    except requests.exceptions.HTTPError as e:
        return DetectResult(
            op=op,
            verdict="unknown",
            confidence=0.0,
            raw={"error": f"secondary sidecar returned {resp.status_code}: {e}", "body": getattr(resp, "text", "")},
        ).to_dict()
    except (ValueError, requests.exceptions.RequestException) as e:
        return DetectResult(
            op=op,
            verdict="unknown",
            confidence=0.0,
            raw={"error": f"secondary sidecar call failed: {e}", "input": payload},
        ).to_dict()

    # The sidecar's shape is already {verdict, confidence, raw}. We
    # wrap it in a DetectResult so the `op` is always set, and so the
    # Rust side has a stable schema it can match on.
    return DetectResult(
        op=op,
        verdict=str(body.get("verdict", "unknown")),
        confidence=float(body.get("confidence", 0.0)),
        raw=body.get("raw", body) if isinstance(body, dict) else {"raw": body},
    ).to_dict()


def image(path: str) -> dict[str, Any]:
    """Detect deepfake on a single image file.

    `path` is the on-disk path to the image. The secondary sidecar
    is responsible for reading + decoding it; we just pass it
    through."""
    return _post("/detect/image", {"path": path}, op="detect-image")


def video(path: str) -> dict[str, Any]:
    """Detect deepfake on a single video file. The secondary sidecar
    is expected to sample frames (e.g. 1fps for the placeholder, more
    for v1.1) and aggregate."""
    return _post("/detect/video", {"path": path}, op="detect-video")


def batch(directory: str) -> dict[str, Any]:
    """Detect deepfake on every image/video in `directory`.

    The secondary sidecar walks the dir, classifies each file, and
    returns a single aggregated verdict in v1."""
    return _post("/detect/batch", {"dir": directory}, op="detect-batch")
