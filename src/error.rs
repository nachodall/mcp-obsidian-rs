//! Error type shared by the tool handlers. Tools return `ToolError` and the
//! MCP boundary turns it into the error text the client sees.

use crate::vault::VaultError;

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error("already exists: {0}")]
    AlreadyExists(String),
    /// A filesystem failure, with the action that failed (`write`, `move`, ...).
    #[error("{action} failed: {source}")]
    Io { action: &'static str, source: std::io::Error },
    #[error("{0}")]
    Message(String),
}

impl ToolError {
    /// Adapter for `map_err`: `.map_err(ToolError::io("write"))?`.
    pub fn io(action: &'static str) -> impl FnOnce(std::io::Error) -> ToolError {
        move |source| ToolError::Io { action, source }
    }
}

/// A bare `?` on an I/O error keeps the generic action `io`.
impl From<std::io::Error> for ToolError {
    fn from(source: std::io::Error) -> Self {
        ToolError::Io { action: "io", source }
    }
}

impl From<&str> for ToolError {
    fn from(message: &str) -> Self {
        ToolError::Message(message.to_string())
    }
}

impl From<String> for ToolError {
    fn from(message: String) -> Self {
        ToolError::Message(message)
    }
}

/// The MCP boundary: tool handlers return plain error strings.
impl From<ToolError> for String {
    fn from(e: ToolError) -> Self {
        e.to_string()
    }
}
