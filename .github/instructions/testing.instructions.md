# Testing (fileio-mcp)

This repo uses Rust MCP integration tests that spawn the server in stdio mode and exercise the MCP tools end-to-end.

## Source of truth

- Test runner: `cargo test`
- Source of truth suite: `tests/mcp_stdio_suite.rs`
- Test workspace: created as a temporary directory at runtime

## MCP stdio integration tests

The integration suite in `tests/mcp_stdio_suite.rs` spawns `fileio-mcp serve --mode stdio`, performs MCP initialization, and exercises tools end-to-end over JSON-RPC.

The harness is intentionally **one tool call per test**:
1) Rust sets up preconditions (files/dirs/links)
2) Exactly one MCP `tools/call` is performed
3) Rust validates just that tool call’s return shape and/or filesystem side effects

## Important: run tests in Docker

The supported way to run tests is inside a Docker container to ensure:
- consistent OS/filesystem behavior
- consistent permissions/umask
- consistent tool availability (Rust)

Local host runs may work, but are not considered the primary or reproducible path.

Use the provided `Dockerfile` + `Justfile` to make this a one-command workflow.

## What the tests do

- Spawns `fileio-mcp serve --mode stdio`
- Performs MCP initialization
- Calls tool methods and validates outcomes
- Skips dangerous operations by default:
  - `fileio_change_ownership`

The harness auto-enables dangerous tests when it detects it is running as root inside a container (Docker/Podman).

## Running (today)

Preferred (Docker):

```bash
just test
```

Local (not the primary supported path):

```bash
cargo test
```

Environment toggles:

- `RUN_DANGEROUS=1` — enables the dangerous-tool tests (auto-enabled inside Docker/Podman containers). Use with care.
- `KEEP_TEST_DIR=1` — keeps the temporary test directory after the run for debugging.

## Expected output

- Standard `cargo test` output
- Exits with code `0` on success, `101` if any tests fail

## Troubleshooting
- If you see JSON-RPC “Server not initialized”, the server did not receive `initialize`/`initialized`.
- If you see tool parameter shape errors, prefer matching the current Rust implementation (some schemas may be more permissive than the actual parser).
