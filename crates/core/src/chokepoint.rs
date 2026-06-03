//! The single chokepoint. Every privileged action goes through here.
//! See spec §2.1, §4.

use crate::gates::{ActionRequest, ConfirmationOutcome, Gate3, Gate4};
use blackglass_audit::{Chain, Event, EventKind};
use blackglass_engagement::Engagement;
use blackglass_profile::Profile as LegacyProfile;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum ChokepointError {
    #[error("audit: {0}")]
    Audit(#[from] blackglass_audit::AuditError),
    #[error("gate 1: domain '{0}' not in profile allowlist")]
    DomainNotAllowed(String),
    #[error("gate 1: action class '{0}' not in profile allowlist")]
    ActionClassNotAllowed(String),
    #[error("gate 2: target '{0}' not in engagement allowlist")]
    TargetNotAllowed(String),
    #[error("gate 3: {0}")]
    Gate3Denied(String),
}

#[derive(Debug)]
pub enum Outcome {
    Allowed { stdout: String, stderr: String },
}

impl Outcome {
    pub fn stdout(&self) -> &str { match self { Self::Allowed { stdout, .. } => stdout } }
    pub fn stderr(&self) -> &str { match self { Self::Allowed { stderr, .. } => stderr } }
}

pub struct Chokepoint {
    pub chain: Chain,
    pub profile: LegacyProfile,
    pub engagement: Engagement,
    pub gate3: Arc<dyn Gate3>,
    pub gate4: Arc<dyn Gate4>,
    pub seq: u64,
    pub evidence_dir: Option<std::path::PathBuf>,
}

impl Chokepoint {
    pub fn new(
        chain: Chain, profile: LegacyProfile, engagement: Engagement,
        gate3: Arc<dyn Gate3>, gate4: Arc<dyn Gate4>,
    ) -> Self {
        Self { chain, profile, engagement, gate3, gate4, seq: 0, evidence_dir: None }
    }

    pub fn with_evidence_dir(mut self, dir: std::path::PathBuf) -> Self {
        self.evidence_dir = Some(dir);
        self
    }

    fn next_seq(&mut self) -> u64 { self.seq += 1; self.seq }

    fn audit(&mut self, kind: EventKind, payload: serde_json::Value) -> Result<(), ChokepointError> {
        let ev = Event {
            seq: self.next_seq(),
            ts: iso8601_utc_now(),
            prev_hash: String::new(),
            kind, payload,
        };
        self.chain.append(ev)?;
        Ok(())
    }
}

pub async fn execute_action(cp: &mut Chokepoint, req: ActionRequest) -> Result<Outcome, ChokepointError> {
    if !cp.profile.allows_domain(&req.domain) {
        cp.audit(EventKind::ActionDenied, json!({"gate":1, "reason":"domain", "req": &req}))?;
        return Err(ChokepointError::DomainNotAllowed(req.domain));
    }
    if !cp.profile.allows_action_class(&req.action_class) {
        cp.audit(EventKind::ActionDenied, json!({"gate":1, "reason":"action_class", "req": &req}))?;
        return Err(ChokepointError::ActionClassNotAllowed(req.action_class));
    }
    if !cp.engagement.allows(&req.target) {
        cp.audit(EventKind::ActionDenied, json!({"gate":2, "reason":"target", "req": &req}))?;
        return Err(ChokepointError::TargetNotAllowed(req.target));
    }
    cp.audit(EventKind::ActionRequested, json!({"req": &req}))?;

    let outcome = cp.gate3.confirm(&req).await;
    match outcome {
        ConfirmationOutcome::Allow | ConfirmationOutcome::AllowAndRemember => {}
        _ => {
            let reason = outcome.as_decision_str().to_string();
            cp.audit(EventKind::ActionDenied, json!({"gate":3, "reason": &reason, "req": &req}))?;
            return Err(ChokepointError::Gate3Denied(reason));
        }
    }
    cp.audit(EventKind::ActionAllowed, json!({"req": &req}))?;

    let fake_stdout = format!("simulated output for {} on {}", req.domain, req.target);
    let fake_stderr = String::new();
    let san = cp.gate4.sanitize(&fake_stdout, &fake_stderr);
    if san.pi_detected {
        let evidence_text = san.redacted_fields.join("\n");
        let evidence_path = if let Some(ref dir) = cp.evidence_dir {
            let p = dir.join(format!("pi-seq{}.txt", cp.seq + 1));
            let _ = std::fs::write(&p, &evidence_text);
            p.display().to_string()
        } else {
            "(evidence_dir not configured)".into()
        };
        cp.audit(EventKind::PromptInjectionSuspected, json!({
            "evidence_path": evidence_path,
            "line_count": san.pi_line_count,
        }))?;
    }
    cp.audit(EventKind::ActionExecuted, json!({
        "req": &req,
        "stdout_sha256": sha256_hex(san.stdout.as_bytes()),
        "stderr_sha256": sha256_hex(san.stderr.as_bytes()),
        "redacted_fields": san.redacted_fields,
    }))?;

    info!(target = %req.target, domain = %req.domain, "action executed (simulated)");
    Ok(Outcome::Allowed { stdout: san.stdout, stderr: san.stderr })
}

fn sha256_hex(b: &[u8]) -> String { hex::encode(Sha256::digest(b)) }

/// Std-only ISO-8601 UTC timestamp ("YYYY-MM-DDTHH:MM:SSZ"), second precision.
fn iso8601_utc_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = dur.as_secs();
    let days = (secs / 86_400) as i64;
    let secs_of_day = (secs % 86_400) as u32;
    let (y, m, d) = civil_from_days(days);
    let h = secs_of_day / 3600;
    let mi = (secs_of_day % 3600) / 60;
    let s = secs_of_day % 60;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, h, mi, s)
}

