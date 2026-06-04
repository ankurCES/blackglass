"""Integration test: blackglass_sidecar.detect_bridge calls the secondary
sidecar correctly. Boots a real uvicorn on 127.0.0.1:8511 and exercises
all 3 functions + the failure path.

Run with:
    /tmp/sidecar-venv/bin/python -m pytest python/sidecar/tests/test_detect_bridge_integration.py -v

This file lives next to the sidecar (not in crates/) because it tests
Python, not Rust — Rust coverage of the detect route is in
crates/core/tests/end_to_end_python_bridge.rs and
crates/python-bridge/tests/bridge.rs."""

from __future__ import annotations

import multiprocessing
import socket
import time

import pytest
import uvicorn

from blackglass_sidecar import detect_bridge


def _free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def _run_uvicorn(port: int) -> None:
    # Import inside the child so the module is re-imported cleanly.
    from blackglass_secondary.server import app

    uvicorn.run(app, host="127.0.0.1", port=port, log_level="error")


@pytest.fixture()
def secondary_url():
    """Boot the secondary sidecar on a free loopback port for the
    duration of one test. Yields the base URL."""
    port = _free_port()
    proc = multiprocessing.Process(target=_run_uvicorn, args=(port,), daemon=True)
    proc.start()
    base = f"http://127.0.0.1:{port}"
    # Wait for /healthz to respond (max 10s).
    import requests
    deadline = time.time() + 10
    while time.time() < deadline:
        try:
            r = requests.get(f"{base}/healthz", timeout=0.5)
            if r.status_code == 200:
                break
        except requests.exceptions.RequestException:
            pass
        time.sleep(0.1)
    else:
        proc.terminate()
        pytest.fail(f"secondary sidecar did not start on {base}")
    yield base
    proc.terminate()
    proc.join(timeout=2)
    if proc.is_alive():
        proc.kill()


def test_image_routes_to_secondary(secondary_url, monkeypatch):
    monkeypatch.setenv("BLACKGLASS_SECONDARY_URL", secondary_url)
    out = detect_bridge.image("/tmp/fake.png")
    assert out["op"] == "detect-image"
    assert out["verdict"] == "unknown"
    assert out["confidence"] == 0.0
    assert out["raw"]["path"] == "/tmp/fake.png"
    assert out["raw"]["model"] == "placeholder-v1"


def test_video_routes_to_secondary(secondary_url, monkeypatch):
    monkeypatch.setenv("BLACKGLASS_SECONDARY_URL", secondary_url)
    out = detect_bridge.video("/tmp/fake.mp4")
    assert out["op"] == "detect-video"
    assert out["verdict"] == "unknown"
    assert out["raw"]["path"] == "/tmp/fake.mp4"


def test_batch_routes_to_secondary(secondary_url, monkeypatch):
    monkeypatch.setenv("BLACKGLASS_SECONDARY_URL", secondary_url)
    out = detect_bridge.batch("/tmp/somedir")
    assert out["op"] == "detect-batch"
    assert out["verdict"] == "unknown"
    assert out["raw"]["dir"] == "/tmp/somedir"


def test_returns_unknown_when_server_down(monkeypatch):
    """If the secondary sidecar is not reachable we must NOT raise —
    we return verdict=unknown with the error captured in raw. The
    chokepoint relies on this graceful failure to emit
    PythonBridgeFailed cleanly."""
    # Pick a port that is almost certainly not bound.
    monkeypatch.setenv("BLACKGLASS_SECONDARY_URL", "http://127.0.0.1:1")
    out = detect_bridge.image("/tmp/anything.png")
    assert out["op"] == "detect-image"
    assert out["verdict"] == "unknown"
    assert "error" in out["raw"]
    assert "not reachable" in out["raw"]["error"]
