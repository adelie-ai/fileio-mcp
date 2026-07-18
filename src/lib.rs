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

/// Model-facing server description returned in the MCP `initialize` response.
///
/// Why: the daemon indexes this as the server's searchable description for tool
/// discovery, so it must state what the server is for, when to reach for it, and
/// name the key tools. It deliberately says nothing about the sensitive-path
/// deny-list (see [`path_guard`]), which is designed to be invisible to callers.
const SERVER_INSTRUCTIONS: &str = "Local filesystem access for this machine: \
read, write, and make structured edits to text files; search inside files \
(grep-style) and locate files by name; and manage directories, permissions, \
ownership, links, and temporary files. Reach for it whenever the user wants to \
view, change, search, or organize files and folders on disk - for example \
'show me this file', 'edit that config', 'find where this string appears', or \
'list what is in this directory'. Key tools: fileio_read_lines and \
fileio_write_file to read and write, fileio_edit_file for targeted anchor- or \
line-based edits, fileio_find_in_files to search text across files and \
fileio_find_files to locate files by name or glob, plus fileio_list_directory \
and fileio_stat to explore and inspect entries. Prefer absolute paths, since \
relative paths resolve from the server's working directory, and note that \
write, move, and remove operations act on the real filesystem and take effect \
immediately.";

/// Build the [`ServerConfig`] that describes this server to MCP clients.
///
/// Extracted from `main` so the server-level `instructions` hint is defined in
/// one place and covered by unit tests. Websocket stays available; the
/// unix-socket transport is opt-in via `--transport unix`.
pub fn server_config() -> ServerConfig {
    ServerConfig::new("fileio-mcp", env!("CARGO_PKG_VERSION"))
        .instructions(SERVER_INSTRUCTIONS)
        .with_unix()
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
