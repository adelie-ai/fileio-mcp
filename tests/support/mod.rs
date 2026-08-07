#![allow(dead_code)]

//! Shared table of every MCP tool this server advertises, each paired with a
//! call whose content-bearing arguments carry a sentinel unique to that
//! tool.
//!
//! `telemetry_suite.rs` (console text from a real process) and
//! `span_field_capture.rs` (raw span/event fields from an in-process
//! capture) both drive the dispatch from this same table, so a tool added
//! to `tools.rs` without a matching entry here fails
//! [`assert_table_covers_every_tool`] in both binaries rather than leaving
//! either test silently short of the tools this server actually exposes.
//!
//! Every path built here stays under the `root` the caller supplies (in
//! practice a `TempDir`) and is deliberately never denied: a guard rejection
//! returns before the tool's own operation function ever runs, so a table
//! meant to prove those functions do not leak has to reach them.

use std::collections::BTreeSet;
use std::path::Path;

use mcp_core::McpService;
use serde_json::{Value, json};

/// One tool call: the name as registered, the arguments to send, and
/// whether those arguments actually carry the tool's sentinel.
///
/// `carries_sentinel` is false for exactly one tool,
/// `fileio_get_current_directory`, which takes no arguments at all - there
/// is nothing for it to leak and nothing for a positive control to find.
pub struct ToolCall {
    pub name: &'static str,
    pub arguments: Value,
    pub carries_sentinel: bool,
}

impl ToolCall {
    fn new(name: &'static str, arguments: Value) -> Self {
        Self {
            name,
            arguments,
            carries_sentinel: true,
        }
    }

    fn without_sentinel(name: &'static str, arguments: Value) -> Self {
        Self {
            name,
            arguments,
            carries_sentinel: false,
        }
    }
}

/// The value content-bearing arguments carry for `tool`, unique per tool so
/// a leak's own text names which one failed.
pub fn sentinel_for(tool: &str) -> String {
    format!("SENTINEL-{tool}-CONTENT")
}

fn path_for(root: &Path, tool: &str) -> String {
    root.join(format!("{}.txt", sentinel_for(tool)))
        .to_string_lossy()
        .into_owned()
}

fn dir_for(root: &Path, tool: &str, suffix: &str) -> String {
    let mut s = root
        .join(format!("{}-{suffix}", sentinel_for(tool)))
        .to_string_lossy()
        .into_owned();
    s.push('/');
    s
}

