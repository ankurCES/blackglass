"""scapy offline packet crafting.

The Rust bridge already enforces offline-only (rejects `send(`/`sr(` in
the spec string), so this module is a thin wrapper that evals the spec
and serializes the result.

Returns:
    dict with keys `bytes_hex` (str) and `length` (int).
"""

from scapy.all import IP, TCP, UDP, Raw  # type: ignore  # noqa: F401
from scapy.packet import Packet

from .audit_types import ScapyResult


def craft(spec: str) -> dict:
    """Craft an offline scapy packet from a spec string.

    The spec is eval'd in a sandboxed namespace that exposes scapy's
    common layer constructors (IP, TCP, UDP, Raw, Ether, etc.). Live
    TX functions (send, sr, sr1) are NOT exposed.

    Example spec:
        'IP(dst="10.0.0.5")/TCP(dport=80)/Raw(load="GET / HTTP/1.0")'
    """
    ns = {
        "IP": IP, "TCP": TCP, "UDP": UDP, "Raw": Raw,
        # A few more common ones:
        "Ether": __import__("scapy.all", fromlist=["Ether"]).Ether,
        "DNS": __import__("scapy.all", fromlist=["DNS"]).DNS,
        "ICMP": __import__("scapy.all", fromlist=["ICMP"]).ICMP,
    }
    pkt: Packet = eval(spec, {"__builtins__": {}}, ns)  # noqa: S307 — sandboxed
    raw = bytes(pkt)
    return ScapyResult(bytes_hex=raw.hex(), length=len(raw)).to_dict()
