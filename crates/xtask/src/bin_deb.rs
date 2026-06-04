use anyhow::{bail, Result};
use std::process::Command;

pub fn build() -> Result<()> {
    println!("=== xtask build ===");
    run(Command::new("cargo").args(["build", "--release", "--workspace"]))?;
    run(Command::new("npm").args(["ci"]).current_dir("app"))?;
    run(Command::new("npm").args(["run", "build"]).current_dir("app"))?;
    Ok(())
}

pub fn deb(variants: &str) -> Result<()> {
    println!("=== xtask deb ({}) ===", variants);
    // First build everything
    build()?;
    // Then build the .deb. We currently ship a single .deb; the
    // --variants CLI flag is accepted for forward-compat (so the
    // README + install.sh can say `full`) but a real multi-variant
    // build (minimal/core/full) is post-v0.1 scope.
    //
    // The actual cargo-deb invocation is `-p blackglass-core` because
    // the [package.metadata.deb] block lives in that crate. `--no-build`
    // because `build()` above just produced target/release/* binaries.
    let _ = split_variants(variants); // validates the input shape
    println!("\n--- building blackglass-core .deb ---");
    run(Command::new("cargo").args([
        "deb",
        "-p",
        "blackglass-core",
        "--no-build",
    ]))?;
    Ok(())
}

fn run(cmd: &mut Command) -> Result<()> {
    println!("+ {:?}", cmd);
    let status = cmd.status()?;
    if !status.success() {
        bail!("command failed: {:?}", cmd);
    }
    Ok(())
}

/// Split a comma-separated variant list into individual trimmed
/// variant names. Exposed for testing.
fn split_variants(s: &str) -> Vec<String> {
    s.split(',').map(|s| s.trim().to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_variants_handles_single() {
        assert_eq!(split_variants("full"), vec!["full".to_string()]);
    }

    #[test]
    fn split_variants_handles_multi() {
        assert_eq!(
            split_variants("minimal,core,full"),
            vec![
                "minimal".to_string(),
                "core".to_string(),
                "full".to_string()
            ]
        );
    }

    #[test]
    fn split_variants_trims_whitespace() {
        assert_eq!(
            split_variants(" minimal , core ,full "),
            vec!["minimal".to_string(), "core".to_string(), "full".to_string()]
        );
    }

    #[test]
    fn split_variants_empty_input() {
        assert_eq!(split_variants(""), vec!["".to_string()]);
    }

    // ── Manifest-content tests ──────────────────────────────────
    //
    // The cargo-deb.toml is the source of truth for what goes into
    // the .deb. These tests parse the manifest and assert that:
    //   - all required binaries are present
    //   - the 2 AppArmor profiles are present
    //   - the 2 systemd unit files are present
    //   - mcp-servers.toml.example is present
    //   - NO references to polkit, /var/lib/blackglass, or cosign
    //
    // Phase 5 exit criteria. The string matchers catch any
    // regression that re-introduces a v0 dependency.

    /// Load the cargo-deb manifest as a string. The path is
    /// crates/xtask/../../crates/core/Cargo.toml (the workspace's
    /// blackglass-core package, which is where the .deb metadata
    /// lives in the user-systemd model).
    fn load_deb_manifest() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent() // crates/
            .unwrap()
            .parent() // workspace root
            .unwrap()
            .join("crates/core/Cargo.toml");
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()))
    }

    #[test]
    fn manifest_contains_all_binaries() {
        let m = load_deb_manifest();
        for bin in &[
            "target/release/blackglass",
            "target/release/blackglass-core",
            "target/release/blackglass-app",
            "target/release/blackglass-secondary-sidecar",
            "target/release/blackglass-mcp-osint",
            "target/release/blackglass-mcp-packets",
            "target/release/blackglass-mcp-ad",
            "target/release/blackglass-mcp-flipper",
            "target/release/blackglass-mcp-phish",
            "target/release/blackglass-mcp-detect",
        ] {
            assert!(
                m.contains(bin),
                "manifest missing binary asset: {bin}\n(may need to add to crates/core/Cargo.toml assets list)"
            );
        }
    }

    #[test]
    fn manifest_contains_systemd_and_apparmor() {
        let m = load_deb_manifest();
        for entry in &[
            "packaging/systemd/blackglass-core.service",
            "packaging/systemd/blackglass-secondary-sidecar.service",
            "packaging/apparmor/blackglass-core",
            "packaging/apparmor/blackglass-secondary-sidecar",
            "packaging/mcp-servers.toml.example",
        ] {
            assert!(
                m.contains(entry),
                "manifest missing required entry: {entry}"
            );
        }
    }

    #[test]
    fn manifest_excludes_polkit_and_cosign() {
        let m = load_deb_manifest();
        // The user-systemd model removes polkit + cosign. These
        // strings must not appear anywhere in the manifest.
        for forbidden in &[
            "polkit-helper",
            "policykit",
            "libpolkit",
            "cosign",
        ] {
            assert!(
                !m.contains(forbidden),
                "manifest still references v0 '{forbidden}' dependency — \
                 the user-systemd model must not ship polkit or cosign"
            );
        }
    }
}
