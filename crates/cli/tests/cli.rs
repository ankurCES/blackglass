use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn init_creates_dirs_and_token_file() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("rt.sock");
    let tok = dir.path().join("op.token");

    Command::cargo_bin("blackglass")
        .unwrap()
        .arg("--socket")
        .arg(&sock)
        .arg("--token-file")
        .arg(&tok)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("initialized"));

    assert!(sock.parent().unwrap().is_dir());
    assert!(tok.parent().unwrap().is_dir());
    assert!(tok.is_file());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&tok).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "token file mode is {:o}, expected 0600", mode);
    }

    let token = std::fs::read_to_string(&tok).unwrap();
    assert_eq!(token.trim().len(), 64, "expected 64-char hex token");
}

#[test]
fn audit_verify_succeeds_on_clean_log_and_fails_on_tampered() {
    use blackglass_audit::{Chain, Event, EventKind};
    use serde_json::json;

    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("a.jsonl");
    let sock = dir.path().join("rt.sock");
    let tok = dir.path().join("op.token");

    let mut chain = Chain::open(&log).unwrap();
    for i in 1..=3u64 {
        chain
            .append(Event {
                seq: i,
                ts: "2026-06-03T00:00:00Z".into(),
                prev_hash: String::new(),
                kind: EventKind::ActionRequested,
                payload: json!({"i": i}),
            })
            .unwrap();
    }
    drop(chain);

    // Clean log → success.
    Command::cargo_bin("blackglass")
        .unwrap()
        .arg("--socket")
        .arg(&sock)
        .arg("--token-file")
        .arg(&tok)
        .arg("audit-verify")
        .arg("--path")
        .arg(&log)
        .assert()
        .success()
        .stdout(predicate::str::contains("OK: 3"));

    // Tamper the second line's payload (change "i":2 to "i":999).
    let s = std::fs::read_to_string(&log).unwrap();
    let mut lines: Vec<String> = s.lines().map(|l| l.to_string()).collect();
    lines[1] = lines[1].replace("\"i\":2", "\"i\":999");
    std::fs::write(&log, lines.join("\n") + "\n").unwrap();

    // Tampered log → failure.
    Command::cargo_bin("blackglass")
        .unwrap()
        .arg("--socket")
        .arg(&sock)
        .arg("--token-file")
        .arg(&tok)
        .arg("audit-verify")
        .arg("--path")
        .arg(&log)
        .assert()
        .failure();
}
