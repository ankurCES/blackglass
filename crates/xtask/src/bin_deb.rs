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
    // Then build each variant
    for variant in split_variants(variants) {
        println!("\n--- building variant: {variant} ---");
        // For now, all variants use the same source .deb; the
        // variant-specific apt-deps are pulled in at install time.
        run(Command::new("cargo").args(["deb", "--variant", &variant]))?;
    }
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
}
