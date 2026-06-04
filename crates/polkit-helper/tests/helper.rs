//! TDD tests for the polkit-helper. See plan §3.1, §3.2.
//!
//! These exercise the pure `decide()` function. The actual `run()`
//! function exec()s the core and is integration-tested by the
//! `confinement-test` subcommand of xtask.

use blackglass_polkit_helper::{BLACKGLASS_GROUP, CORE_BINARY, Decision, decide};
use std::path::PathBuf;

fn pid_file_in(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("core.pid")
}

#[test]
fn rejects_non_core_command() {
    let dir = tempfile::TempDir::new().unwrap();
    let d = decide(0, "/bin/sh", &pid_file_in(&dir));
    match d {
        Decision::Reject { reason } => {
            assert!(reason.contains(CORE_BINARY), "reason should mention core: {reason}");
            assert!(reason.contains("/bin/sh"));
        }
        Decision::Exec { .. } => panic!("should have rejected /bin/sh"),
    }
}

#[test]
fn rejects_unknown_command_with_no_path() {
    let dir = tempfile::TempDir::new().unwrap();
    let d = decide(0, "", &pid_file_in(&dir));
    assert!(matches!(d, Decision::Reject { .. }));
}

#[test]
fn integration_binary_rejects_non_core_command() {
    // The actual binary should bail when given a non-core command.
    // This skips the `decide` unit test and exercises main() too.
    let bin = env!("CARGO_BIN_EXE_blackglass-polkit-helper");
    let out = std::process::Command::new(bin)
        .args(["/bin/sh"])
        .env("PKEXEC_UID", "0")
        .env_remove("SUDO_UID")
        .output()
        .expect("run helper");
    assert!(!out.status.success(), "helper should reject /bin/sh");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(CORE_BINARY) || stderr.contains("only"),
        "stderr should explain the rejection: {stderr}"
    );
}

#[test]
fn integration_binary_rejects_missing_uid() {
    let bin = env!("CARGO_BIN_EXE_blackglass-polkit-helper");
    let out = std::process::Command::new(bin)
        .env_remove("PKEXEC_UID")
        .env_remove("SUDO_UID")
        .output()
        .expect("run helper");
    assert!(!out.status.success(), "helper should fail with no PKEXEC_UID");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("PKEXEC_UID") || stderr.contains("SUDO_UID"),
        "stderr should mention the missing env: {stderr}"
    );
}

#[test]
fn rejects_already_running_pid() {
    // This test requires the `blackglass` group to exist on the host.
    // In CI it doesn't, so we skip the test there. The pid-file check
    // is exercised by the static-decide path in production (where the
    // group does exist).
    let grp = nix::unistd::Group::from_name(BLACKGLASS_GROUP)
        .ok()
        .flatten();
    if grp.is_none() {
        eprintln!(
            "skipping: '{BLACKGLASS_GROUP}' group not present; cannot exercise the \
             pid-already-running path"
        );
        return;
    }
    let dir = tempfile::TempDir::new().unwrap();
    let pid_path = pid_file_in(&dir);
    // Write a PID that's almost certainly alive: our own.
    let my_pid = std::process::id();
    std::fs::write(&pid_path, my_pid.to_string()).unwrap();
    let d = decide(0, CORE_BINARY, &pid_path);
    match d {
        Decision::Reject { reason } => {
            assert!(reason.contains("already running"), "reason: {reason}");
            assert!(reason.contains(&my_pid.to_string()));
        }
        Decision::Exec { .. } => panic!("should have rejected; pid {} is alive", my_pid),
    }
}

#[test]
fn accepts_stale_pid_file() {
    // Same skip-if-no-group caveat.
    let grp = nix::unistd::Group::from_name(BLACKGLASS_GROUP)
        .ok()
        .flatten();
    if grp.is_none() {
        eprintln!("skipping: '{BLACKGLASS_GROUP}' group not present");
        return;
    }
    let dir = tempfile::TempDir::new().unwrap();
    let pid_path = pid_file_in(&dir);
    std::fs::write(&pid_path, "2147483647").unwrap();
    let d = decide(0, CORE_BINARY, &pid_path);
    if let Decision::Reject { reason } = &d {
        assert!(!reason.contains("already running"), "stale PID should not block: {reason}");
    }
}

#[test]
fn blackglass_group_name_is_what_we_expect() {
    // A pin test: the policy file in packaging/ uses this exact
    // string. If we ever rename the group, both have to change.
    assert_eq!(BLACKGLASS_GROUP, "blackglass");
    assert_eq!(CORE_BINARY, "/usr/bin/blackglass-core");
}
