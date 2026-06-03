//! Hash-chained append-only audit log.
//!
//! Each [`Event`] carries the blake3 hash of the previous event. The chain
//! is verified by [`Chain::verify`]. See ADR 0002.

use blake3::Hasher;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("chain broken at seq {seq}: prev_hash mismatch (expected {expected}, got {got})")]
    BrokenChain { seq: u64, expected: String, got: String },
    #[error("chain broken at seq {seq}: hash mismatch (computed {computed}, line says {claimed})")]
    HashMismatch { seq: u64, computed: String, claimed: String },
    #[error("line {0} is not valid JSON")]
    BadLine(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    CoreStarted,
    CoreStopped,
    ProfileLoaded,
    EngagementCreated,
    EngagementActivated,
    EngagementDeactivated,
    ActionRequested,
    ActionAllowed,
    ActionDenied,
    ActionExecuted,
    ActionFailed,
    AuditExported,
    PromptInjectionSuspected,
    OperatorConfirmationRequested,
    OperatorConfirmationResolved,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub ts: String,
    pub prev_hash: String,
    #[serde(flatten)]
    pub kind: EventKind,
    pub payload: Value,
}

impl Event {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, AuditError> {
        let mut obj = serde_json::Map::new();
        obj.insert("seq".into(), json!(self.seq));
        obj.insert("ts".into(), json!(self.ts));
        obj.insert("prev_hash".into(), json!(self.prev_hash));
        let kind_value = serde_json::to_value(&self.kind)?;
        if let Value::Object(kmap) = kind_value {
            for (k, v) in kmap.into_iter() {
                obj.insert(k, v);
            }
        }
        obj.insert("payload".into(), self.payload.clone());
        Ok(serde_json::to_vec(&Value::Object(obj))?)
    }

    pub fn hash(&self) -> Result<String, AuditError> {
        let mut h = Hasher::new();
        h.update(&self.canonical_bytes()?);
        Ok(hex::encode(h.finalize().as_bytes()))
    }
}

pub struct Chain {
    path: std::path::PathBuf,
    last: Option<String>,
}

impl Chain {
    pub fn open(path: impl Into<std::path::PathBuf>) -> Result<Self, AuditError> {
        let path = path.into();
        let last = if path.exists() {
            let s = std::fs::read_to_string(&path)?;
            last_hash_of(&s)?
        } else {
            None
        };
        Ok(Self { path, last })
    }

    pub fn last_hash(&self) -> Option<&str> { self.last.as_deref() }

    pub fn append(&mut self, mut event: Event) -> Result<String, AuditError> {
        if event.prev_hash.is_empty() {
            event.prev_hash = self.last.clone().unwrap_or_else(|| "0".repeat(64));
        } else if let Some(ref prev) = self.last {
            if event.prev_hash != *prev {
                return Err(AuditError::BrokenChain {
                    seq: event.seq,
                    expected: prev.clone(),
                    got: event.prev_hash,
                });
            }
        } else {
            return Err(AuditError::BrokenChain {
                seq: event.seq,
                expected: "0".repeat(64),
                got: event.prev_hash,
            });
        }
        let hash = event.hash()?;
        let event_json = serde_json::to_value(&event)?;
        let wrapper = json!({ "event": event_json, "hash": &hash });
        let mut s = serde_json::to_vec(&wrapper)?;
        s.push(b'\n');
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?
            .write_all(&s)?;
        self.last = Some(hash.clone());
        Ok(hash)
    }

    pub fn verify(path: impl AsRef<std::path::Path>) -> Result<u64, AuditError> {
        let s = std::fs::read_to_string(path)?;
        let mut prev = "0".repeat(64);
        let mut count = 0u64;
        for (i, line) in s.lines().enumerate() {
            if line.is_empty() { continue; }
            let v: Value = serde_json::from_str(line).map_err(|_| AuditError::BadLine(i + 1))?;
            let event = v.get("event").ok_or(AuditError::BadLine(i + 1))?;
            let claimed = v
                .get("hash")
                .and_then(|h| h.as_str())
                .ok_or(AuditError::BadLine(i + 1))?;
            let obj = event.as_object().cloned().ok_or(AuditError::BadLine(i + 1))?;
            let e: Event = serde_json::from_value(Value::Object(obj))
                .map_err(|_| AuditError::BadLine(i + 1))?;
            if e.prev_hash != prev {
                return Err(AuditError::BrokenChain {
                    seq: e.seq,
                    expected: prev,
                    got: e.prev_hash,
                });
            }
            let computed = e.hash()?;
            if computed != claimed {
                return Err(AuditError::HashMismatch {
                    seq: e.seq,
                    computed,
                    claimed: claimed.to_string(),
                });
            }
            prev = computed;
            count += 1;
        }
        Ok(count)
    }
}

fn last_hash_of(s: &str) -> Result<Option<String>, AuditError> {
    let mut last = None;
    for line in s.lines() {
        if line.is_empty() { continue; }
        let v: Value = serde_json::from_str(line).map_err(|_| AuditError::BadLine(0))?;
        if let Some(h) = v.get("hash").and_then(|x| x.as_str()) {
            last = Some(h.to_string());
        }
    }
    Ok(last)
}
