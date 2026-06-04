//! Polkit helper: invoked via D-Bus as `com.blackglass.start-core`.
//!
//! Polkit runs the helper as root; the original user is identified by
//! `PKEXEC_UID` (or `SUDO_UID` if the operator used sudo). The helper
//! re-checks everything the polkit policy already checked, in code,
//! as defense in depth.
//!
//! On invocation:
//!  1. The requested command must be the canonical `/usr/bin/blackglass-core`.
//!  2. The calling user must exist and be in the `blackglass` group.
//!  3. The core must not already be running (PID file check).
//!  4. We exec the core, inheriting the operator's identity via env.
//!
//! See `packaging/polkit/com.blackglass.policy` for the polkit side.

use anyhow::{bail, Result};
use nix::unistd::{Group, Uid, User};
use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};

/// The canonical core binary path. The helper will only ever exec this.
pub const CORE_BINARY: &str = "/usr/bin/blackglass-core";
/// The `blackglass` system group; the caller must be a member.
pub const BLACKGLASS_GROUP: &str = "blackglass";
/// PID file written by the core on startup. The helper refuses to
/// start a second copy while this file points to a live process.
pub const PID_FILE: &str = "/var/run/blackglass/core.pid";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The helper should exec the core with the given env.
    Exec { env: Vec<(OsString, OsString)> },
    /// The helper should refuse. The reason is for the audit log
    /// (polkit also logs to syslog).
    Reject { reason: String },
}

/// The decision function. Pure, testable. The main() binary is a thin
/// wrapper around this.
pub fn decide(caller_uid: u32, requested_command: &str, pid_file: &Path) -> Decision {
    // 1. The command must be the canonical core binary.
    if requested_command != CORE_BINARY {
        return Decision::Reject {
            reason: format!("only {CORE_BINARY} is allowed; got {requested_command}"),
        };
    }
    // 2. The caller must exist.
    let caller = match User::from_uid(Uid::from_raw(caller_uid)) {
        Ok(Some(u)) => u,
        Ok(None) => {
            return Decision::Reject {
                reason: format!("uid {caller_uid} has no user"),
            };
        }
        Err(e) => {
            return Decision::Reject {
                reason: format!("uid lookup failed: {e}"),
            };
        }
    };
    // 3. The blackglass group must exist.
    let grp = match Group::from_name(BLACKGLASS_GROUP) {
        Ok(Some(g)) => g,
        Ok(None) => {
            return Decision::Reject {
                reason: format!("{BLACKGLASS_GROUP} group not found"),
            };
        }
        Err(e) => {
            return Decision::Reject {
                reason: format!("group lookup failed: {e}"),
            };
        }
    };
    // 4. The caller must be in the group.
    match nix::unistd::getgrouplist(
        std::ffi::CString::new(caller.name.as_str()).as_deref().unwrap_or(c"/"),
        grp.gid,
    ) {
        Ok(groups) if groups.contains(&grp.gid) => {}
        Ok(_) => {
            return Decision::Reject {
                reason: format!(
                    "user {} is not in the {BLACKGLASS_GROUP} group",
                    caller.name
                ),
            };
        }
        Err(e) => {
            return Decision::Reject {
                reason: format!("getgrouplist failed: {e}"),
            };
        }
    }
    // 5. The core must not already be running.
    if pid_file.exists() {
        if let Ok(pid_str) = std::fs::read_to_string(pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                let proc = PathBuf::from(format!("/proc/{pid}"));
                if proc.exists() {
                    return Decision::Reject {
                        reason: format!("core already running with pid {pid}"),
                    };
                }
            }
        }
        // Stale PID file. The exec() will recreate it.
    }
    // All checks passed; exec.
    Decision::Exec {
        env: vec![
            (OsString::from("BLACKGLASS_OPERATOR"), OsString::from(&caller.name)),
            (
                OsString::from("BLACKGLASS_OPERATOR_UID"),
                OsString::from(caller_uid.to_string()),
            ),
        ],
    }
}

/// The `main()` binary. Parses the env, calls `decide`, and execs
/// or bails. The command is taken from argv[1] for simplicity in
/// unit tests (the polkit policy passes it via D-Bus, but in tests
/// we invoke the binary directly).
pub fn run(caller_uid: u32, requested_command: &str) -> Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    match decide(caller_uid, requested_command, Path::new(PID_FILE)) {
        Decision::Exec { env } => {
            let mut cmd = std::process::Command::new(requested_command);
            for (k, v) in env {
                cmd.env(k, v);
            }
            // execve replaces the process. Command::exec returns
            // only on error.
            let err = cmd.exec();
            bail!("exec failed: {err}");
        }
        Decision::Reject { reason } => {
            tracing::warn!(reason = %reason, "polkit-helper rejected request");
            bail!(reason);
        }
    }
}
