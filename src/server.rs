use std::path::Path;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::{ServerHandler, tool_handler};

use crate::config::Config;
use crate::index::SearchIndex;
use crate::output::truncate_with_notice;
use crate::vault::Vault;
use crate::watcher::{self, VaultWatcher};

/// Options from the CLI.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// `--read-only`: write tools are not registered.
    pub read_only: bool,
}

/// The MCP server for one vault. Serve it with `rmcp::ServiceExt::serve`.
#[derive(Debug, Clone)]
pub struct Server {
    pub(crate) vault: Arc<Vault>,
    pub(crate) config: Config,
    /// In-memory search index, shared by every clone and by the write tools.
    pub(crate) index: Arc<SearchIndex>,
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
            tool_router: Self::tool_router(&options),
        })
    }

    /// Sum of the per-module routers. Each tool module adds one line here.
    /// Write tools are added only without `--read-only`.
    fn tool_router(options: &Options) -> ToolRouter<Server> {
        let router = Self::list_router()
            + Self::read_router()
            + Self::search_router()
            + Self::backlinks_router()
            + Self::tags_router();
        if options.read_only {
            router
        } else {
            router
                + Self::write_router()
                + Self::tags_write_router()
                + Self::move_router()
                + Self::move_folder_router()
                + Self::delete_router()
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }
}

/// Tools that cut their own output (they tell the client where to continue).
/// Error messages are always cut.
const SELF_LIMITED: [&str; 1] = ["read_note"];

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {
    /// The one place where the size limit applies: every tool output, errors
    /// included, is cut here.
    async fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParams,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::CallToolResponse, rmcp::ErrorData> {
        let self_limited = SELF_LIMITED.contains(&request.name.as_ref());
        let tcc = rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
        let limit = self.config.max_response_bytes;
        let mut response = self.tool_router.call(tcc).await.map_err(|mut e| {
            e.message = truncate_with_notice(&e.message, limit).into();
            e
        })?;
        if let rmcp::model::CallToolResponse::Complete(result) = &mut response
            && !(self_limited && result.is_error != Some(true))
        {
            for block in &mut result.content {
                if let Some(text) = block.as_text() {
                    let cut = truncate_with_notice(&text.text, limit);
                    if cut.len() != text.text.len() {
                        *block = rmcp::model::ContentBlock::text(cut);
                    }
                }
            }
        }
        Ok(response)
    }
}
