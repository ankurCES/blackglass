"""Blackglass secondary sidecar — deepfake detection.

v1 is a placeholder that returns "unknown" for everything. v1.1 will
load a real model (MesoNet, FaceForensics++, or similar) and return a
proper verdict with confidence. The wire format is stable across
versions so the main sidecar (which calls us over HTTP) doesn't
change."""
__version__ = "0.1.0"
