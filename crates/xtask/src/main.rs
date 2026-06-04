//! Build orchestrator. Subcommands: build, deb, sign, confinement-test,
//! verify-install, apparmor-generate.

use clap::{Parser, Subcommand};

mod bin_deb;
mod bin_sign;
mod bin_confinement_test;
mod bin_verify_install;
mod bin_apparmor_generate;

#[derive(Parser)]
#[command(name = "xtask", about = "Blackglass build orchestrator")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build all the Rust binaries and the Tauri frontend.
    Build,
    /// Build the .deb packages.
    Deb {
        /// Comma-separated list of variants: minimal,core,full.
        #[arg(long, default_value = "full")]
        variants: String,
    },
    /// Sign a .deb with cosign keyless signing.
    Sign {
        #[arg(long)]
        input: String,
    },
    /// Run the confinement test (requires root + AppArmor).
    ConfinementTest,
    /// Verify an installed system meets the security prerequisites.
    VerifyInstall,
    /// Generate a draft AppArmor profile from a tool list.
    ApparmorGenerate,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Build => bin_deb::build()?,
        Cmd::Deb { variants } => bin_deb::deb(&variants)?,
        Cmd::Sign { input } => bin_sign::sign(&input)?,
        Cmd::ConfinementTest => bin_confinement_test::run()?,
        Cmd::VerifyInstall => bin_verify_install::run()?,
        Cmd::ApparmorGenerate => bin_apparmor_generate::run()?,
    }
    Ok(())
}
