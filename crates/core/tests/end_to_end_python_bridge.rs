// crates/core/tests/end_to_end_python_bridge.rs
//
// End-to-end: the chokepoint routes a Python-routed tool to the
// sidecar bridge, emits the right audit events, and returns the
// bridge's output. Uses `StubBridge` so the test is hermetic — no
// Python interpreter is loaded.
//
// See plan §1.8 — verifies steps 3 (dispatch), 4 (audit events), and
// 7 (end-to-end test from the spec).

use blackglass_audit::Chain;
use blackglass_core::chokepoint::{execute_action, Chokepoint, ChokepointError};
use blackglass_core::gates::{ActionRequest, AllowAll, Gate3, Gate4};
use blackglass_engagement::{Engagement, Target, TargetKind};
use blackglass_profile::Profile;
use blackglass_python_bridge::{PythonBridge, StubBridge};
use serde_json::json;
use std::sync::Arc;

fn setup() -> (Chokepoint, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let chain = Chain::open(&audit_path).unwrap();
    // Allow all domains/action classes the bridge tests need; the
    // profile's job is policy, the bridge test is about wiring.
    let mut profile = Profile::analyst_default();
    profile.allowed_domains = vec![
        "core".into(), "osint".into(), "packets".into(), "audit".into(),
        "ad".into(), "flipper".into(), "phish".into(), "detect".into(),
    ];
    profile.allowed_action_classes = vec!["read_only".into(), "destructive".into()];
    // Target string is used BOTH for Gate 2 (engagement check) and for
    // routing (is_python_routed reads it). For Python-routed tools,
    // MCP servers pass the tool name as the target, so we do the same.
    let mut eng = Engagement::new("eng-1", "Test", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    eng.add_target(Target { value: "scapy_craft".into(), kind: TargetKind::Hostname });
    let cp = Chokepoint::new(
        chain, profile, eng,
        Arc::new(AllowAll) as Arc<dyn Gate3>,
        Arc::new(AllowAll) as Arc<dyn Gate4>,
        tokio::sync::broadcast::channel(64).0,
    )
    .with_python_bridge(Some(Arc::new(StubBridge::new())
        as Arc<dyn PythonBridge>));
    (cp, dir)
}

#[tokio::test]
async fn scapy_craft_routes_to_stub_bridge_and_emits_python_bridge_invoked() {
    let (mut cp, dir) = setup();

    let result = execute_action(&mut cp, ActionRequest {
        domain: "packets".into(),
        action_class: "read_only".into(),
        target: "scapy_craft".into(),
        args: json!({"spec": "IP()/TCP()"}),
    })
    .await
    .expect("scapy_craft should succeed via stub bridge");

    // The stub's stdout is empty and its stderr contains the
    // "[stub] module::function called" marker.
    assert!(result.stdout().is_empty(), "stub stdout should be empty, got: {:?}", result.stdout());
    assert!(result.stderr().contains("[stub]"), "missing stub marker in stderr: {:?}", result.stderr());
    assert!(result.stderr().contains("scapy_bridge"), "missing module in stderr: {:?}", result.stderr());
    assert!(result.stderr().contains("craft"), "missing function in stderr: {:?}", result.stderr());

    // Audit log: must contain a PythonBridgeInvoked event with the
    // expected module/function/bridge fields.
    let log = std::fs::read_to_string(dir.path().join("audit.jsonl")).unwrap();
    assert!(log.contains("\"python_bridge_invoked\""), "missing PythonBridgeInvoked in log: {log}");
    assert!(log.contains("\"module\":\"blackglass_sidecar.scapy_bridge\""), "missing scapy_bridge module: {log}");
    assert!(log.contains("\"function\":\"craft\""), "missing craft function: {log}");
    assert!(log.contains("\"bridge\":\"python\""), "missing bridge=python tag: {log}");
    // And an ActionExecuted{bridge:"python"} on success.
    assert!(log.contains("\"action_executed\""), "missing ActionExecuted: {log}");
    assert!(log.contains("\"success\":true"), "missing success=true: {log}");
}

#[tokio::test]
async fn python_routed_tool_with_no_bridge_configured_is_rejected() {
    // No `with_python_bridge(...)` call -> bridge is None.
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let chain = Chain::open(&audit_path).unwrap();
    let mut profile = Profile::analyst_default();
    profile.allowed_domains = vec!["packets".into(), "ad".into(), "detect".into(), "flipper".into(), "phish".into()];
    profile.allowed_action_classes = vec!["read_only".into(), "destructive".into()];
    let mut eng = Engagement::new("eng-1", "Test", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    eng.add_target(Target { value: "scapy_craft".into(), kind: TargetKind::Hostname });
    let mut cp = Chokepoint::new(
        chain, profile, eng,
        Arc::new(AllowAll) as Arc<dyn Gate3>,
        Arc::new(AllowAll) as Arc<dyn Gate4>,
        tokio::sync::broadcast::channel(64).0,
    );
    // Intentionally NOT calling .with_python_bridge(...).

    let err = execute_action(&mut cp, ActionRequest {
        domain: "packets".into(),
        action_class: "read_only".into(),
        target: "scapy_craft".into(),
        args: json!({}),
    })
    .await
    .expect_err("must reject when bridge is None");

    assert!(matches!(err, ChokepointError::PythonBridgeNotConfigured(_)));
    assert!(err.to_string().contains("scapy_craft"));

    // Audit log should have a denial event with reason=bridge_not_configured.
    let log = std::fs::read_to_string(&audit_path).unwrap();
    assert!(log.contains("\"action_denied\""), "missing ActionDenied: {log}");
    assert!(log.contains("\"bridge_not_configured\""), "missing reason: {log}");
    // And NO PythonBridgeInvoked (we never reached the bridge).
    assert!(!log.contains("\"python_bridge_invoked\""), "must not invoke when bridge is None");
}

#[tokio::test]
async fn non_python_routed_tool_skips_bridge_path() {
    // osint/whois is NOT in the routing table. The legacy simulated
    // path should run, and no PythonBridgeInvoked event should be
    // emitted.
    let (mut cp, dir) = setup();
    // Allow `whois` as a target.
    cp.engagement.add_target(Target { value: "whois".into(), kind: TargetKind::Hostname });

    let r = execute_action(&mut cp, ActionRequest {
        domain: "osint".into(),
        action_class: "read_only".into(),
        target: "whois".into(),
        args: json!({"query": "example.com"}),
    })
    .await
    .expect("legacy path should succeed");

    // The legacy path returns a fake "simulated output for {domain} on {target}".
    assert!(r.stdout().contains("simulated output"));
    assert!(r.stdout().contains("osint"));
    assert!(r.stdout().contains("whois"));

    // No bridge events should appear.
    let log = std::fs::read_to_string(dir.path().join("audit.jsonl")).unwrap();
    assert!(!log.contains("\"python_bridge_invoked\""), "non-python tool must not invoke bridge: {log}");
    assert!(!log.contains("\"python_bridge_failed\""), "non-python tool must not fail at bridge: {log}");
}

#[tokio::test]
async fn ad_impacket_psexec_routes_to_impacket_bridge() {
    // Verify a different domain/branch dispatches to the right module.
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let chain = Chain::open(&audit_path).unwrap();
    let mut profile = Profile::analyst_default();
    profile.allowed_domains = vec!["ad".into(), "packets".into(), "detect".into(), "flipper".into(), "phish".into()];
    profile.allowed_action_classes = vec!["read_only".into(), "destructive".into()];
    let mut eng = Engagement::new("eng-1", "Test", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    eng.add_target(Target { value: "ad-impacket_psexec".into(), kind: TargetKind::Hostname });
    let mut cp = Chokepoint::new(
        chain, profile, eng,
        Arc::new(AllowAll) as Arc<dyn Gate3>,
        Arc::new(AllowAll) as Arc<dyn Gate4>,
        tokio::sync::broadcast::channel(64).0,
    )
    .with_python_bridge(Some(Arc::new(StubBridge::new())
        as Arc<dyn PythonBridge>));

    let result = execute_action(&mut cp, ActionRequest {
        domain: "ad".into(),
        action_class: "destructive".into(),
        target: "ad-impacket_psexec".into(),
        args: json!({"target": "10.0.0.1", "user": "u", "hash": "h", "remote_cmd": "id"}),
    })
    .await
    .expect("psexec should route through stub");

    assert!(result.stderr().contains("impacket_bridge"));
    let log = std::fs::read_to_string(&audit_path).unwrap();
    assert!(log.contains("\"module\":\"blackglass_sidecar.impacket_bridge\""));
    assert!(log.contains("\"function\":\"psexec\""));
}

#[tokio::test]
async fn detect_image_routes_to_detect_bridge() {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let chain = Chain::open(&audit_path).unwrap();
    let mut profile = Profile::analyst_default();
    profile.allowed_domains = vec!["detect".into(), "packets".into(), "ad".into(), "flipper".into(), "phish".into()];
    profile.allowed_action_classes = vec!["read_only".into(), "destructive".into()];
    let mut eng = Engagement::new("eng-1", "Test", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    eng.add_target(Target { value: "detect-image".into(), kind: TargetKind::Hostname });
    let mut cp = Chokepoint::new(
        chain, profile, eng,
        Arc::new(AllowAll) as Arc<dyn Gate3>,
        Arc::new(AllowAll) as Arc<dyn Gate4>,
        tokio::sync::broadcast::channel(64).0,
    )
    .with_python_bridge(Some(Arc::new(StubBridge::new())
        as Arc<dyn PythonBridge>));

    let _r = execute_action(&mut cp, ActionRequest {
        domain: "detect".into(),
        action_class: "read_only".into(),
        target: "detect-image".into(),
        args: json!({"path": "/tmp/img.png"}),
    })
    .await
    .expect("detect should route through stub");

    let log = std::fs::read_to_string(&audit_path).unwrap();
    assert!(log.contains("\"module\":\"blackglass_sidecar.detect_bridge\""));
    assert!(log.contains("\"function\":\"image\""));
}

#[tokio::test]
async fn python_bridge_failure_emits_failed_event_and_returns_error() {
    // Use a bridge that always errors. Verify we get an Err result
    // AND a PythonBridgeFailed audit event.
    use async_trait::async_trait;
    use blackglass_python_bridge::{BridgeError, BridgeRequest, BridgeResponse};

    struct FailingBridge;
    #[async_trait]
    impl PythonBridge for FailingBridge {
        fn handles(&self, _tool: &str) -> bool { true }
        async fn invoke(&self, _req: BridgeRequest) -> Result<BridgeResponse, BridgeError> {
            Err(BridgeError::Runtime("intentional test failure".into()))
        }
    }

    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let chain = Chain::open(&audit_path).unwrap();
    let mut profile = Profile::analyst_default();
    profile.allowed_domains = vec!["packets".into(), "ad".into(), "detect".into(), "flipper".into(), "phish".into()];
    profile.allowed_action_classes = vec!["read_only".into(), "destructive".into()];
    let mut eng = Engagement::new("eng-1", "Test", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    eng.add_target(Target { value: "scapy_craft".into(), kind: TargetKind::Hostname });
    let mut cp = Chokepoint::new(
        chain, profile, eng,
        Arc::new(AllowAll) as Arc<dyn Gate3>,
        Arc::new(AllowAll) as Arc<dyn Gate4>,
        tokio::sync::broadcast::channel(64).0,
    )
    .with_python_bridge(Some(Arc::new(FailingBridge)));

    let err = execute_action(&mut cp, ActionRequest {
        domain: "packets".into(),
        action_class: "read_only".into(),
        target: "scapy_craft".into(),
        args: json!({}),
    })
    .await
    .expect_err("failing bridge must surface as Err");

    assert!(matches!(err, ChokepointError::PythonBridge(_)));
    assert!(err.to_string().contains("intentional test failure"));

    let log = std::fs::read_to_string(&audit_path).unwrap();
    assert!(log.contains("\"python_bridge_invoked\""), "must still log invoked: {log}");
    assert!(log.contains("\"python_bridge_failed\""), "must log failed: {log}");
    assert!(log.contains("intentional test failure"));
}