// Howard Hinnant's date algorithm (public domain).
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe/1460 + doe/36524 - doe/146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365*yoe + yoe/4 - yoe/100);
    let mp = (5*doy + 2)/153;
    let d = doy - (153*mp + 2)/5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

// =========================================================================
// Sub-plan 3 async chokepoint (Task 7). Coexists with the legacy
// `Chokepoint`/`execute_action` API above so existing server, main, and
// integration tests keep working. The legacy API is exercised by sub-plan 2
// callers; a later task migrates them to the new `evaluate` design.
//
// The chokepoint: every action goes through here. Gate 1 (policy) ->
// Gate 3 (operator confirm, only for destructive classes) -> exec stub ->
// Gate 4 (sanitize) -> return. Audit events are appended at every
// boundary. See spec §4 and §6.4.
//
// Implemented as a sub-module to avoid name collisions with the legacy
// types re-exported at the chokepoint level (e.g. `Profile` from
// `blackglass_profile` vs. the new local `Profile`). The items
// `Profile`, `EvalOutcome`, and `evaluate` are re-exported from this
// sub-module so callers can use `blackglass_core::chokepoint::Profile`.
// =========================================================================

pub mod r#async {
    use std::path::Path;
    use std::time::Duration;
    use serde::Serialize;

    use crate::broker::{ConfirmationBroker, Decision};
    use crate::gates::{ActionRequest, ConfirmationOutcome, Gate3, Gate4, SanitizedOutput};
    use crate::policy::{ActionClass, Policy};

    #[derive(Debug, Clone)]
    pub struct Profile {
        pub name: String,
        pub allowed_classes: Vec<ActionClass>,
    }

    impl Default for Profile {
        fn default() -> Self {
            Self { name: "analyst".into(), allowed_classes: vec![ActionClass::ReadOnly] }
        }
    }

    #[derive(Debug)]
    pub enum EvalOutcome {
        Allowed { sanitized: SanitizedOutput },
        Denied { reason: String },
    }

    #[derive(Serialize)]
    struct AuditActionRequested<'a> {
        request_id: u64,
        source: &'a str,
        tool: &'a str,
        domain: &'a str,
        class: &'a str,
        target: &'a str,
    }

    #[derive(Serialize)]
    struct AuditActionAllowed<'a> {
        request_id: u64,
        class: &'a str,
    }

    #[derive(Serialize)]
    struct AuditActionDenied<'a> {
        request_id: u64,
        reason: &'a str,
        decision: &'a str,
    }

    #[derive(Serialize)]
    struct AuditActionExecuted {
        request_id: u64,
        stdout_bytes: usize,
        stderr_bytes: usize,
    }

    #[derive(Serialize)]
    struct AuditConfirmationRequested<'a> {
        id: &'a str,
        request_id: u64,
        tool: &'a str,
        domain: &'a str,
        class: &'a str,
        target: &'a str,
        source: &'a str,
    }

    #[derive(Serialize)]
    struct AuditConfirmationResolved<'a> {
        id: &'a str,
        request_id: u64,
        decision: &'a str,
    }

    const CONFIRM_TIMEOUT: Duration = Duration::from_secs(15);
    /// Test-suite timeout (200 ms). Always defined; unused in non-test
    /// builds. The runtime path takes `CONFIRM_TIMEOUT` (15 s).
    #[allow(dead_code)]
    const CONFIRM_TIMEOUT_TEST: Duration = Duration::from_millis(200);

    // The spec mandates 9 parameters (policy, profile, req, gate3, gate4,
    // broker, source, tool, data_dir). The chokepoint is a pure function
    // with no shared state, so threading all inputs explicitly is by
    // design — see spec §4.
    #[allow(clippy::too_many_arguments)]
    pub async fn evaluate(
        policy: &Policy,
        _profile: &Profile,
        req: &ActionRequest,
        _gate3: &dyn Gate3,
        gate4: &dyn Gate4,
        broker: &ConfirmationBroker,
        source: &str,
        tool: &str,
        data_dir: &Path,
    ) -> EvalOutcome {
        let request_id: u64 = rand::random();
        let class = parse_class(&req.action_class);

        // Open the audit chain.
        let audit_path = data_dir.join("audit.jsonl");
        let mut chain = blackglass_audit::Chain::open(&audit_path).expect("audit chain open");

        // Event 1: ActionRequested.
        chain.append(blackglass_audit::Event {
            seq: 0,
            ts: now_iso8601(),
            prev_hash: String::new(),
            kind: blackglass_audit::EventKind::ActionRequested,
            payload: serde_json::to_value(AuditActionRequested {
                request_id, source, tool,
                domain: &req.domain, class: &req.action_class, target: &req.target,
            }).unwrap(),
        }).unwrap();

        // Gate 1: policy check.
        if !policy.allows(class) {
            chain.append(blackglass_audit::Event {
                seq: 0,
                ts: now_iso8601(),
                prev_hash: String::new(),
                kind: blackglass_audit::EventKind::ActionDenied,
                payload: serde_json::to_value(AuditActionDenied {
                    request_id, reason: "policy_disallows_class", decision: "deny",
                }).unwrap(),
            }).unwrap();
            return EvalOutcome::Denied { reason: "policy_disallows_class".into() };
        }

        // Gate 3: operator confirmation (only for destructive).
        if class == ActionClass::Destructive {
            let (id, rx) = broker.register().await;
            let timeout = if cfg!(test) { CONFIRM_TIMEOUT_TEST } else { CONFIRM_TIMEOUT };

            chain.append(blackglass_audit::Event {
                seq: 0,
                ts: now_iso8601(),
                prev_hash: String::new(),
                kind: blackglass_audit::EventKind::OperatorConfirmationRequested,
                payload: serde_json::to_value(AuditConfirmationRequested {
                    id: &id, request_id, tool,
                    domain: &req.domain, class: &req.action_class,
                    target: &req.target, source,
                }).unwrap(),
            }).unwrap();

            let outcome = match tokio::time::timeout(timeout, rx).await {
                Ok(Ok(Decision::Allow)) => ConfirmationOutcome::Allow,
                Ok(Ok(Decision::AllowAndRemember)) => ConfirmationOutcome::AllowAndRemember,
                Ok(Ok(Decision::Deny)) | Ok(Err(_)) => ConfirmationOutcome::Deny,
                Err(_) => ConfirmationOutcome::Timeout,
            };

            chain.append(blackglass_audit::Event {
                seq: 0,
                ts: now_iso8601(),
                prev_hash: String::new(),
                kind: blackglass_audit::EventKind::OperatorConfirmationResolved,
                payload: serde_json::to_value(AuditConfirmationResolved {
                    id: &id, request_id, decision: outcome.as_decision_str(),
                }).unwrap(),
            }).unwrap();

            match outcome {
                ConfirmationOutcome::Allow | ConfirmationOutcome::AllowAndRemember => {} // proceed
                ConfirmationOutcome::Deny | ConfirmationOutcome::Timeout | ConfirmationOutcome::Disconnected => {
                    chain.append(blackglass_audit::Event {
                        seq: 0,
                        ts: now_iso8601(),
                        prev_hash: String::new(),
                        kind: blackglass_audit::EventKind::ActionDenied,
                        payload: serde_json::to_value(AuditActionDenied {
                            request_id,
                            reason: if outcome == ConfirmationOutcome::Timeout { "operator_timeout" } else { "operator_denied" },
                            decision: outcome.as_decision_str(),
                        }).unwrap(),
                    }).unwrap();
                    return EvalOutcome::Denied { reason: "operator".into() };
                }
            }
        }

        // Event: ActionAllowed.
        chain.append(blackglass_audit::Event {
            seq: 0,
            ts: now_iso8601(),
            prev_hash: String::new(),
            kind: blackglass_audit::EventKind::ActionAllowed,
            payload: serde_json::to_value(AuditActionAllowed {
                request_id, class: &req.action_class,
            }).unwrap(),
        }).unwrap();

        // Sub-plan 3: we don't actually exec (the Tauri app does). For the
        // chokepoint test, exec is a no-op and Gate 4 is called on empty output.
        let sanitized = gate4.sanitize("", "");

        // Event: ActionExecuted.
        chain.append(blackglass_audit::Event {
            seq: 0,
            ts: now_iso8601(),
            prev_hash: String::new(),
            kind: blackglass_audit::EventKind::ActionExecuted,
            payload: serde_json::to_value(AuditActionExecuted {
                request_id,
                stdout_bytes: sanitized.stdout.len(),
                stderr_bytes: sanitized.stderr.len(),
            }).unwrap(),
        }).unwrap();

        EvalOutcome::Allowed { sanitized }
    }

    fn parse_class(s: &str) -> ActionClass {
        match s {
            "destructive" => ActionClass::Destructive,
            _ => ActionClass::ReadOnly,
        }
    }

    fn now_iso8601() -> String {
        // Delegate to the parent module's properly-implemented helper.
        super::iso8601_utc_now()
    }
}

// Re-exports so the test can write `blackglass_core::chokepoint::Profile`,
// `blackglass_core::chokepoint::EvalOutcome`, and
// `blackglass_core::chokepoint::evaluate` per the spec.
pub use self::r#async::{evaluate, EvalOutcome, Profile};
