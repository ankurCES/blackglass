//! Confirmation broker. See spec §6.2.
//!
//! The chokepoint calls `register()` to get a `(id, oneshot::Receiver)`,
//! awaits the receiver, and emits the resulting decision. The
//! operator-socket handler (Task 5) calls `resolve(id, decision)` to
//! fire the sender. If `resolve` returns `Err`, the chokepoint has
//! already timed out and the handler logs a second
//! `OperatorConfirmationResolved{decision: "deny_late"}` event.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    AllowAndRemember,
    Deny,
}

#[derive(Clone)]
pub struct ConfirmationBroker {
    inner: Arc<Mutex<HashMap<String, oneshot::Sender<Decision>>>>,
}

impl ConfirmationBroker {
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// Register a new pending confirmation. Returns `(id, receiver)`.
    pub async fn register(&self) -> (String, oneshot::Receiver<Decision>) {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.inner.lock().await.insert(id.clone(), tx);
        (id, rx)
    }

    /// Resolve a pending confirmation. Returns Err if id is unknown
    /// (already timed out, or never registered).
    pub async fn resolve(&self, id: &str, decision: Decision) -> Result<(), ()> {
        let mut map = self.inner.lock().await;
        match map.remove(id) {
            Some(tx) => { let _ = tx.send(decision); Ok(()) }
            None => Err(()),
        }
    }

    pub async fn is_empty(&self) -> bool {
        self.inner.lock().await.is_empty()
    }
}

impl Default for ConfirmationBroker {
    fn default() -> Self {
        Self::new()
    }
}
