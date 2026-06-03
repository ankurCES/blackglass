// Tauri shell. Sub-plan 3: connects to `~/.local/share/blackglass/operator.sock`,
// emits `operator-event` for every server-pushed event to the Svelte UI.
// The UI's confirmation flow (Task 13-15) listens to this channel and
// sends `confirm-resolve` invocations back through it.

use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize)]
struct OperatorEvent {
    kind: String,
    raw: serde_json::Value,
}

#[tokio::main]
async fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = run_socket_loop(handle).await {
                    eprintln!("operator socket loop error: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![confirm_resolve])
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
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": null,
        "method": "confirm.resolve",
        "params": { "id": id, "decision": decision }
    });
    let mut w = state.0.lock().await;
    w.write_all(payload.to_string().as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    w.write_all(b"\n").await.map_err(|e| e.to_string())?;
    w.flush().await.map_err(|e| e.to_string())?;
    Ok(())
}

fn operator_sock_path() -> std::io::Result<PathBuf> {
    let dir = match std::env::var("XDG_DATA_HOME") {
        Ok(v) => PathBuf::from(v),
        Err(_) => PathBuf::from(
            std::env::var("HOME")
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?,
        )
        .join(".local")
        .join("share"),
    };
    Ok(dir.join("blackglass").join("operator.sock"))
}
