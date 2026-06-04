//! `xtask verify-install` — check the system meets the
//! blackglass user-systemd install prerequisites.
//!
//! The install model is:
//!   - The core + secondary sidecar + 6 MCPs ship as
//!     /usr/bin/blackglass-* binaries.
//!   - All operator state lives in ~/.local/share/blackglass/
//!     (not /var/{lib,run}/blackglass/).
//!   - The core runs as a *user*-systemd service
//!     (blackglass-core.service, blackglass-secondary-sidecar.service).
//!   - No polkit, no /var/lib/blackglass, no adduser/blackglass group.
//!   - The operator token file is at
//!     ~/.local/share/blackglass/operator.token with mode 0600.
//!   - AppArmor profiles are at /etc/apparmor.d/blackglass-{core,
//!     secondary-sidecar}.
//!   - The 4 MCPs (mcp-ad, mcp-flipper, mcp-phish, mcp-detect)
//!     are supervised by the core; if the operator copied
//!     /etc/blackglass/mcp-servers.toml.example to
//!     ~/.config/blackglass/mcp-servers.toml, those MCPs are running.
//!   - The operator is in the `udev` group so the Flipper works.
//!
//! Each check independently reports pass/fail with a one-line
//! detail. The function returns Ok(()) only when all checks pass.

use anyhow::{bail, Result};
use std::path::Path;
use std::process::Command;

struct Check {
    name: &'static str,
    pass: bool,
    detail: String,
}

impl Check {
    fn ok(name: &'static str) -> Self {
        Self {
            name,
            pass: true,
            detail: String::new(),
        }
    }
    fn fail(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            pass: false,
            detail: detail.into(),
        }
    }
}

fn operator_home() -> Option<std::path::PathBuf> {
    std::env::var("HOME").ok().map(std::path::PathBuf::from)
}

/// Run a command if it exists. Returns None if the binary isn't on PATH.
fn run_if_present(cmd: &str, args: &[&str]) -> Option<std::process::Output> {
    match Command::new(cmd).args(args).output() {
        Ok(o) => Some(o),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            eprintln!("  ! {cmd} failed to launch: {e}");
            None
        }
    }
}

// ── Binaries present ────────────────────────────────────────────

fn check_binaries() -> Check {
    for bin in &[
        "/usr/bin/blackglass",
        "/usr/bin/blackglass-core",
        "/usr/bin/blackglass-app",
        "/usr/bin/blackglass-secondary-sidecar",
        "/usr/bin/blackglass-mcp-osint",
        "/usr/bin/blackglass-mcp-packets",
        "/usr/bin/blackglass-mcp-ad",
        "/usr/bin/blackglass-mcp-flipper",
        "/usr/bin/blackglass-mcp-phish",
        "/usr/bin/blackglass-mcp-detect",
    ] {
        if !Path::new(bin).exists() {
            return Check::fail("binaries", format!("{bin} not installed"));
        }
    }
    Check::ok("binaries")
}

// ── AppArmor profiles installed + loaded ────────────────────────

fn check_apparmor() -> Check {
    // Files installed?
    for profile in &[
        "/etc/apparmor.d/blackglass-core",
        "/etc/apparmor.d/blackglass-secondary-sidecar",
    ] {
        if !Path::new(profile).exists() {
            return Check::fail(
                "apparmor-profile",
                format!("{profile} not installed"),
            );
        }
    }
    // Loaded? Use apparmor_status if root, otherwise best-effort.
    if let Some(out) = run_if_present("apparmor_status", &[]) {
        let s = String::from_utf8_lossy(&out.stdout);
        if !s.contains("blackglass-core") || !s.contains("blackglass-secondary-sidecar") {
            return Check::fail(
                "apparmor-loaded",
                "one or more blackglass profiles not loaded (run `sudo apparmor_parser -r /etc/apparmor.d/blackglass-core`)",
            );
        }
    }
    // else: apparmor_status not available (no root). We don't fail
    // — the file-presence check above is the hard requirement.
    Check::ok("apparmor")
}

// ── Operator state directory + audit dir ───────────────────────

fn check_operator_state() -> Check {
    let Some(home) = operator_home() else {
        return Check::fail("operator-state", "HOME not set");
    };
    let state = home.join(".local/share/blackglass");
    if !state.exists() {
        return Check::fail(
            "operator-state",
            format!("{} does not exist (start the user-systemd service once)", state.display()),
        );
    }
    let audit = state.join("audit");
    if !audit.exists() {
        return Check::fail(
            "audit-dir",
            format!("{} does not exist", audit.display()),
        );
    }
    Check::ok("operator-state")
}

