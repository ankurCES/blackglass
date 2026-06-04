"""Impacket helpers — 5 ad-* operations.

Each function takes the args the Rust bridge passes and returns an
ImpacketResult dict. We import lazily (inside each function) to keep
startup time low; impacket is heavy."""

from .audit_types import ImpacketResult


def _psexec(target, user, hash, remote_cmd):
    # The real impl: build a PSEXEC command, run it, capture output.
    # For now, return a placeholder result; the real wire-up is in the
    # impacket integration test (Task 1.10).
    # The import is lazy and best-effort: impacket 0.11+ stopped
    # exporting `psexec` from `impacket.examples`, but we still want
    # import to succeed so the bridge can be exercised.
    try:
        from impacket.examples import psexec  # type: ignore  # noqa: F401
    except ImportError:
        pass
    return ImpacketResult(
        op="impacket_psexec",
        stdout=f"psexec placeholder target={target} user={user} cmd={remote_cmd}",
        stderr="",
    ).to_dict()


def _wmiexec(target, user, hash, remote_cmd):
    return ImpacketResult(
        op="impacket_wmiexec",
        stdout=f"wmiexec placeholder target={target} user={user} cmd={remote_cmd}",
        stderr="",
    ).to_dict()


def _secretsdump(target, user, hash):
    return ImpacketResult(
        op="impacket_secretsdump",
        stdout=f"secretsdump placeholder target={target} user={user}",
        stderr="",
        hashes=[],
    ).to_dict()


def _kerberoast(target, user, hash):
    return ImpacketResult(
        op="impacket_kerberoast",
        stdout=f"kerberoast placeholder target={target} user={user}",
        stderr="",
    ).to_dict()


def _asreproast(target, user, hash):
    return ImpacketResult(
        op="impacket_asreproast",
        stdout=f"asreproast placeholder target={target} user={user}",
        stderr="",
    ).to_dict()


def run(op: dict) -> dict:
    """Dispatch a single impacket operation. `op` has the tag from the
    Rust enum (Psexec, Wmiexec, etc.)."""
    op_name = op.get("op", "")
    if op_name == "psexec":
        return _psexec(op["target"], op["user"], op["hash"], op["remote_cmd"])
    if op_name == "wmiexec":
        return _wmiexec(op["target"], op["user"], op["hash"], op["remote_cmd"])
    if op_name == "secretsdump":
        return _secretsdump(op["target"], op["user"], op["hash"])
    if op_name == "kerberoast":
        return _kerberoast(op["target"], op["user"], op["hash"])
    if op_name == "asreproast":
        return _asreproast(op["target"], op["user"], op["hash"])
    raise ValueError(f"unknown impacket op: {op_name!r}")
