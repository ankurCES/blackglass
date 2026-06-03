//! The single chokepoint. Every privileged action goes through here.
//! See spec §2.1, §4.

use crate::gates::{ActionRequest, ConfirmationOutcome, Gate3, Gate4};
use blackglass_audit::{Chain, Event, EventKind};
use blackglass_engagement::Engagement;
use blackglass_profile::Profile;
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
    pub profile: Profile,
    pub engagement: Engagement,
    pub gate3: Arc<dyn Gate3>,
    pub gate4: Arc<dyn Gate4>,
    pub seq: u64,
    pub evidence_dir: Option<std::path::PathBuf>,
}

impl Chokepoint {
    pub fn new(
        chain: Chain, profile: Profile, engagement: Engagement,
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
