//! Gate 3 (action-class confirmation) and Gate 4 (output sanitization).
//! See spec §4. Sub-plan 2 implemented Gate 4; sub-plan 3 implements Gate 3.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    pub domain: String,
    pub action_class: String,
    pub target: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizedOutput {
    pub stdout: String,
    pub stderr: String,
    pub redacted_fields: Vec<String>,
    pub pi_detected: bool,
    pub pi_line_count: usize,
}

/// The operator's decision on a Gate 3 confirmation. `deny_late` is not
/// in this enum — it is logged at the operator-socket-handler layer
/// after the chokepoint has already resolved (see spec §6.2 note).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmationOutcome {
    Allow,
    AllowAndRemember,
    Deny,
    Timeout,
    Disconnected,
}

impl ConfirmationOutcome {
    pub fn as_decision_str(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::AllowAndRemember => "allow_and_remember",
            Self::Deny => "deny",
            Self::Timeout => "timeout",
            Self::Disconnected => "disconnected",
        }
    }
}

#[async_trait]
pub trait Gate3: Send + Sync {
    async fn confirm(&self, req: &ActionRequest) -> ConfirmationOutcome;
}

pub trait Gate4: Send + Sync {
    fn sanitize(&self, stdout: &str, stderr: &str) -> SanitizedOutput;
}

// NOTE: Spec puts `AllowAll` behind `#[cfg(test)]`, but `main.rs` and the
// `tests/server.rs` + `tests/chokepoint.rs` integration tests all import
// `AllowAll` as a production stub. `#[cfg(test)]` items are not visible
// from integration tests, so the stub is left public and non-`cfg(test)`
// for the lifetime of sub-plan 3. A later sub-plan replaces it with the
// real `OperatorGate3` (spec §4.3).
pub struct AllowAll;
#[async_trait]
impl Gate3 for AllowAll {
    async fn confirm(&self, _req: &ActionRequest) -> ConfirmationOutcome {
        ConfirmationOutcome::Allow
    }
}

impl Gate4 for AllowAll {
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
