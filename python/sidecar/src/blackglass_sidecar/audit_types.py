"""Dataclasses mirroring the Rust bridge types. Used for type hints only;
the actual wire format is what `craft`, `run`, etc. return — dicts with
the keys the Rust side expects."""

from dataclasses import dataclass, field
from typing import Optional


@dataclass
class ScapyResult:
    bytes_hex: str
    length: int

    def to_dict(self) -> dict:
        return {"bytes_hex": self.bytes_hex, "length": self.length}


@dataclass
class ImpacketResult:
    op: str
    stdout: str
    stderr: str
    hashes: list[str] = field(default_factory=list)

    def to_dict(self) -> dict:
        return {
            "op": self.op,
            "stdout": self.stdout,
            "stderr": self.stderr,
            "hashes": self.hashes,
        }


@dataclass
class FlipperResult:
    op: str
    data: str

    def to_dict(self) -> dict:
        return {"op": self.op, "data": self.data}


@dataclass
class EvilginxResult:
    op: str
    data: dict

    def to_dict(self) -> dict:
        return {"op": self.op, "data": self.data}


@dataclass
class GophishResult:
    op: str
    data: dict

    def to_dict(self) -> dict:
        return {"op": self.op, "data": self.data}


@dataclass
class DetectResult:
    op: str
    verdict: str  # "unknown" | "likely_real" | "likely_fake" | "inconclusive"
    confidence: float
    raw: dict

    def to_dict(self) -> dict:
        return {
            "op": self.op,
            "verdict": self.verdict,
            "confidence": self.confidence,
            "raw": self.raw,
        }
