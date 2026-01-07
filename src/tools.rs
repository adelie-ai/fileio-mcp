#![deny(warnings)]

// Tool registry and MCP tool definitions

use crate::error::Result;
use serde_json::Value;

/// Tool registry that manages all available tools
pub struct ToolRegistry;

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new() -> Self {
        Self
    }

    /// Get all tools in MCP format
    pub fn list_tools(&self) -> Value {
        serde_json::json!([
            {
                "name": "fileio_read_lines",
                "description": "Read lines from a file with optional windowing. Supports both start/end line numbers and start/count parameters.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to read"
                        },
                        "start_line": {
                            "type": "number",
                            "description": "Starting line number (1-based, inclusive)"
                        },
                        "end_line": {
                            "type": "number",
                            "description": "Ending line number (1-based, inclusive)"
                        },
                        "line_count": {
                            "type": "number",
                            "description": "Number of lines to read starting from start_line"
                        },
                        "start_offset": {
                            "type": "number",
                            "description": "Starting offset (0-based) as alternative to start_line"
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_write_file",
                "description": "Write content to a file. Creates parent directories if needed. Supports append mode.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to write"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write to the file"
                        },
                        "append": {
                            "type": "boolean",
                            "description": "If true, append to file instead of overwriting"
                        }
                    },
                    "required": ["path", "content"]
                }
            },
            {
                "name": "fileio_set_file_mode",
                "description": "Set file permissions (mode). Supports octal format (e.g., 755, 0644).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file"
                        },
                        "mode": {
                            "type": "string",
                            "description": "File mode in octal format (e.g., 755, 0644)"
                        }
                    },
                    "required": ["path", "mode"]
                }
            },
            {
                "name": "fileio_mkdir",
                "description": "Create a directory. By default creates parent directories recursively (like mkdir -p).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the directory to create"
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "Create parent directories if they don't exist (default: true)"
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_list_directory",
                "description": "List directory contents. Returns file/directory information including name, path, type, size, and modified time.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the directory to list"
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "Recursively list subdirectories"
                        },
                        "include_hidden": {
                            "type": "boolean",
                            "description": "Include hidden files and directories (starting with .)"
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_file_find",
                "description": "Find files matching a pattern. Uses glob-like pattern matching.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Pattern to match (supports * and ? wildcards)"
                        },
                        "root": {
                            "type": "string",
                            "description": "Root directory to search in (default: current directory)"
                        },
                        "max_depth": {
                            "type": "number",
                            "description": "Maximum directory depth to search"
                        },
                        "file_type": {
                            "type": "string",
                            "description": "Filter by file type: 'file', 'dir', 'symlink'",
                            "enum": ["file", "dir", "symlink"]
                        }
                    },
                    "required": ["pattern"]
                }
            },
            {
                "name": "fileio_find_in_files",
                "description": "Find text or regex patterns in files. Supports case-sensitive/insensitive matching, whole word matching, multiline matching, and various filtering options.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Pattern to search for (string or regex)"
                        },
                        "path": {
                            "type": "string",
                            "description": "Directory or file path to search in"
                        },
                        "case_sensitive": {
                            "type": "boolean",
                            "description": "Case-sensitive matching (default: true)"
                        },
                        "use_regex": {
                            "type": "boolean",
                            "description": "Treat pattern as regex instead of literal string (default: false)"
                        },
                        "max_count": {
                            "type": "number",
                            "description": "Maximum number of matches per file"
                        },
                        "max_depth": {
                            "type": "number",
                            "description": "Maximum directory depth to search"
                        },
                        "include_hidden": {
                            "type": "boolean",
                            "description": "Include hidden files and directories (default: false)"
                        },
                        "file_glob": {
                            "type": "string",
                            "description": "Include only files matching glob pattern"
                        },
                        "exclude_glob": {
                            "type": "string",
                            "description": "Exclude files matching glob pattern"
                        },
                        "whole_word": {
                            "type": "boolean",
                            "description": "Match whole words only (default: false)"
                        },
                        "multiline": {
                            "type": "boolean",
                            "description": "Enable multiline matching (default: false)"
                        }
                    },
                    "required": ["pattern", "path"]
                }
            },
            {
                "name": "fileio_patch_file",
                "description": "Apply patches to files. Supports unified diff format and add/remove lines format.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to patch"
                        },
                        "patch": {
                            "type": "string",
                            "description": "Patch content (unified diff or JSON for add/remove lines)"
                        },
                        "format": {
                            "type": "string",
                            "description": "Patch format: 'unified_diff' or 'add_remove_lines' (default: unified_diff)",
                            "enum": ["unified_diff", "add_remove_lines"]
                        }
                    },
                    "required": ["path", "patch"]
                }
            }
        ])
    }

    /// Execute a tool by name
    pub async fn execute_tool(
        &self,
        name: &str,
        arguments: &Value,
    ) -> Result<Value> {
        let args = arguments.as_object().ok_or_else(|| {
            crate::error::McpError::InvalidToolParameters("Arguments must be an object".to_string())
        })?;

        match name {
            "fileio_read_lines" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;
                let start_line = args.get("start_line").and_then(|v| v.as_u64());
                let end_line = args.get("end_line").and_then(|v| v.as_u64());
                let line_count = args.get("line_count").and_then(|v| v.as_u64());
                let start_offset = args.get("start_offset").and_then(|v| v.as_u64());

                let lines = crate::operations::read_lines::read_lines(
                    path, start_line, end_line, line_count, start_offset,
                )?;

                let lines_json = serde_json::to_string(&lines)
                    .map_err(|e| crate::error::FileIoMcpError::Json(e))?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": lines_json
                    }]
                }))
            }
            "fileio_write_file" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: content".to_string(),
                        )
                    })?;
                let append = args.get("append").and_then(|v| v.as_bool()).unwrap_or(false);

                crate::operations::write_file::write_file(path, content, append)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "File written successfully"
                    }]
                }))
            }
            "fileio_set_file_mode" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;
                let mode = args
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: mode".to_string(),
                        )
                    })?;

                crate::operations::file_mode::set_file_mode(path, mode)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "File mode set successfully"
                    }]
                }))
            }
            "fileio_mkdir" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;
                let recursive = args.get("recursive").and_then(|v| v.as_bool()).unwrap_or(true);

                crate::operations::mkdir::mkdir(path, recursive)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "Directory created successfully"
                    }]
                }))
            }
            "fileio_list_directory" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;
                let recursive = args.get("recursive").and_then(|v| v.as_bool()).unwrap_or(false);
                let include_hidden = args.get("include_hidden").and_then(|v| v.as_bool()).unwrap_or(false);

                let entries = crate::operations::list_dir::list_directory(path, recursive, include_hidden)?;
                let entries_json: Vec<Value> = entries.into_iter().map(|e| e.into()).collect();

                let entries_text = serde_json::to_string(&entries_json)
                    .map_err(|e| crate::error::FileIoMcpError::Json(e))?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": entries_text
                    }]
                }))
            }
            "fileio_file_find" => {
                let pattern = args
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: pattern".to_string(),
                        )
                    })?;
                let root = args.get("root").and_then(|v| v.as_str());
                let max_depth = args.get("max_depth").and_then(|v| v.as_u64()).map(|v| v as usize);
                let file_type = args.get("file_type").and_then(|v| v.as_str());

                let matches = crate::operations::file_find::file_find(pattern, root, max_depth, file_type)?;

                let matches_text = serde_json::to_string(&matches)
                    .map_err(|e| crate::error::FileIoMcpError::Json(e))?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": matches_text
                    }]
                }))
            }
            "fileio_find_in_files" => {
                let pattern = args
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: pattern".to_string(),
                        )
                    })?;
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;
                let case_sensitive = args.get("case_sensitive").and_then(|v| v.as_bool()).unwrap_or(true);
                let use_regex = args.get("use_regex").and_then(|v| v.as_bool()).unwrap_or(false);
                let max_count = args.get("max_count").and_then(|v| v.as_u64());
                let max_depth = args.get("max_depth").and_then(|v| v.as_u64()).map(|v| v as usize);
                let include_hidden = args.get("include_hidden").and_then(|v| v.as_bool()).unwrap_or(false);
                let file_glob = args.get("file_glob").and_then(|v| v.as_str());
                let exclude_glob = args.get("exclude_glob").and_then(|v| v.as_str());
                let whole_word = args.get("whole_word").and_then(|v| v.as_bool()).unwrap_or(false);
                let multiline = args.get("multiline").and_then(|v| v.as_bool()).unwrap_or(false);

                let matches = crate::operations::find_in_files::find_in_files(
                    pattern,
                    path,
                    case_sensitive,
                    use_regex,
                    max_count,
                    max_depth,
                    include_hidden,
                    file_glob,
                    exclude_glob,
                    whole_word,
                    multiline,
                )?;

                let matches_json: Vec<Value> = matches.into_iter().map(|m| m.into()).collect();

                let matches_text = serde_json::to_string(&matches_json)
                    .map_err(|e| crate::error::FileIoMcpError::Json(e))?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": matches_text
                    }]
                }))
            }
            "fileio_patch_file" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;
                let patch = args
                    .get("patch")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: patch".to_string(),
                        )
                    })?;
                let format = args.get("format").and_then(|v| v.as_str());

                crate::operations::patch_file::patch_file(path, patch, format)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "Patch applied successfully"
                    }]
                }))
            }
            _ => Err(crate::error::McpError::ToolNotFound(name.to_string()).into()),
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
