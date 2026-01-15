# Testing (fileio-mcp)

This repo uses a Python-based MCP integration test harness that spawns the server in stdio mode and exercises the MCP tools end-to-end.

## Source of truth

- Test runner: `scripts/test_fileio_tools.py`
- Test workspace: created as a temporary directory at runtime

## Rust integration tests

There is also a Rust integration test suite that speaks MCP JSON-RPC over stdio:

- Rust suite: `tests/mcp_stdio_suite.rs`

This suite spawns `fileio-mcp serve --mode stdio`, performs MCP initialization, and exercises a representative set of tools end-to-end.

The harness is intentionally **one tool call per test**:
1) Python sets up preconditions (files/dirs/links)
2) Exactly one MCP `tools/call` is performed
3) Python validates just that tool call’s return shape and/or filesystem side effects

## Important: run tests in Docker

The supported way to run tests is inside a Docker container to ensure:
- consistent OS/filesystem behavior
- consistent permissions/umask
- consistent tool availability (Rust/Python)

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
./.venv/bin/python -u scripts/test_fileio_tools.py
```

Rust-only (no Python):

```bash
cargo test
```

Environment toggles:

- `RUN_DANGEROUS=1` — enables the dangerous-tool tests (auto-enabled inside Docker/Podman containers). Use with care.
- `KEEP_TEST_DIR=1` — keeps the temporary test directory after the run for debugging.

## Expected output

- Prints a per-test PASS/FAIL/SKIP line
- Exits with code `0` on success, `1` if any tests fail

## Troubleshooting

- If the harness can’t find a server binary, it tries (in order):
  1) `target/debug/fileio-mcp`
  2) `fileio-mcp` on `PATH`
  3) `cargo run -- --mode stdio`
- If you see JSON-RPC “Server not initialized”, the server did not receive `initialize`/`initialized`.
- If you see tool parameter shape errors, prefer matching the current Rust implementation (some schemas may be more permissive than the actual parser).
