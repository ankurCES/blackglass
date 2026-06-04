//! TDD tests for the new PythonBridgeInvoked event kind and the
//! `bridge` field on the action audit payload. See plan §1.1, §1.2.

use blackglass_audit::{Chain, Event, EventKind};
use serde_json::json;
use tempfile::TempDir;

#[test]
fn python_bridge_invoked_round_trips() {
    let kind = EventKind::PythonBridgeInvoked;
    let v = serde_json::to_value(&kind).unwrap();
    assert_eq!(v["kind"], "python_bridge_invoked");
    let back: EventKind = serde_json::from_value(v).unwrap();
    assert_eq!(back, EventKind::PythonBridgeInvoked);
}

#[test]
fn python_bridge_failed_round_trips() {
    let kind = EventKind::PythonBridgeFailed;
    let v = serde_json::to_value(&kind).unwrap();
    assert_eq!(v["kind"], "python_bridge_failed");
    let back: EventKind = serde_json::from_value(v).unwrap();
    assert_eq!(back, EventKind::PythonBridgeFailed);
}

#[test]
fn python_bridge_evidence_dumped_round_trips() {
    let kind = EventKind::PythonBridgeEvidenceDumped;
    let v = serde_json::to_value(&kind).unwrap();
    assert_eq!(v["kind"], "python_bridge_evidence_dumped");
    let back: EventKind = serde_json::from_value(v).unwrap();
    assert_eq!(back, EventKind::PythonBridgeEvidenceDumped);
}

#[test]
fn bridge_field_lands_in_audit_log() {
    let dir = TempDir::new().unwrap();
    let log = dir.path().join("audit.jsonl");
    let mut chain = Chain::open(&log).unwrap();

    // Mimic the chokepoint's PythonBridgeInvoked write.
    chain
        .append(Event {
            seq: 0,
            ts: "2026-06-03T00:00:00Z".into(),
            prev_hash: String::new(),
            kind: EventKind::PythonBridgeInvoked,
            payload: json!({
                "module": "blackglass_sidecar.scapy_bridge",
                "function": "craft",
                "bridge": "scapy_bridge",
                "args": { "layers": ["IP", "TCP"] },
            }),
        })
        .unwrap();

    // Re-open and verify the bridge field is present in the readback.
    let s = std::fs::read_to_string(&log).unwrap();
    assert!(s.contains("\"bridge\":\"scapy_bridge\""), "raw log missing bridge: {s}");
    assert!(s.contains("\"kind\":\"python_bridge_invoked\""));

    // And the query can find it.
    let page = chain
        .query(
            &json!({ "kind": "kind", "kinds": ["python_bridge_invoked"] }),
            0,
            100,
        )
        .unwrap();
    assert_eq!(page.total_matched, 1);
    assert_eq!(page.events[0].payload["bridge"], "scapy_bridge");
}
