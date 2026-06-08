#![deny(warnings)]

// Domain error types for fileio-mcp operations.
// Protocol-level dispatch is now owned by mcp-core.

use thiserror::Error;

/// Top-level error wrapping all fileio-mcp errors (used by operations and tools.rs).
#[derive(Error, Debug)]
pub enum FileIoMcpError {
    /// File I/O operation errors — tool-level failures (isError content).
    #[error("{0}")]
    FileIo(#[from] FileIoError),

    /// JSON serialization/deserialization errors — internal fault.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid tool parameters — maps to mcp-core InvalidParams (JSON-RPC -32602).
    #[error("{0}")]
    InvalidParams(String),

    /// IO errors — tool-level failures.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// File I/O operation errors.
#[derive(Error, Debug)]
pub enum FileIoError {
    /// File not found
    #[error("File not found: {0}")]
    NotFound(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Invalid path
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// Invalid file mode
    #[error("Invalid file mode: {0}")]
    InvalidMode(String),

    /// Read error
    #[error("Read error: {0}")]
    ReadError(String),

    /// Write error
    #[error("Write error: {0}")]
    WriteError(String),

    /// Patch application error
    #[error("Patch application error: {0}")]
    PatchError(String),

    /// Invalid line numbers
    #[error("Invalid line numbers: {0}")]
    InvalidLineNumbers(String),

    /// Regex compilation error
    #[error("Regex compilation error: {0}")]
    RegexError(#[from] regex::Error),
}

/// MCP protocol errors — kept for backward-compat with tools.rs.
/// `InvalidToolParameters` maps to a struct-param error (-32602 via mcp-core).
/// `ToolNotFound` is a tool-level error (isError content).
#[derive(Error, Debug)]
pub enum McpError {
    /// Tool not found — tool-level error (isError).
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// Invalid tool parameters — protocol-level invalid params.
    #[error("Invalid tool parameters: {0}")]
    InvalidToolParameters(String),
}

impl From<McpError> for FileIoMcpError {
    fn from(e: McpError) -> Self {
        match e {
            // Unknown tool is a tool-level error, not a parameter error.
            // Wrap it as FileIo so it flows through to CallError::Tool.
            McpError::ToolNotFound(msg) => FileIoMcpError::FileIo(FileIoError::NotFound(msg)),
            // Bad parameter shape is a parameter error → -32602.
            McpError::InvalidToolParameters(msg) => FileIoMcpError::InvalidParams(msg),
        }
    }
}

/// Result type alias for convenience.
pub type Result<T> = std::result::Result<T, FileIoMcpError>;

impl FileIoError {
    /// Map a std::io::Error to a more specific FileIoError based on the error kind.
    pub fn from_io_error(operation: &str, path: &str, error: std::io::Error) -> Self {
        use std::io::ErrorKind;
        match error.kind() {
            ErrorKind::NotFound => {
                FileIoError::NotFound(format!("{} not found: {}", operation, path))
            }
            ErrorKind::PermissionDenied => FileIoError::PermissionDenied(format!(
                "Permission denied when {}: {}",
                operation, path
            )),
            ErrorKind::AlreadyExists => {
                FileIoError::WriteError(format!("{} already exists: {}", operation, path))
            }
            ErrorKind::InvalidInput => FileIoError::InvalidPath(format!(
                "Invalid input for {}: {} ({})",
                operation, path, error
            )),
            _ => FileIoError::WriteError(format!("Failed to {} {}: {}", operation, path, error)),
        }
    }
}
