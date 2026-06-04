use blackglass_core::mcp_spawn_config::{McpServerSpec, McpSpawnConfig};
use blackglass_core::mcp_supervisor::McpSupervisor;
use std::time::Duration;
use tempfile::tempdir;

fn spec(name: &str, cmd: &str, args: &[&str]) -> McpServerSpec {
    McpServerSpec {
        name: name.into(),
        command: cmd.into(),
        args: args.iter().map(|s| s.to_string()).collect(),
        startup_timeout_ms: 5_000,
        max_restarts: 3,
    }
}

#[tokio::test]
async fn supervisor_spawns_a_long_running_child_and_sees_it_alive() {
    let dir = tempdir().unwrap();
    let config = McpSpawnConfig {
        servers: vec![spec("sleeper", "/bin/sh", &["-c", "sleep 30"])],
    };
    let log_path = dir.path().join("supervisor.log");
    let sup = McpSupervisor::start(config, &log_path).await.unwrap();
    // Give it a moment to spawn.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let status = sup.status("sleeper").await;
    assert_eq!(status, Some(blackglass_core::mcp_supervisor::ChildStatus::Alive));
    sup.shutdown().await;
}

#[tokio::test]
async fn supervisor_restarts_a_dying_child_with_backoff() {
    let dir = tempdir().unwrap();
    // Script that exits immediately. With max_restarts=3, the supervisor
    // should restart it 3 times before giving up.
    let config = McpSpawnConfig {
        servers: vec![spec("crasher", "/bin/sh", &["-c", "exit 1"])],
    };
    let log_path = dir.path().join("supervisor.log");
    let sup = McpSupervisor::start(config, &log_path).await.unwrap();
    // Wait for the backoff sequence: 1s + 2s + 4s = 7s minimum.
    tokio::time::sleep(Duration::from_secs(8)).await;
    let status = sup.status("crasher").await;
    assert_eq!(status, Some(blackglass_core::mcp_supervisor::ChildStatus::GivenUp { restart_count: 3 }));
    sup.shutdown().await;
}

#[tokio::test]
async fn supervisor_emits_mcp_server_exited_audit_events() {
    let dir = tempdir().unwrap();
    let chain_path = dir.path().join("chain.jsonl");
    let config = McpSpawnConfig {
        servers: vec![spec("crasher", "/bin/sh", &["-c", "exit 1"])],
    };
    let log_path = dir.path().join("supervisor.log");
    let sup = McpSupervisor::start_with_chain(config, &log_path, &chain_path).await.unwrap();
    tokio::time::sleep(Duration::from_secs(8)).await;
    let chain = blackglass_audit::Chain::open(&chain_path).unwrap();
    // Chain::query returns an AuditPage; events live at `.events`.
    let page = chain.query(&serde_json::json!({ "kind": "all" }), 0, 1000).unwrap();
    let exited: Vec<_> = page.events.iter().filter(|e| matches!(e.kind, blackglass_audit::EventKind::McpServerExited { .. })).collect();
    assert!(exited.len() >= 3, "expected >=3 McpServerExited events, got {}", exited.len());
    sup.shutdown().await;
}

#[tokio::test]
async fn supervisor_spawns_emits_mcp_server_spawned_audit_event() {
    let dir = tempdir().unwrap();
    let chain_path = dir.path().join("chain.jsonl");
    let config = McpSpawnConfig {
        servers: vec![spec("sleeper", "/bin/sh", &["-c", "sleep 30"])],
    };
    let log_path = dir.path().join("supervisor.log");
    let sup = McpSupervisor::start_with_chain(config, &log_path, &chain_path).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let chain = blackglass_audit::Chain::open(&chain_path).unwrap();
    let page = chain.query(&serde_json::json!({ "kind": "all" }), 0, 1000).unwrap();
    let spawned: Vec<_> = page.events.iter().filter(|e| matches!(e.kind, blackglass_audit::EventKind::McpServerSpawned { .. })).collect();
    assert_eq!(spawned.len(), 1);
    sup.shutdown().await;
}
