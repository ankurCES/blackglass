// Tauri shell. Sub-plan 3: connects to `~/.local/share/blackglass/operator.sock`,
// emits `operator-event` for every server-pushed event to the Svelte UI.
// The UI's confirmation flow (Task 13-15) listens to this channel and
// sends `confirm-resolve` invocations back through it.

use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

use blackglass_app::build_confirm_resolve;
use blackglass_app::AppState;

#[derive(Clone, Serialize)]
struct OperatorEvent {
    kind: String,
    raw: serde_json::Value,
}

#[tokio::main]
async fn main() {
    // Compute the operator socket path + token once, at startup, and
    // hand them to Tauri as managed state. The 3 new Tauri commands
    // (mcp_run_tool_cmd, mcp_list_tools_cmd, audit_event_cmd) pull
    // them out of `State<AppState>` per-call.
    let data_dir = data_dir().join("blackglass");
    std::fs::create_dir_all(&data_dir).ok();
    let operator_sock_path = data_dir.join("operator.sock");
    let operator_token = std::fs::read_to_string(data_dir.join("operator.token"))
        .unwrap_or_default()
        .trim_end_matches('\n')
        .to_string();
    let app_state = AppState {
        operator_sock_path,
        operator_token,
    };

    tauri::Builder::default()
        .manage(app_state)
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = run_socket_loop(handle).await {
                    eprintln!("operator socket loop error: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            confirm_resolve,
            blackglass_app::commands::mcp_run_tool_cmd,
            blackglass_app::commands::mcp_list_tools_cmd,
            blackglass_app::commands::audit_event_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running blackglass app");
}

async fn run_socket_loop(app: AppHandle) -> std::io::Result<()> {
    let sock = operator_sock_path()?;
    // Wait for the socket to exist (core may not be up yet).
    for _ in 0..100 {
        if sock.exists() { break; }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let stream = tokio::net::UnixStream::connect(&sock).await?;
    let (read, mut write) = stream.into_split();
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut lines = BufReader::new(read).lines();

    // Initial ping to confirm the connection.
    write
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n")
        .await?;
    write.flush().await?;

    // Store the write half in app state so `confirm_resolve` can use it.
    app.manage(SocketWrite(tokio::sync::Mutex::new(write)));

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            // Server-pushed `confirm.request` events have a `method` field.
            if v.get("method").and_then(|m| m.as_str()) == Some("confirm.request") {
                let _ = app.emit(
                    "operator-event",
                    OperatorEvent { kind: "confirm.request".into(), raw: v },
                );
            }
        }
    }
    Ok(())
}

struct SocketWrite(tokio::sync::Mutex<tokio::net::unix::OwnedWriteHalf>);

#[tauri::command]
async fn confirm_resolve(
    state: tauri::State<'_, SocketWrite>,
    id: String,
    decision: String,
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    let payload = build_confirm_resolve(&id, &decision);
    let mut w = state.0.lock().await;
    w.write_all(payload.to_string().as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    w.write_all(b"\n").await.map_err(|e| e.to_string())?;
    w.flush().await.map_err(|e| e.to_string())?;
    Ok(())
}

fn operator_sock_path() -> std::io::Result<PathBuf> {
    Ok(data_dir().join("blackglass").join("operator.sock"))
}

fn data_dir() -> PathBuf {
    match std::env::var("XDG_DATA_HOME") {
        Ok(v) => PathBuf::from(v),
        Err(_) => PathBuf::from(
            std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()),
        )
        .join(".local")
        .join("share"),
    }
}