/// Every registered tool, called once with sentinel-bearing, always-allowed
/// arguments under `root`. None of these paths need to exist: the point is
/// that entering the tool's own operation function must not leak, which
/// happens whether or not the underlying filesystem call then succeeds.
pub fn sentinel_tool_calls(root: &Path) -> Vec<ToolCall> {
    let p = |tool: &str| path_for(root, tool);
    let d = |tool: &str, suffix: &str| dir_for(root, tool, suffix);

    vec![
        ToolCall::new("fileio_read_lines", json!({"path": p("fileio_read_lines")})),
        ToolCall::new(
            "fileio_write_file",
            json!({
                "path": p("fileio_write_file"),
                "content": sentinel_for("fileio_write_file"),
                "append": false,
            }),
        ),
        ToolCall::new(
            "fileio_set_permissions",
            json!({"path": [p("fileio_set_permissions")], "mode": "644"}),
        ),
        ToolCall::new(
            "fileio_set_mode",
            json!({"path": [p("fileio_set_mode")], "mode": "644"}),
        ),
        ToolCall::new(
            "fileio_get_permissions",
            json!({"path": [p("fileio_get_permissions")]}),
        ),
        ToolCall::new("fileio_touch", json!({"path": [p("fileio_touch")]})),
        ToolCall::new("fileio_stat", json!({"path": [p("fileio_stat")]})),
        ToolCall::new(
            "fileio_make_directory",
            json!({"path": [d("fileio_make_directory", "dir")], "recursive": true}),
        ),
        ToolCall::new(
            "fileio_list_directory",
            json!({"path": d("fileio_list_directory", "dir")}),
        ),
        ToolCall::new(
            "fileio_find_files",
            json!({
                "pattern": format!("{}-*", sentinel_for("fileio_find_files")),
                "root": d("fileio_find_files", "root"),
            }),
        ),
        ToolCall::new(
            "fileio_find_in_files",
            json!({
                "pattern": sentinel_for("fileio_find_in_files"),
                "path": d("fileio_find_in_files", "root"),
            }),
        ),
        ToolCall::new(
            "fileio_edit_file",
            json!({
                "path": p("fileio_edit_file"),
                "edits": [{
                    "op": "insert_at_line",
                    "line": 1,
                    "text": sentinel_for("fileio_edit_file"),
                }],
            }),
        ),
        ToolCall::new(
            "fileio_copy",
            json!({
                "source": [p("fileio_copy")],
                "destination": d("fileio_copy", "dest"),
            }),
        ),
        ToolCall::new(
            "fileio_move",
            json!({
                "source": [p("fileio_move")],
                "destination": d("fileio_move", "dest"),
            }),
        ),
        ToolCall::new("fileio_remove", json!({"path": [p("fileio_remove")]})),
        ToolCall::new(
            "fileio_remove_directory",
            json!({"path": [d("fileio_remove_directory", "dir")]}),
        ),
        ToolCall::new(
            "fileio_create_hard_link",
            json!({
                "target": p("fileio_create_hard_link"),
                "link_path": path_for(root, "fileio_create_hard_link-link"),
            }),
        ),
        ToolCall::new(
            "fileio_create_symbolic_link",
            json!({
                "target": p("fileio_create_symbolic_link"),
                "link_path": path_for(root, "fileio_create_symbolic_link-link"),
            }),
        ),
        ToolCall::new(
            "fileio_get_basename",
            json!({"path": p("fileio_get_basename")}),
        ),
        ToolCall::new(
            "fileio_get_dirname",
            json!({"path": p("fileio_get_dirname")}),
        ),
        ToolCall::new(
            "fileio_get_canonical_path",
            json!({"path": p("fileio_get_canonical_path")}),
        ),
        ToolCall::new(
            "fileio_read_symbolic_link",
            json!({"path": p("fileio_read_symbolic_link")}),
        ),
        ToolCall::new(
            "fileio_create_temporary",
            json!({"type": "file", "template": d("fileio_create_temporary", "dir")}),
        ),
        ToolCall::new(
            "fileio_change_ownership",
            json!({"path": [p("fileio_change_ownership")], "user": "0"}),
        ),
        ToolCall::without_sentinel("fileio_get_current_directory", json!({})),
        ToolCall::new(
            "fileio_count_lines",
            json!({"path": [p("fileio_count_lines")]}),
        ),
        ToolCall::new(
            "fileio_count_words",
            json!({"path": [p("fileio_count_words")]}),
        ),
    ]
}

/// Panics naming exactly what drifted if `calls` and the tools this server
/// actually registers (via [`fileio_mcp::build_service`]) are not the same
/// set - in either direction, so a stale table entry is caught as surely as
/// a missing one.
pub fn assert_table_covers_every_tool(calls: &[ToolCall]) {
    let registered: BTreeSet<String> = fileio_mcp::build_service()
        .tools()
        .into_iter()
        .map(|t| t.name)
        .collect();
    let tabled: BTreeSet<String> = calls.iter().map(|c| c.name.to_string()).collect();

    let missing: Vec<_> = registered.difference(&tabled).collect();
    let stale: Vec<_> = tabled.difference(&registered).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "the sentinel-call table in tests/support/mod.rs has drifted from the \
         registered tool list.\n\
         registered but missing from the table: {missing:?}\n\
         in the table but no longer a registered tool: {stale:?}"
    );
}
