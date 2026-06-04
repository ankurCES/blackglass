//! TDD tests for `Chain::query` and `Chain::verify_chain_in_band`.
//! These power the Tauri audit browser. See plan §2.4.

use blackglass_audit::{Chain, Event, EventKind};
use serde_json::json;
use tempfile::TempDir;

fn mk_event(seq: u64, kind: EventKind, payload: serde_json::Value) -> Event {
    Event {
        seq,
        ts: format!("2026-06-03T00:00:{seq:02}Z"),
        prev_hash: String::new(), // empty → Chain::append will fill it from `self.last`
        kind,
        payload,
    }
}

fn make_chain_with_events(n: u64) -> (Chain, TempDir) {
    let dir = TempDir::new().unwrap();
    let log = dir.path().join("audit.jsonl");
    let mut chain = Chain::open(&log).unwrap();
    for i in 0..n {
        let kind = if i % 2 == 0 {
            EventKind::ActionExecuted
        } else {
            EventKind::ActionDenied
        };
        chain.append(mk_event(i, kind, json!({ "i": i }))).unwrap();
    }
    (chain, dir)
}

#[test]
fn query_all_returns_every_event() {
    let (chain, _dir) = make_chain_with_events(10);
    let page = chain
        .query(&json!({ "kind": "all" }), 0, 100)
        .expect("query");
    assert_eq!(page.events.len(), 10);
    assert_eq!(page.total_matched, 10);
}

#[test]
fn query_filters_by_event_kind() {
    let (chain, _dir) = make_chain_with_events(10);
    let page = chain
        .query(
            &json!({ "kind": "kind", "kinds": ["action_executed"] }),
            0,
            100,
        )
        .expect("query");
    // 10 events, half are action_executed
    assert_eq!(page.total_matched, 5);
    assert_eq!(page.events.len(), 5);
}

#[test]
fn query_paginates_first_page() {
    let (chain, _dir) = make_chain_with_events(250);
    let page0 = chain.query(&json!({ "kind": "all" }), 0, 100).unwrap();
    let page1 = chain.query(&json!({ "kind": "all" }), 1, 100).unwrap();
    let page2 = chain.query(&json!({ "kind": "all" }), 2, 100).unwrap();
    let page3 = chain.query(&json!({ "kind": "all" }), 3, 100).unwrap();
    assert_eq!(page0.events.len(), 100);
    assert_eq!(page1.events.len(), 100);
    assert_eq!(page2.events.len(), 50);
    assert_eq!(page3.events.len(), 0);
}

#[test]
fn query_paginates_does_not_overshoot() {
    let (chain, _dir) = make_chain_with_events(5);
    let page = chain.query(&json!({ "kind": "all" }), 0, 100).unwrap();
    assert_eq!(page.events.len(), 5);
}

#[test]
fn query_returns_chain_head() {
    let (chain, _dir) = make_chain_with_events(3);
    let page = chain.query(&json!({ "kind": "all" }), 0, 100).unwrap();
    assert_eq!(page.hash_chain_head.len(), 64, "blake3 hex hash is 64 chars");
    assert_eq!(chain.last_hash(), Some(page.hash_chain_head.as_str()));
}

#[test]
fn query_and_filter_for_unknown_kind_returns_nothing() {
    let (chain, _dir) = make_chain_with_events(3);
    let page = chain
        .query(
            &json!({ "kind": "kind", "kinds": ["this_kind_does_not_exist"] }),
            0,
            100,
        )
        .unwrap();
    assert_eq!(page.total_matched, 0);
    assert_eq!(page.events.len(), 0);
}

#[test]
fn query_seq_range_filter() {
    let (chain, _dir) = make_chain_with_events(10);
    let page = chain
        .query(&json!({ "kind": "seq_range", "min": 2, "max": 5 }), 0, 100)
        .unwrap();
    assert_eq!(page.total_matched, 4); // 2, 3, 4, 5
    let seqs: Vec<u64> = page.events.iter().map(|e| e.seq).collect();
    assert_eq!(seqs, vec![2, 3, 4, 5]);
}

#[test]
fn query_and_combines_filters() {
    let (chain, _dir) = make_chain_with_events(20);
    let page = chain
        .query(
            &json!({
                "kind": "and",
                "clauses": [
                    { "kind": "kind", "kinds": ["action_executed"] },
                    { "kind": "seq_range", "min": 0, "max": 5 }
                ]
            }),
            0,
            100,
        )
        .unwrap();
    // action_executed at even seqs: 0, 2, 4. Range 0-5 intersects.
    assert_eq!(page.total_matched, 3);
}

#[test]
fn verify_chain_in_band_succeeds_on_intact_chain() {
    let (chain, _dir) = make_chain_with_events(5);
    let v = chain.verify_chain_in_band().expect("verify");
    assert!(v.verified);
    assert_eq!(v.total_events, 5);
    assert!(v.errors.is_empty());
    assert_eq!(v.root_hash.len(), 64);
}

#[test]
fn verify_chain_in_band_detects_payload_tampering() {
    let (chain, dir) = make_chain_with_events(5);
    let log = dir.path().join("audit.jsonl");
    let content = std::fs::read_to_string(&log).unwrap();
    let tampered = content.replace("\"i\":2", "\"i\":999");
    assert_ne!(content, tampered, "tampering actually changed something");
    std::fs::write(&log, tampered).unwrap();
    let v = chain.verify_chain_in_band().expect("verify");
    assert!(!v.verified, "tampering should fail verification");
    assert!(v.broken_at_seq.is_some());
    assert!(!v.errors.is_empty());
}
