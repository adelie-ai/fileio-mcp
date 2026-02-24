# AGENTS.md — fileio-mcp

This file describes the project structure, conventions, and workflows for AI coding agents working on `fileio-mcp`.

---

## What this project is

`fileio-mcp` is a Rust **MCP server** (plus library) that exposes common filesystem operations as MCP tools. It is primarily intended for use by **LLM agents** and automation frameworks that need safe, auditable, deterministic file system access.

---

## Repository layout

```
fileio-mcp/
├── Cargo.toml                    # Crate manifest and dependencies
├── Justfile                      # Developer task runner
├── Dockerfile                    # Build + test image
├── AGENTS.md                     # This file
├── README.md                     # Human-facing documentation
├── docs/
│   └── result_shapes.md          # JSON shapes returned by multi-path operations
└── src/
    ├── main.rs                   # CLI entry-point (binary `fileio-mcp`)
    ├── lib.rs                    # Library interface and module declarations
    ├── server.rs                 # MCP server orchestration (initialize, tool dispatch, shutdown)
    ├── tools.rs                  # Tool registry: MCP JSON schemas + dispatch to operations
    ├── transport.rs              # STDIN/STDOUT and WebSocket transport
    ├── error.rs                  # All error types (FileIoMcpError, FileIoError, McpError, TransportError)
    └── operations/
        ├── mod.rs                # Module declarations
        ├── read_lines.rs         # fileio_read_lines
        ├── write_file.rs         # fileio_write_file
        ├── edit_file.rs          # fileio_edit_file
        ├── list_dir.rs           # fileio_list_directory
        ├── file_find.rs          # fileio_find_files
        ├── find_in_files.rs      # fileio_find_in_files
        ├── cp.rs                 # fileio_copy
        ├── mv.rs                 # fileio_move
        ├── rm.rs                 # fileio_remove
        ├── rmdir.rs              # fileio_remove_directory
        ├── mkdir.rs              # fileio_make_directory
        ├── stat.rs               # fileio_stat
        ├── touch.rs              # fileio_touch
        ├── mktemp.rs             # fileio_create_temporary
        ├── link.rs               # fileio_create_hard_link / fileio_create_symbolic_link
        ├── file_mode.rs          # fileio_set_permissions / fileio_set_mode
        ├── get_mode.rs           # fileio_get_permissions
        ├── chown.rs              # fileio_change_ownership
        ├── count_lines.rs        # fileio_count_lines
        ├── count_words.rs        # fileio_count_words
        ├── pwd.rs                # fileio_get_current_directory
        └── path_utils.rs         # Shared path helpers
```

---

## MCP tools (no dots in names — underscores only)

All tool names use the prefix `fileio_` followed by a snake_case verb phrase. **Never use dots in tool names.**

Key tool groups:

| Group | Tool names |
|-------|-----------|
| Read | `fileio_read_lines` |
| Write | `fileio_write_file`, `fileio_edit_file`, `fileio_touch` |
| Directory | `fileio_list_directory`, `fileio_make_directory`, `fileio_remove_directory` |
| Find | `fileio_find_files`, `fileio_find_in_files` |
| Copy/Move/Delete | `fileio_copy`, `fileio_move`, `fileio_remove` |
| Metadata | `fileio_stat`, `fileio_get_permissions`, `fileio_set_permissions`, `fileio_set_mode`, `fileio_change_ownership` |
| Path | `fileio_get_current_directory`, `fileio_get_basename`, `fileio_get_dirname`, `fileio_get_canonical_path` |
| Utility | `fileio_create_temporary`, `fileio_create_hard_link`, `fileio_create_symbolic_link`, `fileio_read_symbolic_link`, `fileio_count_lines`, `fileio_count_words` |

---

## Result shapes

All tools return `{ "content": [{ "type": "text", "text": "..." }] }`.

Multi-path operations return a JSON array of per-path result objects. See `docs/result_shapes.md` for exact shapes.

---

## Error handling conventions

- All errors flow through the `FileIoMcpError` top-level enum in `src/error.rs`.
- Use `FileIoError::NotFound` for missing paths, `FileIoError::PermissionDenied` for access errors.
- Use `McpError::InvalidToolParameters` in `tools.rs` when a required argument is missing.
- `Result<T>` is the type alias `std::result::Result<T, FileIoMcpError>`.
- Operations must return `Result<…>` — never panic.

---

## Adding a new tool

1. Create `src/operations/<verb_noun>.rs` with a function implementing the operation.
2. Register the module in `src/operations/mod.rs`.
3. Add the JSON schema to `ToolRegistry::list_tools()` in `src/tools.rs`.
4. Add the dispatch arm to `ToolRegistry::execute_tool()` in `src/tools.rs`.
5. Run `cargo clippy -- -D warnings` and `cargo test` to verify.

---

## Naming conventions

- **Tool names**: `fileio_<verb>_<noun(s)>` with underscores only. No dots.
- **Rust modules**: snake_case verb-noun matching the tool (e.g. `read_lines`, `list_dir`).
- **Rust types**: PascalCase (e.g. `ToolRegistry`, `FileIoError`).
- **JSON field names**: snake_case in all schemas and output.
- **Error variants**: descriptive PascalCase with a `String` payload.

---

## Transport

The server supports two transports (set with `--mode`):

| Mode | Description |
|------|-------------|
| `stdio` | STDIN/STDOUT, auto-detects newline-delimited JSON or `Content-Length` framing. Recommended for VS Code and local use. |
| `websocket` | WebSocket on `--host`/`--port`. Recommended for hosted deployments. |

---

## Building and running

```bash
# Build release binary
cargo build --release

# Run in stdio mode
./target/release/fileio-mcp serve --mode stdio

# Run tests (via Docker)
just test

# Run clippy (zero warnings policy)
cargo clippy -- -D warnings
```

---

## Coding standards

- All source files begin with `#![deny(warnings)]`.
- Zero clippy warnings (`all = "deny"` in `[lints.clippy]`).
- No `unwrap()` or `expect()` in non-test code — propagate errors with `?`.
- Keep `main.rs` as a thin wiring layer; all logic lives in `operations/`.
- Operations are pure functions of their filesystem inputs; no global mutable state.
- Tests live in `#[cfg(test)]` modules within the relevant source file or in `tests/`.
