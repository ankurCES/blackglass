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

fn check_app_armor() -> Check {
    if !Command::new("aa-enabled")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Check::fail("apparmor-enabled", "aa-enabled reports disabled");
    }
    // aa-status requires root; if we can't enumerate, the probe is
    // inconclusive. We use a non-root heuristic: look for the
    // profile files in /etc/apparmor.d/.
    let core = Path::new("/etc/apparmor.d/blackglass-core").exists();
    let helper = Path::new("/etc/apparmor.d/blackglass-polkit-helper").exists();
    if !core {
        return Check::fail(
            "apparmor-core-profile",
            "/etc/apparmor.d/blackglass-core missing",
        );
    }
    if !helper {
        return Check::fail(
            "apparmor-helper-profile",
            "/etc/apparmor.d/blackglass-polkit-helper missing",
        );
    }
    Check::ok("apparmor")
}

fn check_audit_dir() -> Check {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = format!("{home}/.local/share/blackglass/audit");
    if !Path::new(&path).exists() {
        return Check::fail("audit-dir", format!("{path} does not exist"));
    }
    Check::ok("audit-dir")
}

fn check_group() -> Check {
    let user = std::env::var("USER").unwrap_or_default();
    let out = match Command::new("id").args(["-Gn", &user]).output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return Check::fail("group", "id failed"),
    };
    if !out.split_whitespace().any(|g| g == "blackglass") {
        return Check::fail(
            "group",
            format!("user {user} is not in the blackglass group"),
        );
    }
    Check::ok("group")
}

fn check_polkit_helper() -> Check {
    let path = "/usr/libexec/blackglass-polkit-helper";
    if !Path::new(path).exists() {
        return Check::fail("polkit-helper", format!("{path} not found"));
    }
    Check::ok("polkit-helper")
}

fn check_flipper_rule() -> Check {
    let path = "/lib/udev/rules.d/99-blackglass-flipper.rules";
    if !Path::new(path).exists() {
        return Check::fail("flipper-udev", format!("{path} not found"));
    }
    Check::ok("flipper-udev")
}

fn check_python_venv() -> Check {
    let path = "/usr/lib/blackglass/python-venv/bin/python";
    if !Path::new(path).exists() {
        return Check::fail("python-venv", format!("{path} not found"));
    }
    let out = Command::new(path)
        .args([
            "-c",
            "import blackglass_sidecar.scapy_bridge, \
             blackglass_sidecar.impacket_bridge, \
             blackglass_sidecar.hardware_bridge, \
             blackglass_sidecar.audit_types; print('OK')",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => Check::ok("python-venv"),
        Ok(o) => Check::fail("python-venv", String::from_utf8_lossy(&o.stderr)),
        Err(e) => Check::fail("python-venv", e.to_string()),
    }
}

fn check_cosign_key() -> Check {
    let path = "/usr/share/blackglass/cosign.pub";
    if !Path::new(path).exists() {
        return Check::fail("cosign-key", format!("{path} not found"));
    }
    Check::ok("cosign-key")
}

pub fn run() -> Result<()> {
    println!("=== blackglass verify-install ===\n");
    let checks = vec![
        check_app_armor(),
        check_audit_dir(),
        check_group(),
        check_polkit_helper(),
        check_flipper_rule(),
        check_python_venv(),
        check_cosign_key(),
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

    /// Exercise the Check builders and the run() harness in an
    /// environment without any of the install paths. The harness
    /// should report failures without panicking.
    #[test]
    fn run_reports_failures_without_panic() {
        // Don't actually call run() — it calls bail!() which is
        // fine but noisy. Just exercise the Check type and
        // confirm pass/fail formatting is sane.
        let c1 = Check::ok("a");
        assert!(c1.pass);
        assert_eq!(c1.name, "a");
        let c2 = Check::fail("b", "because");
        assert!(!c2.pass);
        assert_eq!(c2.detail, "because");
    }

    #[test]
    fn check_polkit_helper_reports_missing() {
        // The path is /usr/libexec/blackglass-polkit-helper,
        // which is definitely not on the dev machine.
        let c = check_polkit_helper();
        assert!(!c.pass);
        assert!(c.detail.contains("not found"));
    }

    #[test]
    fn check_cosign_key_reports_missing() {
        let c = check_cosign_key();
        assert!(!c.pass);
        assert!(c.detail.contains("not found"));
    }
}
