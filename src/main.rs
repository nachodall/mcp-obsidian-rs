use std::path::PathBuf;
use std::process::ExitCode;

use mcp_obsidian::server::{Options, Server};
use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> anyhow::Result<()> {
    // Usage: mcp-obsidian <vault-path> [--read-only]
    let mut vault_path = None;
    let mut options = Options::default();
    for arg in std::env::args().skip(1) {
        if arg == "--read-only" {
            options.read_only = true;
        } else if vault_path.is_none() {
            vault_path = Some(PathBuf::from(arg));
        } else {
            anyhow::bail!("unexpected argument: {arg}");
        }
    }
    let Some(vault_path) = vault_path else {
        anyhow::bail!("usage: mcp-obsidian <vault-path> [--read-only]");
    };
    let server = Server::new(&vault_path, options)?;
    server.serve(stdio()).await?.waiting().await?;
    Ok(())
}