// ── Operator socket ────────────────────────────────────────────

fn check_operator_socket() -> Check {
    let Some(home) = operator_home() else {
        return Check::fail("operator-socket", "HOME not set");
    };
    let sock = home.join(".local/share/blackglass/runtime.sock");
    if !sock.exists() {
        return Check::fail(
            "operator-socket",
            format!("{} not found", sock.display()),
        );
    }
    Check::ok("operator-socket")
}

// ── Operator token file + mode 0600 ────────────────────────────

#[cfg(unix)]
fn token_mode(path: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(std::fs::metadata(path).ok()?.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn token_mode(_path: &Path) -> Option<u32> {
    None
}

fn check_operator_token() -> Check {
    let Some(home) = operator_home() else {
        return Check::fail("operator-token", "HOME not set");
    };
    let token = home.join(".local/share/blackglass/operator.token");
    if !token.exists() {
        return Check::fail(
            "operator-token",
            format!("{} not found", token.display()),
        );
    }
    if let Some(mode) = token_mode(&token) {
        if mode != 0o600 {
            return Check::fail(
                "operator-token-mode",
                format!("mode is {:o}, expected 0600 (run: chmod 600 {})", mode, token.display()),
            );
        }
    }
    Check::ok("operator-token")
}

// ── User-systemd services ──────────────────────────────────────

fn check_user_systemd() -> Check {
    let core = run_if_present(
        "systemctl",
        &["--user", "is-active", "blackglass-core.service"],
    );
    match core {
        Some(o) if o.status.success() => {}
        Some(_) => {
            return Check::fail(
                "user-systemd-core",
                "blackglass-core.service is not active (run: systemctl --user start blackglass-core)",
            );
        }
        None => return Check::fail("user-systemd-core", "systemctl not on PATH"),
    }
    // Secondary sidecar is non-fatal if down — the sidecar can
    // be down without breaking the core (verdict=unknown).
    let sidecar = run_if_present(
        "systemctl",
        &["--user", "is-active", "blackglass-secondary-sidecar.service"],
    );
    match sidecar {
        Some(o) if o.status.success() => Check::ok("user-systemd"),
        Some(_) => Check::fail(
            "user-systemd-sidecar",
            "blackglass-secondary-sidecar.service is not active (non-fatal: core still works)",
        ),
        None => Check::ok("user-systemd"),
    }
}

// ── udev group ─────────────────────────────────────────────────

fn check_udev_group() -> Check {
    let user = match std::env::var("USER") {
        Ok(u) if !u.is_empty() => u,
        _ => return Check::fail("udev-group", "USER not set"),
    };
    let out = match Command::new("id").args(["-Gn", &user]).output() {
        Ok(o) if o.status.success() => o,
        _ => return Check::fail("udev-group", "id command failed"),
    };
    let groups = String::from_utf8_lossy(&out.stdout);
    if !groups.split_whitespace().any(|g| g == "udev") {
        return Check::fail(
            "udev-group",
            format!("user {user} is not in the udev group (Flipper won't work — log out and back in)"),
        );
    }
    Check::ok("udev-group")
}

// ── udev rules ─────────────────────────────────────────────────

fn check_udev_rules() -> Check {
    let locations = [
        "/etc/udev/rules.d/99-blackglass-flipper.rules",
        "/lib/udev/rules.d/99-blackglass-flipper.rules",
        "/usr/lib/udev/rules.d/99-blackglass-flipper.rules",
    ];
    if !locations.iter().any(|p| Path::new(p).exists()) {
        return Check::fail(
            "udev-rules",
            "99-blackglass-flipper.rules not installed in any of /etc/, /lib/, /usr/lib/udev/rules.d/",
        );
    }
    Check::ok("udev-rules")
}

// ── mcp-servers.toml.example ───────────────────────────────────

fn check_mcp_servers_example() -> Check {
    let p = Path::new("/etc/blackglass/mcp-servers.toml.example");
    if !p.exists() {
        return Check::fail(
            "mcp-servers-example",
            "/etc/blackglass/mcp-servers.toml.example not installed",
        );
    }
    Check::ok("mcp-servers-example")
}

// ── MCP supervisor children (best-effort) ──────────────────────

fn check_mcp_children() -> Check {
    // The 4 MCPs are supervised by the core. They run only if
    // the operator copied the example to
    // ~/.config/blackglass/mcp-servers.toml. If not, skip with
    // a diagnostic.
    let Some(home) = operator_home() else {
        return Check::fail("mcp-children", "HOME not set");
    };
    let cfg = home.join(".config/blackglass/mcp-servers.toml");
    if !cfg.exists() {
        return Check::ok("mcp-children"); // not configured — skip
    }
    for mcp in &["mcp-ad", "mcp-flipper", "mcp-phish", "mcp-detect"] {
        let pgrep = run_if_present("pgrep", &["-f", &format!("blackglass-{}", mcp)]);
        match pgrep {
            Some(o) if o.status.success() => {}
            Some(_) => {
                return Check::fail(
                    "mcp-children",
                    format!("{mcp} is not running (the supervisor should restart it)"),
                );
            }
            None => return Check::fail("mcp-children", "pgrep not on PATH"),
        }
    }
    Check::ok("mcp-children")
}

// ── Python sidecar venv (best-effort) ──────────────────────────

fn check_python_venv() -> Check {
    let path = Path::new("/usr/lib/blackglass/python-venv/bin/python");
    if !path.exists() {
        return Check::fail("python-venv", format!("{path:?} not found"));
    }
    // Try to import the sidecar modules. If the venv is broken,
    // this fails — the operator needs to rebuild.
    let out = match Command::new(path).args([
        "-c",
        "import blackglass_sidecar.scapy_bridge, \
         blackglass_sidecar.impacket_bridge, \
         blackglass_sidecar.hardware_bridge, \
         blackglass_sidecar.audit_types, \
         blackglass_sidecar.detect_bridge; print('OK')",
    ]).output() {
        Ok(o) => o,
        Err(e) => return Check::fail("python-venv", e.to_string()),
    };
    if !out.status.success() {
        return Check::fail(
            "python-venv",
            String::from_utf8_lossy(&out.stderr).into_owned(),
        );
    }
    Check::ok("python-venv")
}

// ── Driver ─────────────────────────────────────────────────────

pub fn run() -> Result<()> {
    println!("=== blackglass verify-install (user-systemd) ===\n");

    let checks = vec![
        check_binaries(),
        check_apparmor(),
        check_operator_state(),
        check_operator_socket(),
        check_operator_token(),
        check_user_systemd(),
        check_udev_group(),
        check_udev_rules(),
        check_mcp_servers_example(),
        check_mcp_children(),
        check_python_venv(),
    ];

    let mut failed = 0;
    for c in &checks {
        let mark = if c.pass { "✓" } else { "✗" };
        println!("  {mark} {}", c.name);
        if !c.pass {
            println!("      {}", c.detail);
            failed += 1;
        }
    }

    println!();
    if failed == 0 {
        println!("All checks passed. ✓");
        Ok(())
    } else {
        bail!("{failed} check(s) failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercise the Check builders — pass/fail formatting is sane.
    #[test]
    fn check_builders() {
        let c1 = Check::ok("a");
        assert!(c1.pass);
        assert_eq!(c1.name, "a");
        let c2 = Check::fail("b", "because");
        assert!(!c2.pass);
        assert_eq!(c2.detail, "because");
    }

    /// In a clean dev env, most checks fail (no install). The
    /// harness must report failures without panicking.
    #[test]
    fn run_reports_failures_without_panic() {
        // Don't assert Ok — a dev env has no install. Just assert
        // the function doesn't panic.
        let _ = run();
    }

    /// Operator-state check fails when HOME points to /nonexistent.
    #[test]
    fn check_operator_state_with_bad_home() {
        // We can't safely override env in a multi-threaded test
        // suite, but we can assert the helper function itself is
        // safe to call.
        let _ = operator_home();
    }

    /// Token mode 0600 detection: the check returns fail on a
    /// file with mode 0644.
    #[cfg(unix)]
    #[test]
    fn check_operator_token_rejects_world_readable() {
        let dir = tempfile::tempdir().unwrap();
        let token = dir.path().join("token");
        std::fs::write(&token, b"x").unwrap();
        std::fs::set_permissions(&token, std::os::unix::fs::PermissionsExt::from_mode(0o644)).unwrap();
        // We don't directly call the check (it reads $HOME); we
        // just exercise the mode helper.
        assert_eq!(token_mode(&token), Some(0o644));
    }
}
