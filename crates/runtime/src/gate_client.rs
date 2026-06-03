//! Async gate client — connects to blackglass-core Unix socket, authenticates,
//! and dispatches execute_action calls on behalf of MCP server tools.

use blackglass_core::rpc::{Method, RpcRequest, RpcResponse};
use blackglass_core::gates::ActionRequest;
use blackglass_ipc::encode_frame;
use serde_json::Value;
use std::path::PathBuf;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[derive(Debug, Error)]
pub enum GateError {
    #[error("connect to {0}: {1}")]
    Connect(PathBuf, std::io::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("auth failed: {0}")]
    AuthFailed(String),
    #[error("gate denied: {0}")]
    Denied(String),
}

pub struct GateOutcome {
    pub stdout: String,
    pub stderr: String,
}

pub struct GateClient {
    socket_path: PathBuf,
    token: String,
}

impl GateClient {
    pub fn new(socket_path: impl Into<PathBuf>, token: String) -> Self {
        Self { socket_path: socket_path.into(), token }
    }

    async fn open(&self) -> Result<UnixStream, GateError> {
        UnixStream::connect(&self.socket_path).await
            .map_err(|e| GateError::Connect(self.socket_path.clone(), e))
    }

    async fn send_recv(stream: &mut UnixStream, req: &RpcRequest) -> Result<RpcResponse, GateError> {
        let bytes = serde_json::to_vec(req)?;
        stream.write_all(&encode_frame(&bytes)).await?;
        let mut lenb = [0u8; 4];
        stream.read_exact(&mut lenb).await?;
        let n = u32::from_be_bytes(lenb) as usize;
        let mut buf = vec![0u8; n];
        stream.read_exact(&mut buf).await?;
        Ok(serde_json::from_slice(&buf)?)
    }

    async fn auth(&self, stream: &mut UnixStream) -> Result<(), GateError> {
        let resp = Self::send_recv(stream, &RpcRequest {
            id: 0,
            method: Method::Auth { token: self.token.clone() },
        }).await?;
        if !resp.ok {
            return Err(GateError::AuthFailed(resp.error.unwrap_or_default()));
        }
        Ok(())
    }

    pub async fn ping(&self) -> Result<(), GateError> {
        let mut s = self.open().await?;
        self.auth(&mut s).await?;
        let resp = Self::send_recv(&mut s, &RpcRequest {
            id: 1,
            method: Method::Ping,
        }).await?;
        if resp.ok { Ok(()) } else { Err(GateError::Denied(resp.error.unwrap_or_default())) }
    }

    pub async fn execute(
        &self,
        domain: &str,
        action_class: &str,
        target: &str,
        args: Value,
    ) -> Result<GateOutcome, GateError> {
        let mut s = self.open().await?;
        self.auth(&mut s).await?;
        let resp = Self::send_recv(&mut s, &RpcRequest {
            id: 1,
            method: Method::ExecuteAction(ActionRequest {
                domain: domain.to_string(),
                action_class: action_class.to_string(),
                target: target.to_string(),
                args,
            }),
        }).await?;
        if resp.ok {
            let result = resp.result.unwrap_or(Value::Null);
            Ok(GateOutcome {
                stdout: result.get("stdout").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                stderr: result.get("stderr").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            })
        } else {
            Err(GateError::Denied(resp.error.unwrap_or_default()))
        }
    }
}
