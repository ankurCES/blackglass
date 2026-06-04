//! Hash-chained append-only audit log.
//!
//! Each [`Event`] carries the blake3 hash of the previous event. The chain
//! is verified by [`Chain::verify`]. See ADR 0002.

use blake3::Hasher;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
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
    PythonBridgeInvoked,
    PythonBridgeFailed,
    PythonBridgeEvidenceDumped,
    McpServerSpawned { server: String, pid: u32 },
    McpServerExited { server: String, code: i32, restart_count: u32 },
    McpRunStarted { domain: String, target: String },
    McpRunCompleted { domain: String, target: String, ok: bool, ms: u64 },
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

// ---------------------------------------------------------------------------
// Query + verify_chain_in_band
// ---------------------------------------------------------------------------
//
// These power the Tauri audit browser (see plan §2.4). They're methods on
// `&self` so the core can call them via `Audit::query(...)` after a
// successful `open`. They're sync because the audit log is small (< 1M
// events) and a JSONL scan is fast enough (< 500ms for 100k events on
// 2026-era hardware).

/// A page of audit events, with the head hash and a total count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditPage {
    pub events: Vec<Event>,
    pub total_matched: u64,
    pub hash_chain_head: String,
    pub hash_chain_verified: bool,
    pub query_ms: u64,
}

/// The result of a chain verification pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainVerification {
    pub verified: bool,
    pub total_events: u64,
    pub broken_at_seq: Option<u64>,
    pub root_hash: String,
    pub last_checkpoint_seq: Option<u64>,
    pub errors: Vec<ChainError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainError {
    pub seq: u64,
    pub expected_hash: String,
    pub actual_hash: String,
    pub reason: String,
}

