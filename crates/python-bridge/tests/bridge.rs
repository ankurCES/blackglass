//! TDD tests for the Python bridge. See plan §1.3, §1.4, §1.12.

use blackglass_python_bridge::{
    BridgeKind, BridgeRequest, BridgeResponse, PythonBridge, StubBridge, build,
};
use serde_json::json;

#[test]
fn stub_bridge_handles_python_routed_tools() {
    let bridge = StubBridge::new();
    assert!(bridge.handles("scapy_craft"));
    assert!(bridge.handles("impacket_secretsdump"));
    assert!(bridge.handles("flipper_run"));
    assert!(bridge.handles("detect_via_rest"));
}

#[test]
fn stub_bridge_rejects_non_python_tools() {
    let bridge = StubBridge::new();
    assert!(!bridge.handles("nmap"));
    assert!(!bridge.handles("tshark_read"));
    assert!(!bridge.handles(""));
}

#[tokio::test]
async fn stub_bridge_invoke_returns_stub_marker() {
    let bridge = StubBridge::new();
    let req = BridgeRequest {
        module: "blackglass_sidecar.scapy_bridge".into(),
        function: "craft".into(),
        args: json!({ "layers": ["IP", "TCP"] }),
        evidence_dir: None,
    };
    let resp = bridge.invoke(req).await.expect("invoke");
    assert!(resp.result["stub"].as_bool().unwrap_or(false));
    assert_eq!(resp.result["module"], "blackglass_sidecar.scapy_bridge");
    assert_eq!(resp.result["function"], "craft");
}

#[tokio::test]
async fn stub_bridge_rejects_unsafe_module() {
    let bridge = StubBridge::new();
    let req = BridgeRequest {
        module: "os".into(), // attempt to call Python's `os` module
        function: "system".into(),
        args: json!({}),
        evidence_dir: None,
    };
    let err = bridge.invoke(req).await.expect_err("should reject");
    assert!(matches!(err, blackglass_python_bridge::BridgeError::InvalidArg(_)));
}

#[tokio::test]
async fn build_returns_stub_by_default() {
    let bridge = build(BridgeKind::Stub);
    // Run a no-op invoke to confirm it works through the trait object
    let req = BridgeRequest {
        module: "blackglass_sidecar.audit_types".into(),
        function: "now_iso".into(),
        args: json!({}),
        evidence_dir: None,
    };
    let resp = bridge.invoke(req).await.expect("invoke through dyn");
    assert!(resp.result["stub"].as_bool().unwrap_or(false));
}

#[test]
fn bridge_kind_parses_strings() {
    assert_eq!(BridgeKind::from_str_loose("real"), BridgeKind::Real);
    assert_eq!(BridgeKind::from_str_loose("pyo3"), BridgeKind::Real);
    assert_eq!(BridgeKind::from_str_loose("REAL"), BridgeKind::Real);
    assert_eq!(BridgeKind::from_str_loose("stub"), BridgeKind::Stub);
    assert_eq!(BridgeKind::from_str_loose(""), BridgeKind::Stub);
    assert_eq!(BridgeKind::from_str_loose("garbage"), BridgeKind::Stub);
}

#[test]
fn bridge_request_round_trips_json() {
    let req = BridgeRequest {
        module: "blackglass_sidecar.impacket_bridge".into(),
        function: "secretsdump".into(),
        args: json!({ "target": "10.0.0.1", "user": "admin" }),
        evidence_dir: Some("/tmp/ev".into()),
    };
    let s = serde_json::to_string(&req).unwrap();
    let back: BridgeRequest = serde_json::from_str(&s).unwrap();
    assert_eq!(back.module, req.module);
    assert_eq!(back.function, req.function);
    assert_eq!(back.evidence_dir, req.evidence_dir);
}

#[test]
fn bridge_response_round_trips_json() {
    let resp = BridgeResponse {
        result: json!({ "hashes": ["a", "b"] }),
        stdout: "captured stdout".into(),
        stderr: "captured stderr".into(),
        evidence_path: Some("/var/lib/blackglass/evidence/x.err".into()),
    };
    let s = serde_json::to_string(&resp).unwrap();
    let back: BridgeResponse = serde_json::from_str(&s).unwrap();
    assert_eq!(back.evidence_path.as_deref(), Some("/var/lib/blackglass/evidence/x.err"));
    assert_eq!(back.stdout, "captured stdout");
}

#[tokio::test]
async fn build_for_real_kind_in_stub_build_returns_stub() {
    // The stub build of the crate (no `real` feature) falls back to the
    // stub when `Real` is requested. This is the safe default.
    let bridge = build(BridgeKind::Real);
    let req = BridgeRequest {
        module: "blackglass_sidecar.audit_types".into(),
        function: "now_iso".into(),
        args: json!({}),
        evidence_dir: None,
    };
    let resp = bridge.invoke(req).await.expect("invoke");
    assert!(resp.result["stub"].as_bool().unwrap_or(false));
}
