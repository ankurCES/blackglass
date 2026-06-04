//! The polkit helper binary. See lib.rs for the design and tests.

use anyhow::{bail, Result};
use std::env;

fn main() -> Result<()> {
    let caller_uid = env::var("PKEXEC_UID")
        .or_else(|_| env::var("SUDO_UID"))
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .ok_or_else(|| anyhow::anyhow!("no PKEXEC_UID or SUDO_UID in env"))?;

    // The polkit action passes the command via the .policy file's
    // argv[1]. In tests we invoke the binary with the command
    // directly; in production the polkit policy supplies it.
    let command = env::args()
        .nth(1)
        .unwrap_or_else(|| blackglass_polkit_helper::CORE_BINARY.to_string());

    blackglass_polkit_helper::run(caller_uid, &command)
}
