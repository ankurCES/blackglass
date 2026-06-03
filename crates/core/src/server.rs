//! Unix-socket RPC server. See ADR 0001, 0004.
//!
//! Auth is per-connection: the client must send an Auth request before any
//! other method on the same connection. Each new connection starts unauthenticated.

use crate::chokepoint::{self, Chokepoint};
use crate::rpc::{Method, RpcRequest, RpcResponse};
use blackglass_ipc::encode_frame;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tracing::{info, warn};

pub struct Server {
    socket_path: std::path::PathBuf,
    expected_token: String,
    chokepoint: Arc<Mutex<Chokepoint>>,
    listener: UnixListener,
}

impl Server {
    /// Remove any stale socket, bind a new one, and return a ready Server.
    /// The socket is live as soon as this returns — callers may connect before
    /// `serve()` starts its accept loop; the kernel queues those connections.
    pub async fn bind(
        socket_path: impl AsRef<Path>,
        expected_token: String,
        chokepoint: Chokepoint,
    ) -> std::io::Result<Self> {
        let path = socket_path.as_ref();
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path)?;
        Ok(Self {
            socket_path: path.to_path_buf(),
            expected_token,
            chokepoint: Arc::new(Mutex::new(chokepoint)),
            listener,
        })
    }

    pub async fn serve(self) -> std::io::Result<()> {
        info!(socket = %self.socket_path.display(), "core listening");
        loop {
            let (stream, _addr) = self.listener.accept().await?;
            let cp = self.chokepoint.clone();
            let token = self.expected_token.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_conn(stream, cp, token).await {
                    warn!(error = %e, "connection ended with error");
                }
            });
        }
    }
}

async fn handle_conn(
    mut stream: UnixStream,
    cp: Arc<Mutex<Chokepoint>>,
    expected_token: String,
) -> std::io::Result<()> {
    let mut authenticated = false;
    loop {
        let mut lenb = [0u8; 4];
        if stream.read_exact(&mut lenb).await.is_err() {
            return Ok(()); // client closed connection
        }
        let len = u32::from_be_bytes(lenb) as usize;
        if len > blackglass_ipc::MAX_FRAME {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too large"));
        }
        let mut payload = vec![0u8; len];
        stream.read_exact(&mut payload).await?;

        let resp = match serde_json::from_slice::<RpcRequest>(&payload) {
            Err(e) => RpcResponse {
                id: 0,
                ok: false,
                result: None,
                error: Some(format!("bad request: {e}")),
            },
            Ok(req) => dispatch(req, &mut authenticated, &expected_token, cp.clone()).await,
        };

        let bytes = serde_json::to_vec(&resp)
            .map_err(std::io::Error::other)?;
        stream.write_all(&encode_frame(&bytes)).await?;
    }
}

async fn dispatch(
    req: RpcRequest,
    authenticated: &mut bool,
    expected_token: &str,
    cp: Arc<Mutex<Chokepoint>>,
) -> RpcResponse {
    match req.method {
        Method::Auth { token } => {
            if token == expected_token {
                *authenticated = true;
                RpcResponse { id: req.id, ok: true, result: Some(json!({"ok": true})), error: None }
            } else {
                RpcResponse {
                    id: req.id,
                    ok: false,
                    result: None,
                    error: Some("bad token".into()),
                }
            }
        }
        Method::Ping => {
            if !*authenticated {
                return RpcResponse {
                    id: req.id,
                    ok: false,
                    result: None,
                    error: Some("not authenticated".into()),
                };
            }
            RpcResponse { id: req.id, ok: true, result: Some(json!({"pong": true})), error: None }
        }
        Method::ExecuteAction(ar) => {
            if !*authenticated {
                return RpcResponse {
                    id: req.id,
                    ok: false,
                    result: None,
                    error: Some("not authenticated".into()),
                };
            }
            let mut guard = cp.lock().await;
            match chokepoint::execute_action(&mut guard, ar).await {
                Ok(outcome) => RpcResponse {
                    id: req.id,
                    ok: true,
                    result: Some(json!({
                        "stdout": outcome.stdout(),
                        "stderr": outcome.stderr(),
                    })),
                    error: None,
                },
                Err(e) => RpcResponse {
                    id: req.id,
                    ok: false,
                    result: None,
                    error: Some(e.to_string()),
                },
            }
        }
    }
}
