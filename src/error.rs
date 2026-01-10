#![deny(warnings)]

// Error types for the fileio-mcp crate

use thiserror::Error;

/// Main error type for the fileio-mcp application
#[derive(Error, Debug)]
pub enum FileIoMcpError {
    /// File I/O operation errors
    #[error("File I/O error: {0}")]
    FileIo(#[from] FileIoError),

    /// JSON serialization/deserialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// MCP protocol errors
    #[error("MCP protocol error: {0}")]
    Mcp(#[from] McpError),

    /// Transport layer errors
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// File I/O operation errors
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

/// MCP protocol errors
#[derive(Error, Debug)]
pub enum McpError {
    /// Invalid protocol version
    #[error("Unsupported protocol version: {0}")]
    InvalidProtocolVersion(String),

    /// Invalid JSON-RPC message
    #[error("Invalid JSON-RPC message: {0}")]
    InvalidJsonRpc(String),

    /// Tool not found
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// Invalid tool parameters
    #[error("Invalid tool parameters: {0}")]
    InvalidToolParameters(String),
}

/// Transport layer errors
#[derive(Error, Debug)]
pub enum TransportError {
    /// WebSocket connection error
    #[error("WebSocket connection error: {0}")]
    WebSocket(String),

    /// Invalid message format
    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    /// Connection closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// IO error in transport
    #[error("Transport IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, FileIoMcpError>;

impl FileIoError {
    /// Map a std::io::Error to a more specific FileIoError based on the error kind
    pub fn from_io_error(operation: &str, path: &str, error: std::io::Error) -> Self {
        use std::io::ErrorKind;
        match error.kind() {
            ErrorKind::NotFound => {
                FileIoError::NotFound(format!("{} not found: {}", operation, path))
            }
            ErrorKind::PermissionDenied => {
                FileIoError::PermissionDenied(format!("Permission denied when {}: {}", operation, path))
            }
            ErrorKind::AlreadyExists => {
                FileIoError::WriteError(format!("{} already exists: {}", operation, path))
            }
            ErrorKind::InvalidInput => {
                FileIoError::InvalidPath(format!("Invalid input for {}: {} ({})", operation, path, error))
            }
            _ => {
                // For other errors, include the original error message
                FileIoError::WriteError(format!("Failed to {} {}: {}", operation, path, error))
            }
        }
    }
}
