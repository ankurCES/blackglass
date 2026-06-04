pub mod commands;
pub mod operator_client;

pub fn build_confirm_resolve(id: &str, decision: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": null,
        "method": "confirm.resolve",
        "params": { "id": id, "decision": decision }
    })
}

/// Shared state managed by Tauri at startup. Holds the paths and
/// tokens the Tauri commands need to talk to the core's operator
/// socket. See design §2.4a.
pub struct AppState {
    pub operator_sock_path: std::path::PathBuf,
    pub operator_token: String,
}
