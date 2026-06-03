pub fn build_confirm_resolve(id: &str, decision: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": null,
        "method": "confirm.resolve",
        "params": { "id": id, "decision": decision }
    })
}
