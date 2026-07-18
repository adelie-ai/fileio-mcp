#![deny(warnings)]
#![recursion_limit = "256"]

// Library crate for fileio-mcp.
// Protocol dispatch, transport framing and CLI are provided by mcp-core.

pub mod coerce;
pub mod error;
pub mod operations;
pub mod path_guard;
pub mod service;
pub mod tools;

use mcp_core::ServerConfig;

/// Build the [`ServerConfig`] that describes this server to MCP clients.
///
/// Extracted from `main` so the server-level `instructions` hint is defined in
/// one place and covered by unit tests. Websocket stays available; the
/// unix-socket transport is opt-in via `--transport unix`.
pub fn server_config() -> ServerConfig {
    ServerConfig::new("fileio-mcp", env!("CARGO_PKG_VERSION")).with_unix()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Acceptance: the server advertises a non-empty `instructions` blurb so
    /// the daemon has a server-level description to index for tool discovery.
    #[test]
    fn server_config_exposes_non_empty_instructions() {
        let cfg = server_config();
        let instructions = cfg
            .instructions
            .as_deref()
            .expect("server_config must set instructions");
        assert!(
            !instructions.trim().is_empty(),
            "instructions blurb must not be empty"
        );
    }

    /// Acceptance: the blurb states the purpose, names the key tools, and cues
    /// the natural situations a model would map to this server, so discovery
    /// surfaces it for file read/write/edit/search queries.
    #[test]
    fn server_instructions_name_core_tools_and_purpose() {
        let cfg = server_config();
        let text = cfg
            .instructions
            .expect("server_config must set instructions");
        for needle in [
            "fileio_read_lines",
            "fileio_write_file",
            "fileio_edit_file",
            "fileio_find_in_files",
            "fileio_find_files",
        ] {
            assert!(
                text.contains(needle),
                "instructions should name the {needle} tool, got: {text}"
            );
        }
        let lower = text.to_lowercase();
        for term in ["read", "write", "edit", "search", "file"] {
            assert!(
                lower.contains(term),
                "instructions should mention '{term}', got: {text}"
            );
        }
    }
}
