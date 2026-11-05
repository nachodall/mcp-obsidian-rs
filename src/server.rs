use std::path::Path;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::{ServerHandler, tool_handler};

use crate::config::Config;
use crate::index::SearchIndex;
use crate::vault::Vault;
use crate::watcher::{self, VaultWatcher};

/// Options from the CLI.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// `--read-only`: write tools are not registered (behavior in ticket 06).
    pub read_only: bool,
}

/// The MCP server for one vault. Serve it with `rmcp::ServiceExt::serve`.
#[derive(Debug, Clone)]
pub struct Server {
    pub(crate) vault: Arc<Vault>,
    pub(crate) config: Config,
    /// In-memory search index, shared by every clone and by the write tools.
    pub(crate) index: Arc<SearchIndex>,
    options: Options,
    /// Keeps the file watcher alive for the lifetime of the server.
    _watcher: Option<Arc<VaultWatcher>>,
    tool_router: ToolRouter<Server>,
}

impl Server {
    /// Loads the config and builds the server. Fails when the config is invalid.
    pub fn new(vault_root: &Path, options: Options) -> anyhow::Result<Server> {
        let config = Config::load(vault_root)?;
        let vault = Arc::new(Vault::new(vault_root, config.clone())?);
        let index = Arc::new(SearchIndex::build(vault.clone())?);
        let watcher = match watcher::start(vault.clone(), index.clone()) {
            Ok(w) => Some(Arc::new(w)),
            Err(e) => {
                eprintln!("watcher: could not start, the index will not follow disk edits: {e:#}");
                None
            }
        };
        Ok(Server {
            vault,
            index,
            _watcher: watcher,
            config,
            options,
            tool_router: Self::tool_router(),
        })
    }

    /// Sum of the per-module routers. Each tool module adds one line here.
    fn tool_router() -> ToolRouter<Server> {
        Self::list_router() + Self::search_router()
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn options(&self) -> &Options {
        &self.options
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {}
