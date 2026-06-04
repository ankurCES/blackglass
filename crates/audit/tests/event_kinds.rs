//! TDD tests for the new MCP-lifecycle event kinds. See plan §1.1 and
//! sub-plan 4 amendment §1.1.

use blackglass_audit::{Chain, Event, EventKind};
use serde_json::json;

#[test]
fn mcp_server_spawned_serializes_with_server_and_pid() {
    let event = Event {
        seq: 0,
        ts: "2026-06-03T00:00:00Z".into(),
        prev_hash: String::new(),
        kind: EventKind::McpServerSpawned {
            server: "mcp-ad".into(),
            pid: 12345,
        },
        payload: json!({}),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""kind":"mcp_server_spawned""#));
    assert!(json.contains(r#""server":"mcp-ad""#));
    assert!(json.contains(r#""pid":12345"#));
}

#[test]
fn mcp_server_exited_serializes_with_code_and_restart_count() {
    let event = Event {
        seq: 0,
        ts: "2026-06-03T00:00:00Z".into(),
        prev_hash: String::new(),
        kind: EventKind::McpServerExited {
            server: "mcp-flipper".into(),
            code: -1,
            restart_count: 3,
        },
        payload: json!({}),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""kind":"mcp_server_exited""#));
    assert!(json.contains(r#""code":-1"#));
    assert!(json.contains(r#""restart_count":3"#));
}

#[test]
fn mcp_run_started_and_completed_serialize() {
    let start = Event {
        seq: 0,
        ts: "2026-06-03T00:00:00Z".into(),
        prev_hash: String::new(),
        kind: EventKind::McpRunStarted {
            domain: "ad".into(),
            target: "ad-impacket_psexec".into(),
        },
        payload: json!({}),
    };
    let end = Event {
        seq: 1,
        ts: "2026-06-03T00:00:01Z".into(),
        prev_hash: String::new(),
        kind: EventKind::McpRunCompleted {
            domain: "ad".into(),
            target: "ad-impacket_psexec".into(),
            ok: true,
            ms: 1234,
        },
        payload: json!({}),
    };
    assert!(serde_json::to_string(&start).unwrap().contains(r#""kind":"mcp_run_started""#));
    assert!(serde_json::to_string(&end).unwrap().contains(r#""kind":"mcp_run_completed""#));
    assert!(serde_json::to_string(&end).unwrap().contains(r#""ok":true"#));
    assert!(serde_json::to_string(&end).unwrap().contains(r#""ms":1234"#));
}

#[test]
fn new_event_kinds_extend_the_hash_chain() {
    // The hash chain must include the new event kinds.
    let dir = tempfile::tempdir().unwrap();
    let mut chain = Chain::open(dir.path().join("chain.jsonl")).unwrap();
    chain
        .append(Event {
            seq: 0,
            ts: "2026-06-03T00:00:00Z".into(),
            prev_hash: String::new(),
            kind: EventKind::McpServerSpawned {
                server: "mcp-ad".into(),
                pid: 1,
            },
            payload: json!({}),
        })
        .unwrap();
    chain
        .append(Event {
            seq: 1,
            ts: "2026-06-03T00:00:01Z".into(),
            prev_hash: String::new(),
            kind: EventKind::McpRunCompleted {
                domain: "ad".into(),
                target: "ad-impacket_psexec".into(),
                ok: true,
                ms: 100,
            },
            payload: json!({}),
        })
        .unwrap();
    let count = Chain::verify(dir.path().join("chain.jsonl")).unwrap();
    assert_eq!(count, 2);
}
