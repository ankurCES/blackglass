"""v1 placeholder deepfake detector.

Returns 'unknown' for everything. v1.1 will load a real model
(MesoNet, FaceForensics++, or similar) and return a real verdict
with a non-zero confidence.

The wire format mirrors `blackglass_sidecar.audit_types.DetectResult`:
    {
        "verdict": "unknown" | "likely_real" | "likely_fake" | "inconclusive",
        "confidence": float,  # 0.0 .. 1.0
        "raw": dict,           # model-specific debug info
    }
"""


def detect_image(path: str) -> dict:
    return {
        "verdict": "unknown",
        "confidence": 0.0,
        "raw": {"model": "placeholder-v1", "path": path},
    }


def detect_video(path: str) -> dict:
    return {
        "verdict": "unknown",
        "confidence": 0.0,
        "raw": {"model": "placeholder-v1", "path": path},
    }


def detect_batch(dir: str) -> dict:
    return {
        "verdict": "unknown",
        "confidence": 0.0,
        "raw": {"model": "placeholder-v1", "dir": dir},
    }
