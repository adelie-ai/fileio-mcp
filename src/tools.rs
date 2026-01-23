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
                            "description": "Path to the file to read. Must exist and be readable. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect. If you need to read a specific file, use an absolute path or verify the working directory first."
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
                            "description": "Path to the file to write. Parent directories will be created if they don't exist. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect. If you need to write to a specific file, use an absolute path or verify the working directory first."
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
                "description": "Set file or directory permissions (chmod equivalent). Use this to change file permissions on Unix-like systems. Accepts octal format strings like '755' (rwxr-xr-x), '0644' (rw-r--r--), etc. The mode string can include or omit the leading zero. Works on files and directories. Can accept a single path string or an array of paths to set permissions on multiple files/directories.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Array of paths to files or directories whose permissions to change. All paths will have the same mode applied. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
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
                "description": "Set file or directory permissions (chmod equivalent). This is an alias for fileio_set_permissions with the same functionality. Accepts octal format strings like '755', '0644', etc. Use whichever name is more convenient. Accepts an array of paths to set permissions on multiple files/directories.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Array of paths to files or directories whose permissions to change. All paths will have the same mode applied. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
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
                "description": "Get file or directory permissions (mode) as an octal string. Returns the current permissions in octal format (e.g., '0755', '0644'). Useful for checking current permissions before modifying them or for auditing purposes. Can accept a single path string or an array of paths to get permissions for multiple files/directories.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Array of paths to files or directories to query. Returns permissions for all paths. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_touch",
                "description": "Touch files - creates them if they don't exist, or updates their access and modification timestamps to the current time if they do exist. Automatically creates parent directories if needed. Equivalent to the Unix 'touch' command. Useful for creating empty files or updating timestamps for build systems. Accepts an array of paths to touch multiple files.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Array of paths to files to touch. All files will be created or have their timestamps updated. Parent directories will be created if they don't exist. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_stat",
                "description": "Get comprehensive file or directory statistics. Returns detailed metadata including: size in bytes, file type (file/directory/symlink), permissions (mode) as octal string, timestamps (modified, accessed, created as Unix epoch seconds), and boolean flags (is_file, is_dir, is_symlink). Returns JSON with all available information about the file system entry. Can accept a single path string or an array of paths to get statistics for multiple files/directories.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Array of paths to files or directories to query. Returns statistics for all paths. Must exist. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_make_directory",
                "description": "Create directories. By default, creates parent directories recursively (equivalent to 'mkdir -p'). If recursive is false, will fail if parent directories don't exist. If the directory already exists, the operation succeeds (idempotent). Accepts an array of paths to create multiple directories.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Array of paths to directories to create. All directories will be created with the same recursive setting. Can be nested paths like '/a/b/c'. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
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
                            "description": "Path to the directory to list. Must exist and be a directory. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect. If you need to list a specific directory, use an absolute path or verify the working directory first."
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
                            "description": "Root directory to start searching from. Default: current directory ('.'). Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect. If you need to search a specific directory, use an absolute path or verify the working directory first."
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
                            "description": "Directory or file path to search in. If a file, searches only that file. If a directory, searches recursively through all files. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect. If you need to search a specific location, use an absolute path or verify the working directory first."
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
                "name": "fileio_edit_file",
                "description": "Edit a text file using deterministic, structured operations (LLM-friendly). Supports anchor-based edits (insert_before/insert_after/replace/delete with literal or regex search) and line-based edits (insert_at_line/replace_lines/delete_lines). Prefer this over patch-style diffs. By default, anchor-based edits require a match and will error if not found.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to edit. By default the file must exist unless create_if_missing=true. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
                        },
                        "edits": {
                            "type": "array",
                            "description": "Array of edit operations applied in order. Anchor-based ops: insert_after/insert_before/replace/delete require 'search' and optionally 'use_regex', 'occurrence' (1-based), 'require_match'. Line-based ops: insert_at_line requires 'line'; replace_lines/delete_lines require 'start_line' and 'end_line'.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "op": {
                                        "type": "string",
                                        "enum": [
                                            "insert_after",
                                            "insert_before",
                                            "replace",
                                            "delete",
                                            "insert_at_line",
                                            "replace_lines",
                                            "delete_lines"
                                        ]
                                    },
                                    "search": {"type": "string"},
                                    "text": {"type": "string"},
                                    "use_regex": {"type": "boolean"},
                                    "occurrence": {"type": "number"},
                                    "require_match": {"type": "boolean"},
                                    "line": {"type": "number"},
                                    "start_line": {"type": "number"},
                                    "end_line": {"type": "number"}
                                },
                                "required": ["op"]
                            }
                        },
                        "create_if_missing": {
                            "type": "boolean",
                            "description": "If true, creates the file if it does not exist (treats missing file as empty). Default: false."
                        },
                        "dry_run": {
                            "type": "boolean",
                            "description": "If true, does not write the file; returns the would-be content. Default: false."
                        },
                        "return_content": {
                            "type": "boolean",
                            "description": "If true, returns the updated file content in the tool result. Default: false (unless dry_run=true)."
                        }
                    },
                    "required": ["path", "edits"]
                }
            },
            {
                "name": "fileio_copy",
                "description": "Copy files or directories (cp equivalent). Copies the sources to the destination. Supports glob patterns in the source array (e.g., '*.txt', 'file?.log'). When using multiple sources, destination must be a directory. For files, creates a copy at the destination. For directories, requires recursive=true to copy the entire directory tree. If destination is a directory, the sources will be copied into it. If destination is a file path, it will be overwritten (only works with single source). Creates parent directories of destination if needed.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Array of source paths to copy. Can include glob patterns (e.g., '*.txt', 'file?.log', 'dir/*.rs'). All sources will be copied to the destination (which must be a directory when using multiple sources). Must exist or match existing files. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
                        },
                        "destination": {
                            "type": "string",
                            "description": "Destination path. For glob patterns or arrays: must be a directory. For single files: can be a file path or directory (source name preserved). For directories: must be a directory path or new directory name. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
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
                "description": "Move or rename files or directories (mv equivalent). Moves the sources to the destination location. Supports glob patterns in the source array (e.g., '*.txt', 'file?.log'). When using multiple sources, destination must be a directory. Can be used to rename (same directory, different name) or move (different location). Creates parent directories of destination if needed. The sources will no longer exist at their original locations after a successful move.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Array of source paths to move. Can include glob patterns (e.g., '*.txt', 'file?.log', 'dir/*.rs'). All sources will be moved to the destination (which must be a directory when using multiple sources). Must exist or match existing files. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
                        },
                        "destination": {
                            "type": "string",
                            "description": "Destination path. For glob patterns or arrays: must be a directory. For single files: can be a file path (rename) or directory path (move into directory). Parent directories will be created if needed. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
                        }
                    },
                    "required": ["source", "destination"]
                }
            },
            {
                "name": "fileio_remove",
                "description": "Remove files or directories (rm equivalent). Permanently deletes the specified path. Supports glob patterns (e.g., '*.tmp', 'file?.log', 'dir/*.bak') to remove multiple files matching the pattern, or an array of paths. For directories, recursive=true is required to remove non-empty directories. Use force=true to suppress errors if the file doesn't exist (idempotent removal). Warning: This operation cannot be undone.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Array of paths to files or directories to remove. Can include glob patterns (e.g., '*.tmp', 'file?.log', 'dir/*.bak'). All paths will be removed with the same recursive and force settings. Must exist or match existing files unless force=true. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
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
                "description": "Remove directories (rmdir equivalent). Specifically for removing directories. Requires recursive=true for non-empty directories. Will fail if any path is not a directory. Use this when you want to ensure you're only removing directories, not files. Accepts an array of paths to remove multiple directories.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Array of paths to directories to remove. All directories will be removed with the same recursive setting. Must exist and be directories. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
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
                            "description": "Target file path to link to. Must exist and be a file (not directory). Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect. If you need to link to a specific file, use an absolute path or verify the working directory first."
                        },
                        "link_path": {
                            "type": "string",
                            "description": "Path where the hard link will be created. Parent directories will be created if needed. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
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
                            "description": "Target file or directory path that the symlink will point to. Can be relative or absolute. Doesn't need to exist. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect. If you need to link to a specific file, use an absolute path or verify the working directory first."
                        },
                        "link_path": {
                            "type": "string",
                            "description": "Path where the symbolic link will be created. Parent directories will be created if needed. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
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
                            "description": "Path to extract basename from. Can be absolute or relative. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
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
                            "description": "Path to extract dirname from. Can be absolute or relative. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
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
                            "description": "Path to canonicalize. Can be relative or absolute, and can contain symlinks. Must exist. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect. If you need to canonicalize a specific file, use an absolute path or verify the working directory first."
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
                            "description": "Path to the symbolic link to read. Must exist and be a symbolic link. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect. If you need to read a specific symlink, use an absolute path or verify the working directory first."
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
                            "description": "Optional directory path where to create the temporary file/directory. If not provided, uses the system temporary directory. The directory must exist. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
                        }
                    },
                    "required": ["type"]
                }
            },
            {
                "name": "fileio_change_ownership",
                "description": "Change file or directory ownership (chown equivalent). Changes the owner and/or group of a file or directory. Currently supports numeric UID/GID only (username/groupname resolution not implemented). At least one of user or group must be provided. Requires appropriate permissions (typically root or file owner). Works on Unix-like systems only. Can accept a single path string or an array of paths to change ownership of multiple files/directories.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Array of paths to files or directories whose ownership to change. All paths will have the same ownership applied. Must exist. Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
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
                "name": "fileio_get_current_directory",
                "description": "Get the current working directory (pwd equivalent). Returns the absolute path of the current working directory. Useful for determining where relative paths will be resolved from, or for getting the current location in the file system.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "fileio_count_lines",
                "description": "Count the number of lines in files. Returns the total line count (newline-separated) for each file. Useful for getting line counts in code files, logs, or any text file. A file with no newlines counts as 1 line. Accepts an array of paths to count lines in multiple files.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "oneOf": [
                                {
                                    "type": "string",
                                    "description": "Path to the file to count lines in. Must exist and be a file (not directory). Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect. If you need to count lines in a specific file, use an absolute path or verify the working directory first."
                                },
                                {
                                    "type": "array",
                                    "items": {
                                        "type": "string"
                                    },
                                    "description": "Array of paths to files to count lines in. Returns line counts for all files."
                                }
                            ]
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fileio_count_words",
                "description": "Count the number of words in files. Returns the total word count (whitespace-separated) for each file. Useful for text analysis, document statistics, or content metrics. Words are separated by any whitespace (spaces, tabs, newlines). Accepts an array of paths to count words in multiple files.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Array of paths to files to count words in. Returns word counts for all files. Must exist and be files (not directories). Use absolute paths to avoid ambiguity - relative paths are resolved from the current working directory, which may not be the directory you expect."
                        }
                    },
                    "required": ["path"]
                }
            }
        ])
    }

    /// Helper to parse path parameter (array of strings)
    fn parse_paths(value: &Value) -> Result<Vec<String>> {
        let arr = value.as_array().ok_or_else(|| {
            crate::error::McpError::InvalidToolParameters(
                "Path must be an array of strings".to_string(),
            )
        })?;
        let mut paths = Vec::new();
        for item in arr {
            if let Some(s) = item.as_str() {
                paths.push(s.to_string());
            } else {
                return Err(crate::error::McpError::InvalidToolParameters(
                    "Path array must contain only strings".to_string(),
                )
                .into());
            }
        }
        Ok(paths)
    }

    fn parse_optional_u64(args: &serde_json::Map<String, Value>, key: &str) -> Result<Option<u64>> {
        match args.get(key) {
            None => Ok(None),
            Some(v) => {
                if v.is_null() {
                    return Ok(None);
                }
                if let Some(n) = v.as_u64() {
                    return Ok(Some(n));
                }
                Err(crate::error::McpError::InvalidToolParameters(format!(
                    "{} must be a non-negative integer",
                    key
                ))
                .into())
            }
        }
    }

    /// Execute a tool by name
    pub async fn execute_tool(&self, name: &str, arguments: &Value) -> Result<Value> {
        let args = arguments.as_object().ok_or_else(|| {
            crate::error::McpError::InvalidToolParameters("Arguments must be an object".to_string())
        })?;

        match name {
            "fileio_read_lines" => {
                let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: path".to_string(),
                    )
                })?;
                let start_line = Self::parse_optional_u64(args, "start_line")?;
                let end_line = Self::parse_optional_u64(args, "end_line")?;
                let line_count = Self::parse_optional_u64(args, "line_count")?;
                let start_offset = Self::parse_optional_u64(args, "start_offset")?;

                let lines = crate::operations::read_lines::read_lines(
                    path,
                    start_line,
                    end_line,
                    line_count,
                    start_offset,
                )?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&lines)
                            .map_err(|e| crate::error::FileIoMcpError::Json(e))?
                    }]
                }))
            }
            "fileio_write_file" => {
                let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
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
                let append = args
                    .get("append")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                crate::operations::write_file::write_file(path, content, append)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "File written successfully"
                    }]
                }))
            }
            "fileio_set_permissions" | "fileio_set_mode" => {
                let path_value = args.get("path").ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: path".to_string(),
                    )
                })?;
                let paths = Self::parse_paths(path_value)?;
                let mode = args.get("mode").and_then(|v| v.as_str()).ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: mode".to_string(),
                    )
                })?;

                let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
                crate::operations::file_mode::set_file_mode(&path_refs, mode)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "File mode set successfully"
                    }]
                }))
            }
            "fileio_get_permissions" => {
                let path_value = args.get("path").ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: path".to_string(),
                    )
                })?;
                let paths = Self::parse_paths(path_value)?;
                let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();

                let modes = crate::operations::get_mode::get_file_mode(&path_refs)?;
                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&modes)
                            .map_err(|e| crate::error::FileIoMcpError::Json(e))?
                    }]
                }))
            }
            "fileio_touch" => {
                let path_value = args.get("path").ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: path".to_string(),
                    )
                })?;
                let paths = Self::parse_paths(path_value)?;
                let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();

                crate::operations::touch::touch(&path_refs)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "File(s) touched successfully"
                    }]
                }))
            }
            "fileio_stat" => {
                let path_value = args.get("path").ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: path".to_string(),
                    )
                })?;
                let paths = Self::parse_paths(path_value)?;
                let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();

                let stat_results = crate::operations::stat::stat(&path_refs)?;
                let stat_json_array: Vec<Value> =
                    stat_results.into_iter().map(|s| s.into()).collect();

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&stat_json_array)
                            .map_err(|e| crate::error::FileIoMcpError::Json(e))?
                    }]
                }))
            }
            "fileio_make_directory" => {
                let path_value = args.get("path").ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: path".to_string(),
                    )
                })?;
                let paths = Self::parse_paths(path_value)?;
                let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
                let recursive = args
                    .get("recursive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                crate::operations::mkdir::mkdir(&path_refs, recursive)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "Directory(ies) created successfully"
                    }]
                }))
            }
            "fileio_list_directory" => {
                let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: path".to_string(),
                    )
                })?;
                let recursive = args
                    .get("recursive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let include_hidden = args
                    .get("include_hidden")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let entries =
                    crate::operations::list_dir::list_directory(path, recursive, include_hidden)?;
                let entries_json: Vec<Value> = entries.into_iter().map(|e| e.into()).collect();

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&entries_json)
                            .map_err(|e| crate::error::FileIoMcpError::Json(e))?
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
                let max_depth = args
                    .get("max_depth")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);
                let file_type = args.get("file_type").and_then(|v| v.as_str());

                let matches =
                    crate::operations::file_find::file_find(pattern, root, max_depth, file_type)?;
                let matches_json: Vec<Value> = matches.into_iter().map(|m| m.into()).collect();

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&matches_json)
                            .map_err(|e| crate::error::FileIoMcpError::Json(e))?
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
                let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: path".to_string(),
                    )
                })?;
                let case_sensitive = args
                    .get("case_sensitive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let use_regex = args
                    .get("use_regex")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let max_count = args.get("max_count").and_then(|v| v.as_u64());
                let max_depth = args
                    .get("max_depth")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);
                let include_hidden = args
                    .get("include_hidden")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let file_glob = args.get("file_glob").and_then(|v| v.as_str());
                let exclude_glob = args.get("exclude_glob").and_then(|v| v.as_str());
                let whole_word = args
                    .get("whole_word")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let multiline = args
                    .get("multiline")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

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

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&matches_json)
                            .map_err(|e| crate::error::FileIoMcpError::Json(e))?
                    }]
                }))
            }
            "fileio_edit_file" => {
                let req: crate::operations::edit_file::EditFileRequest =
                    serde_json::from_value(serde_json::Value::Object(args.clone()))
                        .map_err(|e| crate::error::FileIoMcpError::Json(e))?;
                let result = crate::operations::edit_file::edit_file(req)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&result)
                            .map_err(|e| crate::error::FileIoMcpError::Json(e))?
                    }]
                }))
            }
            "fileio_copy" => {
                let source_value = args.get("source").ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: source".to_string(),
                    )
                })?;
                let sources = Self::parse_paths(source_value)?;
                let source_refs: Vec<&str> = sources.iter().map(|s| s.as_str()).collect();
                let destination = args
                    .get("destination")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: destination".to_string(),
                        )
                    })?;
                let recursive = args
                    .get("recursive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let results = crate::operations::cp::cp(&source_refs, destination, recursive)?;
                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&results)
                            .map_err(|e| crate::error::FileIoMcpError::Json(e))?
                    }]
                }))
            }
            "fileio_move" => {
                let source_value = args.get("source").ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: source".to_string(),
                    )
                })?;
                let sources = Self::parse_paths(source_value)?;
                let source_refs: Vec<&str> = sources.iter().map(|s| s.as_str()).collect();
                let destination = args
                    .get("destination")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::McpError::InvalidToolParameters(
                            "Missing required parameter: destination".to_string(),
                        )
                    })?;

                let results = crate::operations::mv::mv(&source_refs, destination)?;
                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&results)
                            .map_err(|e| crate::error::FileIoMcpError::Json(e))?
                    }]
                }))
            }
            "fileio_remove" => {
                let path_value = args.get("path").ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: path".to_string(),
                    )
                })?;
                let paths = Self::parse_paths(path_value)?;
                let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
                let recursive = args
                    .get("recursive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

                let results = crate::operations::rm::rm(&path_refs, recursive, force)?;
                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&results)
                            .map_err(|e| crate::error::FileIoMcpError::Json(e))?
                    }]
                }))
            }
            "fileio_remove_directory" => {
                let path_value = args.get("path").ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: path".to_string(),
                    )
                })?;
                let paths = Self::parse_paths(path_value)?;
                let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
                let recursive = args
                    .get("recursive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let results = crate::operations::rmdir::rmdir(&path_refs, recursive)?;
                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&results)
                            .map_err(|e| crate::error::FileIoMcpError::Json(e))?
                    }]
                }))
            }
            "fileio_create_hard_link" => {
                let target = args.get("target").and_then(|v| v.as_str()).ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: target".to_string(),
                    )
                })?;
                let link_path =
                    args.get("link_path")
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
                let target = args.get("target").and_then(|v| v.as_str()).ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: target".to_string(),
                    )
                })?;
                let link_path =
                    args.get("link_path")
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
                let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
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
                let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
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
                let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
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
                let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
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
                let temp_type = args.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: type".to_string(),
                    )
                })?;
                let template = args.get("template").and_then(|v| v.as_str());

                let path = match temp_type {
                    "file" => crate::operations::mktemp::mktemp_file(template)?,
                    "dir" => crate::operations::mktemp::mktemp_dir(template)?,
                    _ => {
                        return Err(crate::error::McpError::InvalidToolParameters(format!(
                            "Invalid type: {} (must be 'file' or 'dir')",
                            temp_type
                        ))
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
                let path_value = args.get("path").ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: path".to_string(),
                    )
                })?;
                let paths = Self::parse_paths(path_value)?;
                let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
                let user = args.get("user").and_then(|v| v.as_str());
                let group = args.get("group").and_then(|v| v.as_str());

                crate::operations::chown::chown(&path_refs, user, group)?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "Ownership changed successfully"
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
                let path_value = args.get("path").ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: path".to_string(),
                    )
                })?;
                let paths = Self::parse_paths(path_value)?;
                let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();

                let counts = crate::operations::count_lines::count_lines(&path_refs)?;
                let counts_json = serde_json::to_string(&counts)
                    .map_err(|e| crate::error::FileIoMcpError::Json(e))?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": counts_json
                    }]
                }))
            }
            "fileio_count_words" => {
                let path_value = args.get("path").ok_or_else(|| {
                    crate::error::McpError::InvalidToolParameters(
                        "Missing required parameter: path".to_string(),
                    )
                })?;
                let paths = Self::parse_paths(path_value)?;
                let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();

                let counts = crate::operations::count_words::count_words(&path_refs)?;
                let counts_json = serde_json::to_string(&counts)
                    .map_err(|e| crate::error::FileIoMcpError::Json(e))?;

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": counts_json
                    }]
                }))
            }
            _ => Err(crate::error::McpError::ToolNotFound(name.to_string()).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_read_lines_rejects_negative_start_line() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "a").unwrap();
        let path = file.path().to_str().unwrap();

        let registry = ToolRegistry::new();
        let args = serde_json::json!({"path": path, "start_line": -1});
        let res = registry.execute_tool("fileio_read_lines", &args).await;
        assert!(res.is_err());
        let msg = format!("{}", res.err().unwrap());
        assert!(msg.to_lowercase().contains("start_line"));
        assert!(msg.to_lowercase().contains("non-negative"));
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
