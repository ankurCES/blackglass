"""Hardware bridge — Flipper, evilginx2, gophish.

Each function takes the args the Rust bridge passes and returns a dict.
evilginx2 and gophish talk to their respective services over HTTP."""

import base64
from .audit_types import EvilginxResult, FlipperResult, GophishResult


# --- Flipper ---

def _flipper_list(path):
    # Lazy import: pyflipper is only needed for actual hardware.
    from pyflipper import PyFlipper  # type: ignore
    pf = PyFlipper()
    files = pf.storage.list(path)
    return FlipperResult(op="list", data=",".join(files)).to_dict()


def _flipper_read(path):
    from pyflipper import PyFlipper  # type: ignore
    pf = PyFlipper()
    content = pf.storage.read(path)
    return FlipperResult(op="read", data=content).to_dict()


def _flipper_write(path, data_b64):
    from pyflipper import PyFlipper  # type: ignore
    pf = PyFlipper()
    data = base64.b64decode(data_b64)
    pf.storage.write(path, data)
    return FlipperResult(op="write", data="ok").to_dict()


def _flipper_run(command):
    from pyflipper import PyFlipper  # type: ignore
    pf = PyFlipper()
    output = pf.cli.run(command)
    return FlipperResult(op="run", data=output).to_dict()


def flipper_run(op: dict) -> dict:
    op_name = op.get("op", "")
    if op_name == "list":
        return _flipper_list(op["path"])
    if op_name == "read":
        return _flipper_read(op["path"])
    if op_name == "write":
        return _flipper_write(op["path"], op["data_b64"])
    if op_name == "run":
        return _flipper_run(op["command"])
    raise ValueError(f"unknown flipper op: {op_name!r}")


# --- evilginx2 ---

def _evilginx_admin_request(path, method="GET", data=None):
    import requests
    base = "http://127.0.0.1:8080"  # evilginx2 admin API
    r = requests.request(method, f"{base}{path}", json=data, timeout=10)
    r.raise_for_status()
    return r.json() if r.content else {}


def _evilginx_list():
    return EvilginxResult(op="list", data=_evilginx_admin_request("/api/phishlets")).to_dict()


def _evilginx_enable(phishlet):
    return EvilginxResult(
        op="enable",
        data=_evilginx_admin_request(f"/api/phishlets/{phishlet}/enable", method="POST"),
    ).to_dict()


def _evilginx_disable(phishlet):
    return EvilginxResult(
        op="disable",
        data=_evilginx_admin_request(f"/api/phishlets/{phishlet}/disable", method="POST"),
    ).to_dict()


def _evilginx_get_captures():
    return EvilginxResult(op="get_captures", data=_evilginx_admin_request("/api/captures")).to_dict()


def _evilginx_lure_create(phishlet, path):
    return EvilginxResult(
        op="lure_create",
        data=_evilginx_admin_request("/api/lures", method="POST", data={"phishlet": phishlet, "path": path}),
    ).to_dict()


def evilginx_run(op: dict) -> dict:
    op_name = op.get("op", "")
    if op_name == "list":
        return _evilginx_list()
    if op_name == "enable":
        return _evilginx_enable(op["phishlet"])
    if op_name == "disable":
        return _evilginx_disable(op["phishlet"])
    if op_name == "get_captures":
        return _evilginx_get_captures()
    if op_name == "lure_create":
        return _evilginx_lure_create(op["phishlet"], op["path"])
    raise ValueError(f"unknown evilginx op: {op_name!r}")


# --- gophish ---

def _gophish_call(method, path, data=None):
    # The `gophish` PyPI client has its own `Gophish` class. For v1 we
    # use raw requests because the client is small and unstable.
    import requests
    base = "https://127.0.0.1:3333"  # default gophish admin port
    # In production the API key comes from /etc/blackglass/gophish.key
    headers = {"Authorization": "Bearer placeholder"}
    r = requests.request(method, f"{base}{path}", json=data, headers=headers, verify=False, timeout=10)
    r.raise_for_status()
    return r.json() if r.content else {}


def gophish_run(op: dict) -> dict:
    op_name = op.get("op", "")
    if op_name == "campaign_list":
        return GophishResult(op="campaign_list", data=_gophish_call("GET", "/api/campaigns/")).to_dict()
    if op_name == "campaign_create":
        return GophishResult(
            op="campaign_create",
            data=_gophish_call("POST", "/api/campaigns/", data={
                "name": op["name"], "template": {"name": op["template"]},
                "url": op["url"], "groups": [{"name": g} for g in op["groups"]],
            }),
        ).to_dict()
    if op_name == "campaign_status":
        return GophishResult(op="campaign_status", data=_gophish_call("GET", f"/api/campaigns/{op['id']}")).to_dict()
    if op_name == "results":
        return GophishResult(op="results", data=_gophish_call("GET", f"/api/campaigns/{op['id']}/results")).to_dict()
    raise ValueError(f"unknown gophish op: {op_name!r}")
