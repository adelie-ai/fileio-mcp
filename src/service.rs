#![deny(warnings)]

//! [`McpService`] implementation for fileio-mcp.
//!
//! This module bridges the existing `ToolRegistry` (which knows the tool
//! schemas and how to execute them) with mcp-core's `McpService` trait.
//! The protocol, transport framing, and CLI are all handled by mcp-core.

use mcp_core::{CallError, McpService, ToolDef, ToolReply, async_trait};
use serde_json::Value;

use crate::error::FileIoMcpError;
use crate::path_guard::PathGuard;
use crate::tools::ToolRegistry;

/// The fileio-mcp service.  Owns a `ToolRegistry` (which holds the
/// `PathGuard`) and implements `McpService` for mcp-core.
pub struct FileIoService {
    registry: ToolRegistry,
}

impl FileIoService {
    /// Create with the default path guard (hardcoded deny-list).
    pub fn new() -> Self {
        Self {
            registry: ToolRegistry::new(),
        }
    }

    /// Create with a custom path guard (CLI block-paths / block-file).
    pub fn with_guard(guard: PathGuard) -> Self {
        Self {
            registry: ToolRegistry::with_guard(guard),
        }
    }
}

impl Default for FileIoService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpService for FileIoService {
    fn tools(&self) -> Vec<ToolDef> {
        // Reuse the existing inputSchema JSON verbatim from ToolRegistry.
        let schema_array = self.registry.list_tools();
        let arr = match schema_array.as_array() {
            Some(a) => a,
            None => return Vec::new(),
        };
        arr.iter()
            .filter_map(|tool| {
                let name = tool.get("name")?.as_str()?;
                let description = tool.get("description")?.as_str()?;
                let input_schema = tool.get("inputSchema")?.clone();
                Some(ToolDef::new(name, description, input_schema))
            })
            .collect()
    }

    async fn call_tool(&self, name: &str, arguments: &Value) -> Result<ToolReply, CallError> {
        match self.registry.execute_tool(name, arguments).await {
            Ok(result) => {
                // The registry returns a Value shaped like:
                //   {"content": [{"type":"text","text":"..."}]}
                // We extract the text, try to parse as JSON for structuredContent,
                // and forward as a ToolReply.
                let text = result
                    .get("content")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|entry| entry.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();

                // If the text is valid JSON, attach it as structuredContent too.
                let structured = serde_json::from_str::<Value>(&text).ok();
                let mut reply = ToolReply::text(text);
                if let Some(v) = structured {
                    reply = reply.with_structured(v);
                }
                Ok(reply)
            }
            Err(e) => map_error(e),
        }
    }
}

/// Map a `FileIoMcpError` to the correct `CallError` variant.
///
/// - `InvalidParams` → `CallError::InvalidParams` (JSON-RPC -32602; the
///   client sees it as a protocol error, which is correct for
///   structurally-invalid arguments).
/// - Everything else (domain failures: NotFound, PermissionDenied, IO, …) →
///   `CallError::Tool` (surfaced as `isError: true` content per MCP spec).
fn map_error(e: FileIoMcpError) -> Result<ToolReply, CallError> {
    match e {
        FileIoMcpError::InvalidParams(msg) => Err(CallError::invalid_params(msg)),
        other => Err(CallError::tool(other.to_string())),
    }
}
