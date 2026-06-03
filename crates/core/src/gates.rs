//! Gate 3 (action-class confirmation) and Gate 4 (output sanitization).
//! See spec §4. Sub-plan 2 implemented Gate 4; sub-plan 3 implements Gate 3.

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::broker::{ConfirmationBroker, Decision};
use crate::operator_server::{ConfirmChannel, ConfirmRequest};

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

/// Gate 3 implementation that requires operator confirmation via the
/// `ConfirmationBroker` + `ConfirmChannel`. The chokepoint calls
/// `confirm(req)`; the gate registers a pending decision, pushes a
/// `confirm.request` event onto the channel (so any connected Tauri
/// client sees it and shows a modal), and awaits the operator's
/// `confirm.resolve` for up to 15 seconds. If the operator never
/// connects, the timeout fires and we return `ConfirmationOutcome::Timeout`.
///
/// `source` and `tool` are the labels attached to the `ConfirmRequest`
/// event so the operator UI can display what the action is. In
/// production these are constants from `main.rs`; in tests they can
/// be anything.
pub struct BrokerGate3 {
    broker: ConfirmationBroker,
    channel: ConfirmChannel,
    source: String,
    tool: String,
    timeout: Duration,
}

impl BrokerGate3 {
    pub fn new(
        broker: ConfirmationBroker,
        channel: ConfirmChannel,
        source: impl Into<String>,
        tool: impl Into<String>,
    ) -> Self {
        Self {
            broker,
            channel,
            source: source.into(),
            tool: tool.into(),
            timeout: Duration::from_secs(15),
        }
    }

    /// Construct a `BrokerGate3` with empty `source`/`tool` defaults.
    /// Used by the production `main` where the chokepoint is a singleton
    /// and the MCP server identity is not yet known at construction time.
    /// The Tauri shell currently shows `target` and `class` to the user;
    /// `source`/`tool` are forwarded verbatim and may be `""` until the
    /// chokepoint is extended to carry MCP-server identity per request.
    pub fn new_anonymous(broker: ConfirmationBroker, channel: ConfirmChannel) -> Self {
        Self::new(broker, channel, "", "")
    }

    /// Override the default 15 s timeout. Production keeps the default;
    /// tests use this to keep the test suite snappy.
    #[allow(dead_code)]
    pub fn with_timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }
}

#[async_trait]
impl Gate3 for BrokerGate3 {
    async fn confirm(&self, req: &ActionRequest) -> ConfirmationOutcome {
        let (id, rx) = self.broker.register().await;
        // The request_id is opaque to the protocol; it's just the audit
        // log's correlation key. We generate a fresh u64 here so the
        // OperatorConfirmationRequested event (if/when audit emission
        // is added) can be cross-referenced to the ActionRequested event.
        let request_id: u64 = rand::random();
        let deadline_ms = self.timeout.as_millis() as u64;

        self.channel.push(ConfirmRequest {
            id: id.clone(),
            request_id,
            tool: self.tool.clone(),
            domain: req.domain.clone(),
            class: req.action_class.clone(),
            target: req.target.clone(),
            source: self.source.clone(),
            deadline_in_ms: deadline_ms,
        });

        match tokio::time::timeout(self.timeout, rx).await {
            Ok(Ok(Decision::Allow)) => ConfirmationOutcome::Allow,
            Ok(Ok(Decision::AllowAndRemember)) => ConfirmationOutcome::AllowAndRemember,
            Ok(Ok(Decision::Deny)) | Ok(Err(_)) => ConfirmationOutcome::Deny,
            Err(_) => ConfirmationOutcome::Timeout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::{ConfirmationBroker, Decision};
    use crate::operator_server::ConfirmChannel;
    use std::time::Duration;

    fn req() -> ActionRequest {
        ActionRequest {
            domain: "osint".into(),
            action_class: "destructive".into(),
            target: "example.com".into(),
            args: serde_json::json!({}),
        }
    }

    /// When the broker resolves with `Allow`, the gate returns `Allow`.
    /// This is the happy path: an operator clicks Allow on the Tauri UI,
    /// the resolve is sent over the operator socket, the broker matches
    /// it to the pending rx, and `confirm()` returns.
    #[tokio::test]
    async fn broker_gate_allow_resolves_to_allow() {
        let broker = ConfirmationBroker::new();
        let channel = ConfirmChannel::new();
        let gate = BrokerGate3::new(broker.clone(), channel, "osint", "whois")
            .with_timeout(Duration::from_secs(1));

        let gate_task = tokio::spawn(async move { gate.confirm(&req()).await });

        // Give the gate a moment to register its pending id, then resolve it.
        // We use a small retry loop because register() is async.
        let id = {
            let mut id = None;
            for _ in 0..50 {
                let ids = broker.pending_ids().await;
                if let Some(first) = ids.into_iter().next() {
                    id = Some(first);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            id.expect("gate should have registered a pending confirmation")
        };
        broker
            .resolve(&id, Decision::Allow)
            .await
            .expect("resolve should succeed");

        let outcome = gate_task.await.unwrap();
        assert_eq!(outcome, ConfirmationOutcome::Allow);
    }

    /// When the broker is asked to resolve after the gate's timeout
    /// elapses, the gate returns `Timeout` (not `Deny` — the operator
    /// simply didn't respond in time).
    #[tokio::test]
    async fn broker_gate_timeout_returns_timeout() {
        let broker = ConfirmationBroker::new();
        let channel = ConfirmChannel::new();
        let gate = BrokerGate3::new(broker, channel, "osint", "whois")
            .with_timeout(Duration::from_millis(50));

        let outcome = gate.confirm(&req()).await;
        assert_eq!(outcome, ConfirmationOutcome::Timeout);
    }

    /// `new_anonymous` builds a gate with empty source/tool, used by the
    /// production `main` where MCP-server identity isn't plumbed through
    /// the chokepoint yet.
    #[tokio::test]
    async fn broker_gate_new_anonymous_compiles() {
        let broker = ConfirmationBroker::new();
        let channel = ConfirmChannel::new();
        let _gate: BrokerGate3 = BrokerGate3::new_anonymous(broker, channel);
    }
}
