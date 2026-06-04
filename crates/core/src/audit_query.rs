//! Operator-socket `audit.query` and `audit.verify_chain` methods.
//!
//! These are pure pass-throughs to `blackglass_audit::Chain`. The audit
//! chain file is provided to the operator server at startup (see
//! `operator_server::run`); on disk it lives under
//! `~/.local/share/blackglass/audit/audit.jsonl` in production.
//!
//! No I/O orchestration happens here — `handle_query` only calls
//! `Chain::query(&self, …)`, and `handle_verify` only calls
//! `Chain::verify(path)`. The dispatch + error-to-JSON-RPC mapping lives
//! in `operator_server::handle_rpc`.

use blackglass_audit::Chain;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditQueryError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("audit: {0}")]
    Audit(#[from] blackglass_audit::AuditError),
}

/// Parameters for `audit.query`.
///
/// All fields default so a caller can pass `{}` and get the first page of
/// all events. `page_size` defaults to 50 — a reasonable initial-load
/// size for the Tauri audit browser.
#[derive(Debug, Deserialize)]
pub struct QueryParams {
    /// Filter grammar — see `Chain::query` docs in `blackglass_audit`.
    /// `{"kind":"all"}` is the default and matches every event.
    #[serde(default)]
    pub filter: serde_json::Value,
    #[serde(default)]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page_size() -> u32 {
    50
}

/// Response for `audit.query`.
///
/// Carries the full `AuditPage` from the audit crate (events, head hash,
/// chain-verified flag, query time) so the Tauri audit browser can show
/// "chain verified at <hash>" / "chain NOT verified" badges without a
/// second round-trip. The `page` and `page_size` echoes make it easier
/// for the UI to render pagination state.
#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub events: Vec<blackglass_audit::Event>,
    pub total_matched: u64,
    pub page: u32,
    pub page_size: u32,
    /// blake3 hash of the last event currently in the chain file
    /// (or 64 zeros if the chain is empty).
    pub hash_chain_head: String,
    /// `true` iff `Chain::verify` succeeded against the chain file at
    /// the time of the query.
    pub hash_chain_verified: bool,
    /// Wall-clock duration of the query in milliseconds.
    pub query_ms: u64,
}

pub fn handle_query(chain: &Chain, params: QueryParams) -> Result<QueryResponse, AuditQueryError> {
    let page = chain.query(&params.filter, params.page, params.page_size)?;
    Ok(QueryResponse {
        events: page.events,
        total_matched: page.total_matched,
        page: params.page,
        page_size: params.page_size,
        hash_chain_head: page.hash_chain_head,
        hash_chain_verified: page.hash_chain_verified,
        query_ms: page.query_ms,
    })
}

/// `audit.verify_chain` returns the count of valid events in the chain
/// file. `Chain::verify` is static + path-based, so we just delegate.
///
/// An `Ok(n)` result means the chain is intact and contains `n` events.
/// An `Err(_)` means the chain is broken; the dispatcher maps that to a
/// JSON-RPC error.
pub fn handle_verify(chain: &Chain) -> Result<u64, AuditQueryError> {
    Ok(Chain::verify(chain.path())?)
}
