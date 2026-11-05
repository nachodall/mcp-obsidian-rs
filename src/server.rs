use std::path::Path;

use rmcp::ServerHandler;

use crate::config::Config;

/// Options from the CLI.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// `--read-only`: write tools are not registered (behavior in ticket 06).
    pub read_only: bool,
}

/// The MCP server for one vault. Serve it with `rmcp::ServiceExt::serve`.
#[derive(Debug, Clone)]
pub struct Server {
    _private: (),
}

impl Server {
    /// Loads the config and builds the server. Fails when the config is invalid.
    pub fn new(_vault_root: &Path, _options: Options) -> anyhow::Result<Server> {
        todo!()
    }

    pub fn config(&self) -> &Config {
        todo!()
    }

    pub fn options(&self) -> &Options {
        todo!()
    }
}

impl ServerHandler for Server {}
