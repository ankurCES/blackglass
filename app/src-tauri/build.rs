// build.rs for the blackglass-app Tauri shell.
//
// Two responsibilities:
//
//   1. Run tauri_build::build() — generates the Tauri scaffolding
//      (gen/schemas, capabilities, etc.) that tauri::generate_context!
//      needs at compile time.
//
//   2. Ensure app/dist/ exists before the proc-macro runs.
//      tauri::generate_context!() bakes the frontend assets into
//      the binary at compile time, so a missing dist/ is a fatal
//      error. dist/ is gitignored (it's a build artifact, regenerated
//      on every `pnpm build`), so a fresh clone + `cargo build` will
//      fail without this step. We invoke `pnpm build` (or fall back
//      to `npm run build`) when dist/ is missing or empty.
//
// We use cargo:rerun-if-changed to make sure this script only re-runs
// when the inputs change — otherwise Cargo would treat the build as
// stale on every invocation and trigger an infinite pnpm-build loop.

use std::path::Path;
use std::process::Command;

fn main() {
    tauri_build::build();

    // The frontend lives at ../ (i.e. app/). tauri-build's CWD for
    // emit_* paths is the package dir (app/src-tauri/).
    let app_dir = Path::new("../");
    let dist = app_dir.join("dist");

    // Tell Cargo when to re-run this script.
    // We re-run if package.json changes (deps changed), if any
    // frontend source changes, or if dist/index.html changes
    // (including being deleted — Cargo tracks specific files
    // for this directive, not just directories).
    //
    // Note: this does NOT cause an infinite loop. The build script
    // writes to dist/ and then exits. The next invocation only
    // happens if dist/ contents change again (which we don't do
    // in this script — we only check that index.html exists).
    println!("cargo:rerun-if-changed=../package.json");
    println!("cargo:rerun-if-changed=../pnpm-lock.yaml");
    println!("cargo:rerun-if-changed=../src");
    println!("cargo:rerun-if-changed=../dist");
    println!("cargo:rerun-if-changed=../dist/index.html");

    // If dist/ exists and contains index.html, we're good.
    if dist.join("index.html").exists() {
        return;
    }

    // dist/ is missing — build the frontend.
    eprintln!("blackglass-app: app/dist/ missing, building frontend with pnpm...");

    // Prefer pnpm (faster, more deterministic) if available, else
    // fall back to npm. We resolve this at build-script time so
    // the user doesn't need a specific toolchain.
    let (cmd, args): (&str, &[&str]) = if Command::new("pnpm")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        ("pnpm", &["build"])
    } else {
        ("npm", &["run", "build"])
    };

    let status = Command::new(cmd)
        .args(args)
        .current_dir(app_dir)
        .status()
        .unwrap_or_else(|e| {
            panic!(
                "blackglass-app: failed to spawn {cmd} to build the frontend: {e}. \
                 Install pnpm or npm and run `cd app && pnpm build` (or `npm run build`) \
                 before `cargo build -p blackglass-app`."
            )
        });

    if !status.success() {
        panic!(
            "blackglass-app: `{cmd} build` failed (exit {code}). \
             Run `cd app && {cmd} build` manually to see the error.",
            code = status.code().unwrap_or(-1),
        );
    }

    if !dist.join("index.html").exists() {
        panic!(
            "blackglass-app: `{cmd} build` did not produce app/dist/index.html. \
             Check the build output for errors."
        );
    }

    eprintln!("blackglass-app: frontend built, continuing Rust compilation.");
}
