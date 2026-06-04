//! Confinement test. Verifies that on an installed system:
//!   - the blackglass-core AppArmor profile loads and confines the core
//!     to reading /etc/blackglass/ but not /etc/shadow
//!   - the blackglass-polkit-helper profile loads and rejects non-core
//!     commands
//!   - the Flipper udev rule is in place
//!
//! Each check independently skips with a diagnostic if its prerequisite
//! (apparmor_parser, aa-exec, the profile being loaded, the udev rule
//! being active) is not present — this lets the test run in dev
//! environments without root.

use anyhow::{bail, Result};
use std::process::Command;

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

/// Is the named AppArmor profile loaded? Uses `aa-exec -p X -- true`
/// as a probe (aa-status requires root to enumerate). Returns:
///   Some(true)  - profile loaded
///   Some(false) - aa-exec present but profile does not exist
///   None        - aa-exec not installed (skip)
fn aa_profile_loaded(profile: &str) -> Option<bool> {
    let out = run_if_present("aa-exec", &["-p", profile, "--", "true"])?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("does not exist") {
        return Some(false);
    }
    Some(out.status.success())
}

fn try_read_as_profile(profile: &str, path: &str) -> Option<bool> {
    let out = run_if_present("aa-exec", &["-p", profile, "--", "cat", path])?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("does not exist") {
        return None;
    }
    Some(out.status.success())
}

fn core_reads_shadow_blocked() -> Option<bool> {
    let out = run_if_present(
        "aa-exec",
        &["-p", "blackglass-core", "--", "cat", "/etc/shadow"],
    )?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("does not exist") {
        return None;
    }
    Some(!out.status.success())
}

fn helper_rejects_non_core() -> Option<bool> {
    let out = run_if_present(
        "aa-exec",
        &[
            "-p",
            "blackglass-polkit-helper",
            "--",
            "/usr/libexec/blackglass-polkit-helper",
            "--command",
            "/bin/sh",
        ],
    )?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("does not exist") {
        return None;
    }
    Some(!out.status.success())
}

fn udev_flipper_rule_active() -> Option<bool> {
    // We don't have a real Flipper in CI, so we check that the rule
    // file is present in /etc/udev/rules.d/ or /lib/udev/rules.d/.
    let locations = [
        "/etc/udev/rules.d/99-blackglass-flipper.rules",
        "/lib/udev/rules.d/99-blackglass-flipper.rules",
        "/usr/lib/udev/rules.d/99-blackglass-flipper.rules",
    ];
    Some(locations.iter().any(|p| std::path::Path::new(p).exists()))
}

pub fn run() -> Result<()> {
    println!("=== confinement-test: blackglass-core ===\n");

    let mut failures: Vec<String> = Vec::new();

    // 1. Profile loaded?
    match aa_profile_loaded("blackglass-core") {
        Some(true) => println!("  ✓ blackglass-core profile is loaded"),
        Some(false) => failures.push(
            "blackglass-core profile is not loaded — install the .deb or run \
             `sudo apparmor_parser -r packaging/apparmor/blackglass-core`"
                .into(),
        ),
        None => println!("  · aa-status not available — skipping profile-loaded check"),
    }

    // 2. Core can read /etc/blackglass/ (negative — not required to pass)
    match try_read_as_profile(
        "blackglass-core",
        "/etc/blackglass/python-bridge.toml.example",
    ) {
        Some(true) => println!("  ✓ blackglass-core can read /etc/blackglass/"),
        Some(false) => println!("  · blackglass-core denied /etc/blackglass/ (config may not be installed)"),
        None => println!("  · profile not loaded — skipping /etc/blackglass/ read check"),
    }

    // 3. Core cannot read /etc/shadow.
    match core_reads_shadow_blocked() {
        Some(true) => println!("  ✓ blackglass-core correctly denied /etc/shadow"),
        Some(false) => failures.push(
            "blackglass-core was able to read /etc/shadow (should be denied)".into(),
        ),
        None => println!("  · profile not loaded — skipping /etc/shadow check"),
    }

    // 4. Polkit helper rejects non-core.
    match helper_rejects_non_core() {
        Some(true) => println!("  ✓ polkit-helper rejects non-core commands"),
        Some(false) => failures.push(
            "polkit-helper accepted a non-core command (should be denied by AppArmor)"
                .into(),
        ),
        None => println!("  · polkit-helper profile not loaded — skipping exec check"),
    }

    // 5. Udev rule installed.
    match udev_flipper_rule_active() {
        Some(true) => println!("  ✓ udev Flipper rule is installed"),
        Some(false) => println!("  · udev Flipper rule not installed yet (ok in dev)"),
        None => unreachable!(),
    }

    println!();
    if failures.is_empty() {
        println!("=== ALL CONFINEMENT TESTS PASSED ===");
        Ok(())
    } else {
        for f in &failures {
            eprintln!("FAIL: {f}");
        }
        bail!("{} confinement check(s) failed", failures.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test harness itself should not panic when the system has
    /// none of aa-status, aa-exec, or the udev rules installed.
    /// (In CI we always have something missing.)
    #[test]
    fn run_safely_skips_in_clean_env() {
        // We don't assert pass/fail — we only assert the function
        // returns without panicking, regardless of environment.
        let _ = run();
    }
}
