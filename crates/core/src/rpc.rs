//! Wire-format RPC methods exposed by the core. See ADR 0001, 0004.

use crate::gates::ActionRequest;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Internally-tagged enum; the "method" key in the flat JSON object is the tag.
/// Unknown methods fail to deserialize → server returns "bad request" error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum Method {
    Auth { token: String },
    ExecuteAction(ActionRequest),
    Ping,
}

/// Wire request. `method` is flattened so all fields live at the same JSON level:
///   {"id":1,"method":"ping"}
///   {"id":2,"method":"auth","token":"..."}
///   {"id":3,"method":"execute_action","domain":"...","action_class":"...","target":"...","args":{}}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub id: u64,
    #[serde(flatten)]
    pub method: Method,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub id: u64,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
