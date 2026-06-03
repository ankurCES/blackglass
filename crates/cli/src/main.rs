use clap::{Parser, Subcommand};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "blackglass", version, about = "Blackglass CLI (sub-plan 1)")]
struct Cli {
    #[arg(long, global = true, default_value = "~/.local/share/blackglass/runtime.sock")]
    socket: String,
    #[arg(long, global = true, default_value = "~/.local/share/blackglass/operator.token")]
    token_file: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create data directories and write a fresh auth token.
    Init,
    /// Ping the core server (requires a running blackglass-core).
    Ping,
    /// Verify the hash-chain integrity of an audit log.
    AuditVerify {
        #[arg(long, default_value = "~/.local/share/blackglass/audit/audit.jsonl")]
        path: String,
    },
}

fn expand(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(p)
}

fn read_token(p: &PathBuf) -> anyhow::Result<String> {
    Ok(std::fs::read_to_string(p)?.trim().to_string())
}

/// Write a length-prefixed JSON frame and read back the response on `c`.
fn rpc_round_trip(c: &mut UnixStream, req: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
    c.write_all(&blackglass_ipc::encode_frame(&serde_json::to_vec(req)?))?;
    let mut lenb = [0u8; 4];
    c.read_exact(&mut lenb)?;
    let n = u32::from_be_bytes(lenb) as usize;
    let mut buf = vec![0u8; n];
    c.read_exact(&mut buf)?;
    Ok(serde_json::from_slice(&buf)?)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let socket = expand(&cli.socket);
    let token_file = expand(&cli.token_file);

    match cli.cmd {
        Cmd::Init => {
            if let Some(p) = socket.parent() {
                std::fs::create_dir_all(p)?;
            }
            if let Some(p) = token_file.parent() {
                std::fs::create_dir_all(p)?;
            }
            let token_bytes: [u8; 32] = rand::random();
            let token = hex::encode(token_bytes);
            std::fs::write(&token_file, &token)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perm = std::fs::metadata(&token_file)?.permissions();
                perm.set_mode(0o600);
                std::fs::set_permissions(&token_file, perm)?;
            }
            println!("initialized; token written to {}", token_file.display());
        }

        Cmd::Ping => {
            let tok = read_token(&token_file)?;
            let mut c = UnixStream::connect(&socket)?;
            // Auth then Ping on the same connection (auth is per-connection).
            let _auth = rpc_round_trip(
                &mut c,
                &serde_json::json!({"id": 0, "method": "auth", "token": tok}),
            )?;
            let resp = rpc_round_trip(&mut c, &serde_json::json!({"id": 1, "method": "ping"}))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }

        Cmd::AuditVerify { path } => {
            let p = expand(&path);
            match blackglass_audit::Chain::verify(&p) {
                Ok(count) => println!("OK: {count} events verified"),
                Err(e) => {
                    eprintln!("FAIL: {e}");
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}
