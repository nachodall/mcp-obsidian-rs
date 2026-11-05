#![allow(dead_code)]

pub mod fs;

use std::path::{Path, PathBuf};

use mcp_obsidian::server::{Options, Server};
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde_json::Value;
use tempfile::TempDir;

/// Temp vault on disk plus a sibling directory outside the vault.
pub struct TestVault {
    _tmp: TempDir,
    pub root: PathBuf,
    pub outside: PathBuf,
}

impl TestVault {
    pub fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("vault");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        TestVault { _tmp: tmp, root, outside }
    }

    pub fn file(&self, rel: &str, content: &str) -> &Self {
        let p = self.root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
        self
    }

    pub fn note(&self, rel: &str, content: &str) -> &Self {
        self.file(rel, content)
    }

    pub fn attachment(&self, rel: &str) -> &Self {
        self.file(rel, "binary")
    }

    pub fn config(&self, toml: &str) -> &Self {
        self.file(".mcp-obsidian.toml", toml)
    }

    /// Standard fixture: notes, attachment, excluded `secret/`, `.obsidian/`,
    /// `.trash/`, and a symlink `link` pointing outside the vault.
    pub fn standard() -> Self {
        let v = Self::new();
        v.note("root.md", "# root")
            .note("projects/alpha.md", "# alpha")
            .note("projects/deep/beta.md", "# beta")
            .attachment("projects/diagram.png")
            .note("secret/hidden-note.md", "# secret")
            .attachment("secret/leak.pdf")
            .file(".obsidian/app.json", "{\"token\":\"x\"}")
            .note(".trash/deleted.md", "# deleted")
            .config("excluded_folders = [\"secret\"]\n");
        std::fs::write(v.outside.join("outside-file.md"), "# outside").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&v.outside, v.root.join("link")).unwrap();
        v
    }
}

pub struct Client {
    pub service: RunningService<RoleClient, ()>,
}

/// Starts the real server in process over a tokio duplex channel.
/// Returns Err with the startup error text when the server refuses to start.
pub async fn try_start(root: &Path, options: Options) -> Result<(Client, Server), String> {
    let server = Server::new(root, options).map_err(|e| format!("{e:#}"))?;
    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let s = server.clone();
    tokio::spawn(async move {
        if let Ok(running) = s.serve(server_io).await {
            let _ = running.waiting().await;
        }
    });
    let service = ().serve(client_io).await.map_err(|e| e.to_string())?;
    Ok((Client { service }, server))
}

pub async fn start(root: &Path) -> Client {
    try_start(root, Options::default()).await.unwrap_or_else(|e| panic!("server failed to start: {e}")).0
}

impl Client {
    /// Calls a tool by name. Err is the protocol-level or tool-level error text.
    pub async fn call(&self, name: &str, args: Value) -> Result<String, String> {
        let mut params = CallToolRequestParams::new(name.to_string());
        if let Value::Object(map) = args {
            params = params.with_arguments(map);
        }
        match self.service.call_tool(params).await {
            Ok(res) => {
                let text = result_text(&res);
                if res.is_error == Some(true) { Err(text) } else { Ok(text) }
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub async fn ok(&self, name: &str, args: Value) -> String {
        self.call(name, args).await.unwrap_or_else(|e| panic!("{name} failed: {e}"))
    }

    pub async fn err(&self, name: &str, args: Value) -> String {
        match self.call(name, args).await {
            Ok(t) => panic!("{name} should have failed, got: {t}"),
            Err(e) => e,
        }
    }
}

pub fn result_text(res: &CallToolResult) -> String {
    res.content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}
