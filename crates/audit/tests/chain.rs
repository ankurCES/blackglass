use blackglass_audit::{AuditError, Event, EventKind, Chain};
use serde_json::json;

#[test]
fn event_serializes_to_canonical_json() {
    let e = Event {
        seq: 1,
        ts: "2026-06-03T00:00:00Z".into(),
        prev_hash: "0".repeat(64),
        kind: EventKind::ActionRequested,
        payload: json!({"target": "10.0.0.1", "tool": "nmap"}),
    };
    let s = e.canonical_bytes().unwrap();
    assert!(s.starts_with(b"{\"kind\":\"action_requested\""));
    assert!(!s.ends_with(b"\n"));
}

#[test]
fn hash_is_blake3_of_canonical_bytes() {
    let e = Event {
        seq: 1,
        ts: "2026-06-03T00:00:00Z".into(),
        prev_hash: "0".repeat(64),
        kind: EventKind::ActionRequested,
        payload: json!({}),
    };
    let h = e.hash().unwrap();
    assert_eq!(h.len(), 64);
    assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn append_then_verify_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("audit.jsonl");
    let mut chain = Chain::open(&p).unwrap();
    for i in 1..=5 {
        chain.append(Event {
            seq: i,
            ts: format!("2026-06-03T00:00:0{}Z", i),
            prev_hash: String::new(),
            kind: EventKind::ActionRequested,
            payload: json!({"i": i}),
        }).unwrap();
    }
    let count = Chain::verify(&p).unwrap();
    assert_eq!(count, 5);
}

#[test]
fn verify_detects_tampered_line() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("audit.jsonl");
    let mut chain = Chain::open(&p).unwrap();
    for i in 1..=3 {
        chain.append(Event {
            seq: i,
            ts: "2026-06-03T00:00:00Z".into(),
            prev_hash: String::new(),
            kind: EventKind::ActionRequested,
            payload: json!({"i": i}),
        }).unwrap();
    }
    // Tamper: rewrite the second line's payload
    let s = std::fs::read_to_string(&p).unwrap();
    let mut lines: Vec<String> = s.lines().map(|l| l.to_string()).collect();
    lines[1] = lines[1].replace("\"i\":2", "\"i\":999");
    std::fs::write(&p, lines.join("\n") + "\n").unwrap();

    let err = Chain::verify(&p).unwrap_err();
    assert!(matches!(err, AuditError::HashMismatch { .. }), "got: {err:?}");
}

#[test]
fn allowed_and_denied_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("audit.jsonl");
    let mut chain = Chain::open(&p).unwrap();
    chain.append(Event {
        seq: 1, ts: "t".into(), prev_hash: String::new(),
        kind: EventKind::ActionAllowed,
        payload: json!({"reason": "in allowlist"}),
    }).unwrap();
    chain.append(Event {
        seq: 2, ts: "t".into(), prev_hash: String::new(),
        kind: EventKind::ActionDenied,
        payload: json!({"reason": "not in allowlist"}),
    }).unwrap();
    assert_eq!(Chain::verify(&p).unwrap(), 2);
}

#[test]
fn prompt_injection_suspected_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("audit.jsonl");
    let mut chain = Chain::open(&p).unwrap();
    chain.append(Event {
        seq: 1,
        ts: "2026-06-03T00:00:00Z".into(),
        prev_hash: String::new(),
        kind: EventKind::PromptInjectionSuspected,
        payload: json!({"evidence_path": "/tmp/pi-001.txt", "line_count": 2}),
    }).unwrap();
    assert_eq!(Chain::verify(&p).unwrap(), 1);
}
