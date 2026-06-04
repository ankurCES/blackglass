// crates/core/tests/chokepoint.rs
use blackglass_audit::Chain;
use blackglass_core::chokepoint::{execute_action, Chokepoint, Outcome};
use blackglass_core::gates::{ActionRequest, AllowAll, Gate3, Gate4};
use blackglass_engagement::{Engagement, Target, TargetKind};
use blackglass_profile::Profile;
use serde_json::json;
use std::sync::Arc;

fn setup() -> (Chokepoint, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let chain = Chain::open(&audit_path).unwrap();
    let profile = Profile::analyst_default();
    let mut eng = Engagement::new("eng-1", "Test", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    eng.add_target(Target { value: "10.0.0.5".into(), kind: TargetKind::Ip });
    let cp = Chokepoint::new(
        chain, profile, eng,
        Arc::new(AllowAll) as Arc<dyn Gate3>,
        Arc::new(AllowAll) as Arc<dyn Gate4>,
        tokio::sync::broadcast::channel(64).0,
    );
    (cp, dir)
}

#[tokio::test]
async fn allows_action_against_in_scope_target() {
    let (mut cp, _d) = setup();
    let r = execute_action(&mut cp, ActionRequest {
        domain: "osint".into(),
        action_class: "read_only".into(),
        target: "10.0.0.5".into(),
        args: json!({}),
    }).await.unwrap();
    assert!(matches!(r, Outcome::Allowed { .. }));
}

#[tokio::test]
async fn denies_action_against_out_of_scope_target() {
    let (mut cp, _d) = setup();
    let err = execute_action(&mut cp, ActionRequest {
        domain: "osint".into(),
        action_class: "read_only".into(),
        target: "10.0.0.6".into(),
        args: json!({}),
    }).await.unwrap_err();
    assert!(err.to_string().contains("not in engagement allowlist"));
}

struct DenyAll;
#[async_trait::async_trait]
impl Gate3 for DenyAll {
    async fn confirm(&self, _req: &ActionRequest) -> blackglass_core::gates::ConfirmationOutcome {
        blackglass_core::gates::ConfirmationOutcome::Deny
    }
}

#[tokio::test]
async fn gate3_denial_is_logged_and_propagated() {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let chain = Chain::open(&audit_path).unwrap();
    let profile = Profile::analyst_default();
    let mut eng = Engagement::new("e", "T", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    eng.add_target(Target { value: "10.0.0.5".into(), kind: TargetKind::Ip });
    let mut cp = Chokepoint::new(
        chain, profile, eng,
        Arc::new(DenyAll) as Arc<dyn Gate3>,
        Arc::new(AllowAll) as Arc<dyn Gate4>,
        tokio::sync::broadcast::channel(64).0,
    );
    let err = execute_action(&mut cp, ActionRequest {
        domain: "osint".into(), action_class: "read_only".into(),
        target: "10.0.0.5".into(), args: json!({}),
    }).await.unwrap_err();
    assert!(err.to_string().contains("deny"), "expected 'deny' in err, got: {err}");
    let count = Chain::verify(&audit_path).unwrap();
    assert!(count >= 2, "expected at least 2 audit events, got {count}");
}

#[tokio::test]
async fn gate1_denies_disallowed_domain() {
    let (mut cp, _d) = setup();
    let err = execute_action(&mut cp, ActionRequest {
        domain: "phish".into(),
        action_class: "read_only".into(),
        target: "10.0.0.5".into(),
        args: json!({}),
    }).await.unwrap_err();
    assert!(err.to_string().contains("not in profile allowlist"));
}

#[tokio::test]
async fn audit_log_verifies_after_real_run() {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let chain = Chain::open(&audit_path).unwrap();
    let profile = Profile::analyst_default();
    let mut eng = Engagement::new("e", "T", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    eng.add_target(Target { value: "10.0.0.5".into(), kind: TargetKind::Ip });
    let mut cp = Chokepoint::new(
        chain, profile, eng,
        Arc::new(AllowAll) as Arc<dyn Gate3>,
        Arc::new(AllowAll) as Arc<dyn Gate4>,
        tokio::sync::broadcast::channel(64).0,
    );
    for _ in 0..3 {
        let _ = execute_action(&mut cp, ActionRequest {
            domain: "osint".into(), action_class: "read_only".into(),
            target: "10.0.0.5".into(), args: json!({}),
        }).await.unwrap();
    }
    // 3 actions x 3 events (requested, allowed, executed) = 9
    let n = Chain::verify(&audit_path).unwrap();
    assert_eq!(n, 9, "expected 9 events, got {n}");
}

use blackglass_core::gates::SanitizedOutput;

struct PiGate;
impl Gate4 for PiGate {
    fn sanitize(&self, _stdout: &str, _stderr: &str) -> SanitizedOutput {
        SanitizedOutput {
            stdout: "BEGIN\ncleaned\nEND".into(),
            stderr: String::new(),
            redacted_fields: vec!["injected line".into()],
            pi_detected: true,
            pi_line_count: 1,
        }
    }
}

#[tokio::test]
async fn pi_detection_emits_audit_event_and_writes_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let evidence_dir = dir.path().join("evidence");
    std::fs::create_dir_all(&evidence_dir).unwrap();

    let chain = Chain::open(&audit_path).unwrap();
    let mut eng = Engagement::new("e", "t", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    eng.add_target(Target { value: "10.0.0.5".into(), kind: TargetKind::Ip });
    let mut cp = Chokepoint::new(
        chain, Profile::analyst_default(), eng,
        Arc::new(AllowAll) as Arc<dyn Gate3>,
        Arc::new(PiGate) as Arc<dyn Gate4>,
        tokio::sync::broadcast::channel(64).0,
    ).with_evidence_dir(evidence_dir.clone());

    let _ = execute_action(&mut cp, ActionRequest {
        domain: "osint".into(),
        action_class: "read_only".into(),
        target: "10.0.0.5".into(),
        args: json!({}),
    }).await.unwrap();

    // PI event should be in the audit log (requested, allowed, pi_suspected, executed = 4)
    let n = Chain::verify(&audit_path).unwrap();
    assert!(n >= 4, "expected ≥4 events (requested, allowed, pi, executed), got {n}");

    // Evidence file should exist
    let evidence_files: Vec<_> = std::fs::read_dir(&evidence_dir).unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(!evidence_files.is_empty(), "expected evidence file written");
}

#[tokio::test]
async fn gate3_returns_allow_outcome() {
    use blackglass_core::gates::{ActionRequest, AllowAll, ConfirmationOutcome, Gate3};
    let g = AllowAll;
    let req = ActionRequest {
        domain: "recon".into(),
        action_class: "destructive".into(),
        target: "10.0.0.1".into(),
        args: serde_json::json!({}),
    };
    let outcome = g.confirm(&req).await;
    assert!(matches!(outcome, ConfirmationOutcome::Allow));
}

mod chokepoint_full {
    use blackglass_core::broker::{ConfirmationBroker, Decision};
    use blackglass_core::chokepoint::{evaluate, EvalOutcome, Profile};
    use blackglass_core::gates::{ActionRequest, Gate3, Gate4, SanitizedOutput};
    use blackglass_core::policy::{ActionClass, Policy};
    use async_trait::async_trait;
    use std::time::Duration;

    struct StubGate3 {
        decision: blackglass_core::broker::Decision,
    }
    #[async_trait]
    impl Gate3 for StubGate3 {
        async fn confirm(&self, _req: &ActionRequest) -> blackglass_core::gates::ConfirmationOutcome {
            match &self.decision {
                Decision::Allow => blackglass_core::gates::ConfirmationOutcome::Allow,
                Decision::AllowAndRemember => blackglass_core::gates::ConfirmationOutcome::AllowAndRemember,
                Decision::Deny => blackglass_core::gates::ConfirmationOutcome::Deny,
            }
        }
    }

    struct StubGate4;
    impl Gate4 for StubGate4 {
        fn sanitize(&self, stdout: &str, stderr: &str) -> SanitizedOutput {
            SanitizedOutput {
                stdout: stdout.into(),
                stderr: stderr.into(),
                redacted_fields: vec![],
                pi_detected: false,
                pi_line_count: 0,
            }
        }
    }

    /// Test stand-in for the operator-socket handler. Polls the broker for
    /// newly-registered pending ids and resolves each as `decision`. Stops
    /// after `stop_after` successful resolutions.
    async fn auto_resolver(broker: ConfirmationBroker, decision: Decision, stop_after: usize) {
        let mut resolved = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(5)).await;
            let pending = broker.pending_ids().await;
            for id in pending {
                if resolved >= stop_after { return; }
                let _ = broker.resolve(&id, decision.clone()).await;
                resolved += 1;
            }
        }
    }

    #[tokio::test]
    async fn destructive_with_operator_allow_emits_5_events() {
        let dir = tempfile::tempdir().unwrap();
        let policy = Policy::default();
        let profile = Profile {
            name: "test".into(),
            allowed_classes: vec![ActionClass::Destructive],
        };
        let gate3 = StubGate3 { decision: Decision::Allow };
        let gate4 = StubGate4;
        let broker = ConfirmationBroker::new();

        let req = ActionRequest {
            domain: "recon".into(),
            action_class: "destructive".into(),
            target: "10.10.0.5/24".into(),
            args: serde_json::json!({}),
        };

        // NOTE: the spec's `evaluate()` registers a pending confirmation on
        // the broker and awaits the receiver, but provides no public way for
        // the test to discover the id. The production caller is the
        // operator-socket handler (Task 5); in a test we stand in for it
        // with a helper task that polls `pending_ids()` and resolves any
        // new id as Allow. The `pending_ids()` accessor is a minimal,
        // additive broker API extension (documented in the deviation report).
        let resolver_broker = broker.clone();
        let resolver = tokio::spawn(async move {
            auto_resolver(resolver_broker, Decision::Allow, 1).await;
        });

        let outcome = evaluate(
            &policy,
            &profile,
            &req,
            &gate3,
            &gate4,
            &broker,
            "ai-session-claude-opus-4",
            "nmap_scan",
            dir.path(),
        ).await;

        resolver.abort();

        assert!(matches!(outcome, EvalOutcome::Allowed { .. }));
        let audit_path = dir.path().join("audit.jsonl");
        let count = blackglass_audit::Chain::verify(&audit_path).unwrap();
        assert_eq!(count, 5, "expected 5 audit events, got {count}");
    }
}
