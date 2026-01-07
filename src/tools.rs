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
                "description": "Read lines from a file with flexible windowing options. Use this to read specific line ranges from a file. Supports two modes: (1) start_line/end_line for range-based reading, or (2) start_line/line_count for count-based reading. Line numbers are 1-based. If no parameters are provided, reads the entire file. Returns an array of line objects with line_number and content fields.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to read. Must exist and be readable."
                        },
                        "start_line": {
                            "type": "number",
                            "description": "Starting line number (1-based, inclusive). Use with end_line for range, or with line_count for count-based reading."
                        },
                        "end_line": {
                            "type": "number",
                            "description": "Ending line number (1-based, inclusive). Used with start_line to define a range."
                        },
                        "line_count": {
                            "type": "number",
                            "description": "Number of lines to read starting from start_line. Alternative to end_line for count-based reading."
                        },
                        "start_offset": {
                            "type": "number",
                            "description": "Starting byte offset (0-based) as alternative to start_line. Less commonly used."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_write_file",
                "description": "Write content to a file. This tool will create the file if it doesn't exist, and create any necessary parent directories automatically. By default, overwrites existing files. Use append mode to add content to the end of an existing file. The write operation is atomic (uses temporary file then rename) to prevent corruption.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to write. Parent directories will be created if they don't exist."
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write to the file. Can be multi-line text."
                        },
                        "append": {
                            "type": "boolean",
                            "description": "If true, append content to the end of the file instead of overwriting. Default: false (overwrite)."
                        }
                    },
                    "required": ["path", "content"]
                }
            },
            {
                "name": "fileio_set_permissions",
                "description": "Set file or directory permissions (chmod equivalent). Use this to change file permissions on Unix-like systems. Accepts octal format strings like '755' (rwxr-xr-x), '0644' (rw-r--r--), etc. The mode string can include or omit the leading zero. Works on files and directories.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file or directory whose permissions to change"
                        },
                        "mode": {
                            "type": "string",
                            "description": "File mode in octal format. Examples: '755' (executable), '644' (readable), '600' (owner only), '0644' (same as 644). Format: owner-group-other permissions as 3 octal digits."
                        }
                    },
                    "required": ["path", "mode"]
                }
            },
            {
                "name": "fileio_set_mode",
                "description": "Set file or directory permissions (chmod equivalent). This is an alias for fileio_set_permissions with the same functionality. Accepts octal format strings like '755', '0644', etc. Use whichever name is more convenient.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file or directory whose permissions to change"
                        },
                        "mode": {
                            "type": "string",
                            "description": "File mode in octal format. Examples: '755' (executable), '644' (readable), '600' (owner only). Format: owner-group-other permissions as 3 octal digits."
                        }
                    },
                    "required": ["path", "mode"]
                }
            },
            {
                "name": "fileio_get_permissions",
                "description": "Get file or directory permissions (mode) as an octal string. Returns the current permissions in octal format (e.g., '0755', '0644'). Useful for checking current permissions before modifying them or for auditing purposes.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file or directory to query. Must exist."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_touch",
                "description": "Touch a file - creates it if it doesn't exist, or updates its access and modification timestamps to the current time if it does exist. Automatically creates parent directories if needed. Equivalent to the Unix 'touch' command. Useful for creating empty files or updating timestamps for build systems.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to touch. Parent directories will be created if they don't exist."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_stat",
                "description": "Get comprehensive file or directory statistics. Returns detailed metadata including: size in bytes, file type (file/directory/symlink), permissions (mode) as octal string, timestamps (modified, accessed, created as Unix epoch seconds), and boolean flags (is_file, is_dir, is_symlink). Returns JSON with all available information about the file system entry.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file or directory to query. Must exist."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_make_directory",
                "description": "Create a directory. By default, creates parent directories recursively (equivalent to 'mkdir -p'). If recursive is false, will fail if parent directories don't exist. If the directory already exists, the operation succeeds (idempotent).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the directory to create. Can be a nested path like '/a/b/c'."
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "Create parent directories if they don't exist. Default: true (like mkdir -p). Set to false to only create the final directory if all parents exist."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_list_directory",
                "description": "List directory contents with detailed information. Returns an array of entries, each containing: name, full path, entry type (file/directory/symlink), size in bytes, and modified timestamp. Can list recursively to include all subdirectories, and can include or exclude hidden files (those starting with '.'). Useful for directory exploration and file discovery.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the directory to list. Must exist and be a directory."
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "If true, recursively list all subdirectories and their contents. Default: false (only immediate children)."
                        },
                        "include_hidden": {
                            "type": "boolean",
                            "description": "If true, include hidden files and directories (those starting with '.'). Default: false (exclude hidden files)."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_find_files",
                "description": "Find files and directories matching a pattern. Uses efficient file system traversal with glob-like pattern matching. Supports wildcards: * (matches any sequence) and ? (matches single character). Can filter by file type and limit search depth. Returns an array of matching file paths. Similar to the 'find' command but with pattern matching.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Pattern to match against filenames. Supports * (any sequence) and ? (single character) wildcards. Examples: '*.txt', 'test?.log', 'file*'."
                        },
                        "root": {
                            "type": "string",
                            "description": "Root directory to start searching from. Default: current directory ('.')."
                        },
                        "max_depth": {
                            "type": "number",
                            "description": "Maximum directory depth to search. 0 = only root, 1 = root + immediate children, etc. If not specified, searches all depths."
                        },
                        "file_type": {
                            "type": "string",
                            "description": "Filter results by entry type. Options: 'file' (regular files only), 'dir' (directories only), 'symlink' (symbolic links only). If not specified, returns all types.",
                            "enum": ["file", "dir", "symlink"]
                        }
                    },
                    "required": ["pattern"]
                }
            },
            {
                "name": "fileio_find_in_files",
                "description": "Search for text or regex patterns within file contents (like grep/ripgrep). Recursively searches through files, returning matches with file path, line number, column range, and matched text. Supports both literal string matching and full regex patterns. Can filter by file glob patterns, limit search depth, control case sensitivity, and match whole words. Returns detailed match information for each occurrence.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Pattern to search for. Can be a literal string or regex pattern depending on use_regex setting. Examples: 'hello', '\\d+', 'function\\s+\\w+'."
                        },
                        "path": {
                            "type": "string",
                            "description": "Directory or file path to search in. If a file, searches only that file. If a directory, searches recursively through all files."
                        },
                        "case_sensitive": {
                            "type": "boolean",
                            "description": "If true, matching is case-sensitive (e.g., 'Hello' != 'hello'). Default: true."
                        },
                        "use_regex": {
                            "type": "boolean",
                            "description": "If true, treat pattern as a regular expression. If false, treat as literal string. Default: false (literal matching)."
                        },
                        "max_count": {
                            "type": "number",
                            "description": "Maximum number of matches to return per file. Useful for limiting output. If not specified, returns all matches."
                        },
                        "max_depth": {
                            "type": "number",
                            "description": "Maximum directory depth to search. 0 = only specified path, 1 = path + immediate children, etc. If not specified, searches all depths."
                        },
                        "include_hidden": {
                            "type": "boolean",
                            "description": "If true, search in hidden files and directories (those starting with '.'). Default: false (skip hidden files)."
                        },
                        "file_glob": {
                            "type": "string",
                            "description": "Include only files matching this glob pattern. Examples: '*.rs', '*.{js,ts}', 'test_*'. If not specified, searches all files."
                        },
                        "exclude_glob": {
                            "type": "string",
                            "description": "Exclude files matching this glob pattern. Examples: '*.log', 'node_modules/*', 'target/*'. Applied after file_glob filtering."
                        },
                        "whole_word": {
                            "type": "boolean",
                            "description": "If true, match only complete words (word boundaries). Example: 'test' matches 'test' but not 'testing'. Default: false."
                        },
                        "multiline": {
                            "type": "boolean",
                            "description": "If true, allow regex patterns to match across multiple lines. Only applies when use_regex is true. Default: false."
                        }
                    },
                    "required": ["pattern", "path"]
                }
            },
            {
                "name": "fileio_patch_file",
                "description": "Apply patches to files. Supports two formats: (1) unified diff format (standard patch format with @@ headers, - for removals, + for additions), and (2) add_remove_lines format (JSON with explicit line operations). Use this to modify files by applying structured changes rather than rewriting entire files. The file must exist before patching.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to patch. Must exist."
                        },
                        "patch": {
                            "type": "string",
                            "description": "Patch content. For unified_diff: standard unified diff text with @@ headers. For add_remove_lines: JSON string with operations array containing 'add' and 'remove' operations with line numbers and content."
                        },
                        "format": {
                            "type": "string",
                            "description": "Patch format. 'unified_diff' for standard patch format, 'add_remove_lines' for JSON-based line operations. Default: 'unified_diff'.",
                            "enum": ["unified_diff", "add_remove_lines"]
                        }
                    },
                    "required": ["path", "patch"]
                }
            },
            {
                "name": "fileio_copy",
                "description": "Copy files or directories (cp equivalent). Copies the source to the destination. Supports glob patterns in source (e.g., '*.txt', 'file?.log'). When using globs, multiple files can be copied to a destination directory. For files, creates a copy at the destination. For directories, requires recursive=true to copy the entire directory tree. If destination is a directory, the source will be copied into it. If destination is a file path, it will be overwritten (only works with single source). Creates parent directories of destination if needed.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source": {
                            "type": "string",
                            "description": "Source file or directory path to copy, or glob pattern (e.g., '*.txt', 'file?.log', 'dir/*.rs'). Must exist or match existing files."
                        },
                        "destination": {
                            "type": "string",
                            "description": "Destination path. For glob patterns: must be a directory. For single files: can be a file path or directory (source name preserved). For directories: must be a directory path or new directory name."
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "If true, copy directories recursively (required for copying directories). For files, this parameter is ignored. Default: false."
                        }
                    },
                    "required": ["source", "destination"]
                }
            },
            {
                "name": "fileio_move",
                "description": "Move or rename files or directories (mv equivalent). Moves the source to the destination location. Supports glob patterns in source (e.g., '*.txt', 'file?.log'). When using globs, multiple files can be moved to a destination directory. Can be used to rename (same directory, different name) or move (different location). Creates parent directories of destination if needed. The source will no longer exist at its original location after a successful move.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source": {
                            "type": "string",
                            "description": "Source file or directory path to move, or glob pattern (e.g., '*.txt', 'file?.log', 'dir/*.rs'). Must exist or match existing files."
                        },
                        "destination": {
                            "type": "string",
                            "description": "Destination path. For glob patterns: must be a directory. For single files: can be a file path (rename) or directory path (move into directory). Parent directories will be created if needed."
                        }
                    },
                    "required": ["source", "destination"]
                }
            },
            {
                "name": "fileio_remove",
                "description": "Remove files or directories (rm equivalent). Permanently deletes the specified path. Supports glob patterns (e.g., '*.tmp', 'file?.log', 'dir/*.bak') to remove multiple files matching the pattern. For directories, recursive=true is required to remove non-empty directories. Use force=true to suppress errors if the file doesn't exist (idempotent removal). Warning: This operation cannot be undone.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to file or directory to remove, or glob pattern (e.g., '*.tmp', 'file?.log', 'dir/*.bak'). Must exist or match existing files unless force=true."
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "If true, remove directories and all their contents recursively. Required for non-empty directories. For files, this parameter is ignored. Default: false."
                        },
                        "force": {
                            "type": "boolean",
                            "description": "If true, don't return an error if the file doesn't exist or no files match the pattern (idempotent). Default: false (error if missing/no matches)."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_remove_directory",
                "description": "Remove a directory (rmdir equivalent). Specifically for removing directories. Requires recursive=true for non-empty directories. Will fail if the path is not a directory. Use this when you want to ensure you're only removing directories, not files.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to directory to remove. Must exist and be a directory."
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "If true, remove directory and all contents recursively. Required for non-empty directories. Default: false (only empty directories)."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_create_hard_link",
                "description": "Create a hard link (ln equivalent). Creates a hard link from link_path to target. Both paths will refer to the same file data. Hard links only work for files (not directories) and must be on the same filesystem. The target must exist. Deleting either path doesn't delete the file until all links are removed.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "Target file path to link to. Must exist and be a file (not directory)."
                        },
                        "link_path": {
                            "type": "string",
                            "description": "Path where the hard link will be created. Parent directories will be created if needed."
                        }
                    },
                    "required": ["target", "link_path"]
                }
            },
            {
                "name": "fileio_create_symbolic_link",
                "description": "Create a symbolic link (ln -s equivalent). Creates a symbolic (soft) link from link_path to target. The target doesn't need to exist when creating the symlink. Symlinks can point to files or directories and can cross filesystem boundaries. If the target is moved or deleted, the symlink becomes broken.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "Target file or directory path that the symlink will point to. Can be relative or absolute. Doesn't need to exist."
                        },
                        "link_path": {
                            "type": "string",
                            "description": "Path where the symbolic link will be created. Parent directories will be created if needed."
                        }
                    },
                    "required": ["target", "link_path"]
                }
            },
            {
                "name": "fileio_get_basename",
                "description": "Extract the filename (basename) from a path. Returns just the final component of the path. Examples: '/path/to/file.txt' -> 'file.txt', 'file.txt' -> 'file.txt', '/usr/bin/' -> 'bin'. Useful for getting just the filename without the directory path.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to extract basename from. Can be absolute or relative."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_get_dirname",
                "description": "Extract the directory path (dirname) from a path. Returns the directory portion without the filename. Examples: '/path/to/file.txt' -> '/path/to', 'file.txt' -> '', '/usr/bin/' -> '/usr'. Returns empty string if no directory component exists.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to extract dirname from. Can be absolute or relative."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_get_canonical_path",
                "description": "Get the canonical (absolute, real) path, resolving all symbolic links and relative components. Returns the absolute path with all symlinks resolved and '..' and '.' components normalized. The path must exist. Useful for getting the true location of a file regardless of symlinks.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to canonicalize. Can be relative or absolute, and can contain symlinks. Must exist."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_read_symbolic_link",
                "description": "Read the target path of a symbolic link. Returns the path that the symlink points to. The symlink must exist and be a symbolic link (not a regular file or directory). Returns the target path as stored in the symlink, which may be relative or absolute.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the symbolic link to read. Must exist and be a symbolic link."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_create_temporary",
                "description": "Create a temporary file or directory (mktemp equivalent). Creates a uniquely named temporary file or directory and returns its path. The file/directory is created and persists (not automatically deleted). Use this when you need a temporary location for intermediate files. If template is provided, creates the temp file/dir in that directory; otherwise uses the system temp directory.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "description": "Type of temporary entry to create. 'file' creates an empty temporary file, 'dir' creates an empty temporary directory.",
                            "enum": ["file", "dir"]
                        },
                        "template": {
                            "type": "string",
                            "description": "Optional directory path where to create the temporary file/directory. If not provided, uses the system temporary directory. The directory must exist."
                        }
                    },
                    "required": ["type"]
                }
            },
            {
                "name": "fileio_change_ownership",
                "description": "Change file or directory ownership (chown equivalent). Changes the owner and/or group of a file or directory. Currently supports numeric UID/GID only (username/groupname resolution not implemented). At least one of user or group must be provided. Requires appropriate permissions (typically root or file owner). Works on Unix-like systems only.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to file or directory whose ownership to change. Must exist."
                        },
                        "user": {
                            "type": "string",
                            "description": "User ID (numeric UID as string, e.g., '1000'). Currently only numeric UIDs are supported. If not provided, user ownership is unchanged."
                        },
                        "group": {
                            "type": "string",
                            "description": "Group ID (numeric GID as string, e.g., '1000'). Currently only numeric GIDs are supported. If not provided, group ownership is unchanged."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_change_root",
                "description": "Change root directory (chroot equivalent). Changes the root directory of the current process to the specified path. This is a system-level operation that requires root privileges. After chroot, the process can only access files within the new root directory. The new_root must exist and be a directory. This operation affects the entire process and cannot be undone. Use with extreme caution.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "new_root": {
                            "type": "string",
                            "description": "New root directory path. Must exist and be a directory. Requires root/administrator privileges."
                        }
                    },
                    "required": ["new_root"]
                }
            },
            {
                "name": "fileio_get_current_directory",
                "description": "Get the current working directory (pwd equivalent). Returns the absolute path of the current working directory. Useful for determining where relative paths will be resolved from, or for getting the current location in the file system.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "fileio_count_lines",
                "description": "Count the number of lines in a file. Returns the total line count (newline-separated). Useful for getting line counts in code files, logs, or any text file. A file with no newlines counts as 1 line.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to count lines in. Must exist and be a file (not directory)."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_count_words",
                "description": "Count the number of words in a file. Returns the total word count (whitespace-separated). Useful for text analysis, document statistics, or content metrics. Words are separated by any whitespace (spaces, tabs, newlines).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to count words in. Must exist and be a file (not directory)."
                        }
                    },
                    "required": ["path"]
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
            "fileio_set_permissions" | "fileio_set_mode" => {
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
            "fileio_get_permissions" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;

                let mode = crate::operations::get_mode::get_file_mode(path)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": mode
                    }]
                }))
            }
            "fileio_touch" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;

                crate::operations::touch::touch(path)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "File touched successfully"
                    }]
                }))
            }
            "fileio_stat" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;

                let stat_result = crate::operations::stat::stat(path)?;
                let stat_json: Value = stat_result.into();
                let stat_text = serde_json::to_string(&stat_json)
                    .map_err(|e| crate::error::FileIoMcpError::Json(e))?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": stat_text
                    }]
                }))
            }
            "fileio_make_directory" => {
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
            "fileio_find_files" => {
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
            "fileio_copy" => {
                let source = args
                    .get("source")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: source".to_string(),
                        )
                    })?;
                let destination = args
                    .get("destination")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: destination".to_string(),
                        )
                    })?;
                let recursive = args.get("recursive").and_then(|v| v.as_bool()).unwrap_or(false);

                crate::operations::cp::cp(source, destination, recursive)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "File or directory copied successfully"
                    }]
                }))
            }
            "fileio_move" => {
                let source = args
                    .get("source")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: source".to_string(),
                        )
                    })?;
                let destination = args
                    .get("destination")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: destination".to_string(),
                        )
                    })?;

                crate::operations::mv::mv(source, destination)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "File or directory moved successfully"
                    }]
                }))
            }
            "fileio_remove" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;
                let recursive = args.get("recursive").and_then(|v| v.as_bool()).unwrap_or(false);
                let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

                crate::operations::rm::rm(path, recursive, force)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "File or directory removed successfully"
                    }]
                }))
            }
            "fileio_remove_directory" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;
                let recursive = args.get("recursive").and_then(|v| v.as_bool()).unwrap_or(false);

                crate::operations::rmdir::rmdir(path, recursive)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "Directory removed successfully"
                    }]
                }))
            }
            "fileio_create_hard_link" => {
                let target = args
                    .get("target")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: target".to_string(),
                        )
                    })?;
                let link_path = args
                    .get("link_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: link_path".to_string(),
                        )
                    })?;

                crate::operations::link::hard_link(target, link_path)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "Hard link created successfully"
                    }]
                }))
            }
            "fileio_create_symbolic_link" => {
                let target = args
                    .get("target")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: target".to_string(),
                        )
                    })?;
                let link_path = args
                    .get("link_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: link_path".to_string(),
                        )
                    })?;

                crate::operations::link::symlink(target, link_path)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "Symbolic link created successfully"
                    }]
                }))
            }
            "fileio_get_basename" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;

                let basename = crate::operations::path_utils::basename(path)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": basename
                    }]
                }))
            }
            "fileio_get_dirname" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;

                let dirname = crate::operations::path_utils::dirname(path)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": dirname
                    }]
                }))
            }
            "fileio_get_canonical_path" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;

                let realpath = crate::operations::path_utils::realpath(path)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": realpath
                    }]
                }))
            }
            "fileio_read_symbolic_link" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;

                let target = crate::operations::path_utils::readlink(path)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": target
                    }]
                }))
            }
            "fileio_create_temporary" => {
                let temp_type = args
                    .get("type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: type".to_string(),
                        )
                    })?;
                let template = args.get("template").and_then(|v| v.as_str());

                let path = match temp_type {
                    "file" => crate::operations::mktemp::mktemp_file(template)?,
                    "dir" => crate::operations::mktemp::mktemp_dir(template)?,
                    _ => {
                        return Err(crate::error::McpError::InvalidToolParameters(
                            format!("Invalid type: {} (must be 'file' or 'dir')", temp_type),
                        )
                        .into());
                    }
                };

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": path
                    }]
                }))
            }
            "fileio_change_ownership" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;
                let user = args.get("user").and_then(|v| v.as_str());
                let group = args.get("group").and_then(|v| v.as_str());

                crate::operations::chown::chown(path, user, group)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "Ownership changed successfully"
                    }]
                }))
            }
            "fileio_change_root" => {
                let new_root = args
                    .get("new_root")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: new_root".to_string(),
                        )
                    })?;

                crate::operations::chroot::chroot(new_root)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "Root directory changed successfully"
                    }]
                }))
            }
            "fileio_get_current_directory" => {
                let cwd = crate::operations::pwd::pwd()?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": cwd
                    }]
                }))
            }
            "fileio_count_lines" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;

                let count = crate::operations::count_lines::count_lines(path)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": count.to_string()
                    }]
                }))
            }
            "fileio_count_words" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: path".to_string(),
                        )
                    })?;

                let count = crate::operations::count_words::count_words(path)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": count.to_string()
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
