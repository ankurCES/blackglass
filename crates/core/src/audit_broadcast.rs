//! Bridge between the audit chain (the single-writer log of forensic
//! events) and the operator-socket `audit.event` push channel.
//!
//! Every code path in the core that calls `chain.append(event)` should
//! instead call `append_and_broadcast(&chain, &tx, event)`. This
//! guarantees two properties:
//!
//! 1. **Single point of truth.** The chain and the broadcast channel
//!    always see the same event in the same order (the chain is appended
//!    first; broadcast is best-effort after). Callers don't have to
//!    remember to do both, and they can't accidentally skip the
//!    broadcast.
//!
//! 2. **Best-effort delivery.** `tx.send(...)` may fail (no subscribers,
//!    or subscribers lagged). We swallow the error: a missed push is not
//!    a correctness problem for the audit log itself, and the chain
//!    is still authoritative. Subscribers can re-read the chain via
//!    `audit.query` if they fall behind.
//!
//! The post-append `payload["_hash"]` injection lets a subscriber
//! correlate the pushed event with the persisted event (the chain
//! hash of the event is also in the chain file, but the push carries
//! the event as a JSON object — adding the hash inline avoids a
//! second lookup).

use blackglass_audit::{Chain, Event};
use tokio::sync::broadcast;

/// Append `event` to the chain and best-effort broadcast it to any
/// subscribed operator clients. Returns the chain's hash of the
/// appended event on success. The broadcast `SendError` is ignored:
/// "no subscribers" is a normal state (e.g. nobody has called
/// `subscribe` yet).
///
/// Callers MUST hold any lock the chain is guarded by (e.g. a
/// `Mutex<Chain>` in `mcp_run_tool.rs`) — this function does NOT
/// lock the chain, it just calls `&mut Chain::append` via `&mut self`.
pub fn append_and_broadcast(
    chain: &mut Chain,
    tx: &broadcast::Sender<Event>,
    mut event: Event,
) -> Result<String, blackglass_audit::AuditError> {
    let hash = chain.append(event.clone())?;
    // Tag the broadcast copy with the just-computed hash so subscribers
    // can correlate the push with the on-disk record without a second
    // query. We mutate the local `event` (the chain has already been
    // appended, so the mutation doesn't leak).
    if let serde_json::Value::Object(ref mut map) = event.payload {
        map.insert("_hash".to_string(), serde_json::Value::String(hash.clone()));
    } else {
        // payload was e.g. `null` or not an object — wrap it so the
        // hash field still has a home. In practice every emission
        // path uses `json!({...})`, so this branch is defensive.
        let mut map = serde_json::Map::new();
        if !event.payload.is_null() {
            map.insert("payload".to_string(), event.payload);
        }
        map.insert("_hash".to_string(), serde_json::Value::String(hash.clone()));
        event.payload = serde_json::Value::Object(map);
    }
    let _ = tx.send(event); // best-effort
    Ok(hash)
}