impl Chain {
    /// Scan the audit log and return the events that match `filter`, sliced
    /// to `[page*page_size, (page+1)*page_size)`.
    ///
    /// Filter grammar (matches the design doc's §13.3):
    /// - `{"kind":"all"}` — every event
    /// - `{"kind":"kind","kinds":["action_executed", ...]}` — by EventKind
    /// - `{"kind":"and","clauses":[...]}` — logical AND
    /// - `{"kind":"or","clauses":[...]}` — logical OR
    /// - `{"kind":"not","clause":{...}}` — logical NOT
    /// - `{"kind":"seq_range","min":N,"max":M}` — by sequence number
    /// - `{"kind":"time_range","start":"...","end":"..."}` — by timestamp
    /// - `{"kind":"domain","domains":["..."]}` — by payload.domain
    /// - `{"kind":"tool","tools":["..."]}` — by payload.tool
    /// - `{"kind":"decision","decisions":["allowed","denied","pending","errored"]}`
    /// - `{"kind":"actor","actors":["..."]}` — by payload.actor
    /// - `{"kind":"target_match","substring":"..."}` — substring on payload
    /// - `{"kind":"session","session_id":"..."}` — by payload.session_id
    /// - unknown filter kinds match everything (forward-compat)
    pub fn query(&self, filter: &Value, page: u32, page_size: u32) -> Result<AuditPage, AuditError> {
        let start = std::time::Instant::now();
        let file = std::fs::File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut matched: Vec<Event> = Vec::new();
        let mut total = 0u64;
        for line in reader.lines() {
            let line = line?;
            if line.is_empty() { continue; }
            let v: Value = serde_json::from_str(&line).map_err(|_| AuditError::BadLine(0))?;
            let event_val = match v.get("event") {
                Some(e) => e,
                None => continue,
            };
            let obj = match event_val.as_object() {
                Some(o) => o.clone(),
                None => continue,
            };
            let event: Event = match serde_json::from_value(Value::Object(obj)) {
                Ok(e) => e,
                Err(_) => continue,
            };
            if matches_filter(&event, filter) {
                total += 1;
                matched.push(event);
            }
        }
        let start_idx = (page as usize).saturating_mul(page_size as usize);
        let end_idx = std::cmp::min(start_idx + page_size as usize, matched.len());
        let page_events: Vec<Event> = if start_idx >= matched.len() {
            Vec::new()
        } else {
            matched[start_idx..end_idx].to_vec()
        };
        let head = self.last.clone().unwrap_or_else(|| "0".repeat(64));
        let verified = Chain::verify(&self.path).is_ok();
        Ok(AuditPage {
            events: page_events,
            total_matched: total,
            hash_chain_head: head,
            hash_chain_verified: verified,
            query_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Verify the chain end-to-end. Returns a [`ChainVerification`] that
    /// includes a per-event error list and the recomputed root hash. This
    /// is the "in-band" variant because it doesn't take a path argument;
    /// the static [`Chain::verify`] is the path-based form used by the
    /// postinstall and the smoke-test.
    pub fn verify_chain_in_band(&self) -> Result<ChainVerification, AuditError> {
        let s = std::fs::read_to_string(&self.path)?;
        let mut prev = "0".repeat(64);
        let mut events: Vec<Event> = Vec::new();
        let mut errors: Vec<ChainError> = Vec::new();
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
                errors.push(ChainError {
                    seq: e.seq,
                    expected_hash: prev.clone(),
                    actual_hash: e.prev_hash.clone(),
                    reason: "prev_hash mismatch".into(),
                });
            }
            let computed = e.hash()?;
            if computed != claimed {
                errors.push(ChainError {
                    seq: e.seq,
                    expected_hash: computed.clone(),
                    actual_hash: claimed.to_string(),
                    reason: "computed hash mismatch".into(),
                });
            }
            prev = computed;
            events.push(e);
        }
        let last_checkpoint_seq = events
            .iter()
            .rev()
            .find(|e| matches!(e.kind, EventKind::OperatorConfirmationResolved))
            .map(|e| e.seq);
        Ok(ChainVerification {
            verified: errors.is_empty(),
            total_events: events.len() as u64,
            broken_at_seq: errors.first().map(|e| e.seq),
            root_hash: prev,
            last_checkpoint_seq,
            errors,
        })
    }
}

fn matches_filter(event: &Event, filter: &Value) -> bool {
    let kind = filter.get("kind").and_then(|k| k.as_str()).unwrap_or("all");
    match kind {
        "all" => true,
        "kind" => {
            let wanted: Vec<String> = filter
                .get("kinds")
                .and_then(|k| k.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            // EventKind serializes as `{"kind": "<variant>"}` because of the
            // `tag = "kind"` attribute. Extract the inner string.
            let event_kind_str = serde_json::to_value(&event.kind)
                .ok()
                .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(String::from))
                .unwrap_or_default();
            wanted.iter().any(|k| k == &event_kind_str)
        }
        "and" => filter
            .get("clauses")
            .and_then(|c| c.as_array())
            .map(|a| a.iter().all(|c| matches_filter(event, c)))
            .unwrap_or(true),
        "or" => filter
            .get("clauses")
            .and_then(|c| c.as_array())
            .map(|a| a.iter().any(|c| matches_filter(event, c)))
            .unwrap_or(false),
        "not" => filter
            .get("clause")
            .map(|c| !matches_filter(event, c))
            .unwrap_or(true),
        "seq_range" => {
            let seq = event.seq;
            let min = filter.get("min").and_then(|v| v.as_u64()).unwrap_or(0);
            let max = filter.get("max").and_then(|v| v.as_u64()).unwrap_or(u64::MAX);
            seq >= min && seq <= max
        }
        "time_range" => {
            let after = filter
                .get("start")
                .and_then(|v| v.as_str())
                .map(|s| s <= event.ts.as_str())
                .unwrap_or(true);
            let before = filter
                .get("end")
                .and_then(|v| v.as_str())
                .map(|s| event.ts.as_str() <= s)
                .unwrap_or(true);
            after && before
        }
        "domain" => {
            let wanted: Vec<&str> = filter
                .get("domains")
                .and_then(|k| k.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            wanted.iter().any(|d| {
                event.payload.get("domain").and_then(|v| v.as_str()) == Some(d)
            })
        }
        "tool" => {
            let wanted: Vec<&str> = filter
                .get("tools")
                .and_then(|k| k.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            wanted.iter().any(|t| {
                event.payload.get("tool").and_then(|v| v.as_str()) == Some(t)
            })
        }
        "decision" => {
            let wanted: Vec<String> = filter
                .get("decisions")
                .and_then(|k| k.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let actual = event
                .payload
                .get("decision")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            wanted.iter().any(|d| d == actual)
        }
        "actor" => {
            let wanted: Vec<&str> = filter
                .get("actors")
                .and_then(|k| k.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            wanted.iter().any(|d| {
                event.payload.get("actor").and_then(|v| v.as_str()) == Some(d)
            })
        }
        "target_match" => {
            let needle = filter.get("substring").and_then(|v| v.as_str()).unwrap_or("");
            if needle.is_empty() {
                true
            } else {
                event.payload.to_string().contains(needle)
            }
        }
        "session" => {
            let wanted = filter.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
            event.payload.get("session_id").and_then(|v| v.as_str()) == Some(wanted)
        }
        // Unknown filter kinds: don't surprise the user by hiding events.
        // Forward-compat: the new filter is just ignored.
        _ => true,
    }
}
