"""FastAPI server for the secondary sidecar. Listens on 127.0.0.1:8511.

The main sidecar (and the chokepoint, via the bridge) calls these
endpoints over HTTP. The service is bound to localhost only — it
should not be reachable from the network. The systemd unit and the
AppArmor profile enforce this at the OS level; the bind address is
the in-process defense-in-depth.

Run directly for development:
    uvicorn blackglass_secondary.server:app --host 127.0.0.1 --port 8511

The Rust launcher (`blackglass-secondary-sidecar`) spawns uvicorn as
a child process and forwards signals.
"""

from fastapi import FastAPI

from . import detect
from . import __version__ as PKG_VERSION

app = FastAPI(title="blackglass-secondary", version=PKG_VERSION)


@app.get("/healthz")
def healthz() -> dict:
    return {"ok": True, "version": PKG_VERSION}


@app.post("/detect/image")
def detect_image_endpoint(body: dict) -> dict:
    return detect.detect_image(body["path"])


@app.post("/detect/video")
def detect_video_endpoint(body: dict) -> dict:
    return detect.detect_video(body["path"])


@app.post("/detect/batch")
def detect_batch_endpoint(body: dict) -> dict:
    return detect.detect_batch(body["dir"])
