# fileio-mcp

A small, fast, and modular Rust **MCP server** (plus library) that exposes common filesystem operations over a simple IPC/RPC transport. It is primarily intended for use by **LLM agents** and other automated clients that need safe, auditable file system access.

## Who this is for

- **LLM agent runtimes** that need deterministic, tool-driven file I/O (read, write, edit, list, find).
- **Automation frameworks** that prefer a single, auditable service boundary for filesystem access.
- **Editors/CI/sandboxes** that want to delegate file operations to a hardened server.

## Why use fileio-mcp

- **Automate file operations safely**: Centralize file system actions (copy, move, read, write, mkdir, remove, stat, etc.) behind a single, auditable service.
- **Built for LLM agents**: Provide a stable MCP tool surface that LLMs can call deterministically for file I/O.
- **Integrate with tools and editors**: Expose file operations to editors, CI systems, or sandboxes that can't perform certain filesystem tasks directly.
- **Reduce duplication**: Reuse a single, well-tested implementation of file primitives instead of reimplementing cross-platform behavior in multiple tools.
- **Rust safety and performance**: Implemented in Rust for predictable performance, strong error handling, and minimal runtime overhead.

## End-user benefits

- Single integration point for file I/O: fewer edge cases and consistent error semantics.
- Clear separation of responsibility: the service performs file work while clients remain lightweight.
- Easier permission and audit controls: operations are centralized and can be observed or limited at the transport layer.
- Extensible operation set: add or override operations as requirements evolve.

## Key capabilities

- **Deterministic edits**: Structured edit operations (`fileio_edit_file`) avoid fragile patch diffs.
- **Line-aware reads**: Flexible read APIs with explicit 1-based line numbers.
- **Search utilities**: File and content search with filters and regex support.
- **Safe defaults**: Clear error semantics and explicit control over destructive operations.

## What it is

`fileio-mcp` is both a library and a small **MCP server/CLI**. It implements a set of canonical filesystem operations as modular, testable units (see the `operations/` folder), and exposes those operations as MCP tools over IPC/RPC. The primary use case is **LLM agents** that need reliable, deterministic file operations.

Key components:

- `src/main.rs` - CLI entrypoint / server runner (binary `fileio-mcp`).
- `src/server.rs` - Server orchestration and request handling.
- `src/transport.rs` - Abstractions for the transport mechanism used to accept client requests.
- `src/lib.rs` - Library interface and shared types.
- `src/operations/` - Individual operation implementations (cp, mv, rm, mkdir, stat, read/write, etc.).
- `src/error.rs` - Centralized error types and conversion utilities.

## How it works (high level)

1. A client sends a request over the configured transport to perform an operation (for example, copy a file or read lines).
2. The server receives the request and dispatches it to the matching operation handler in `operations/`.
3. The handler performs the filesystem action with careful error handling and returns a structured response.
4. The transport layer serializes the response back to the client.

Design points:

- Operations are implemented as focused modules so they're easy to test and reason about.
- Transport and server layers are separated from filesystem logic to allow different IPC/RPC mechanisms to be plugged in.
- Errors are structured and propagated so clients can make programmatic decisions based on failure modes.

## Build & run

Build the project (requires Rust toolchain):

```bash
cargo build --release
```

Run the server binary (example):

```bash
# from repository root
./target/release/fileio-mcp --help
```

The exact transport and runtime flags depend on how you embed or deploy the server; consult `src/main.rs` for CLI options and `src/transport.rs` for supported transports.

## Using the library

The crate can be included as a dependency to call operations directly from Rust code. The library surface is in `src/lib.rs` and the operations are available as modules under `src/operations` for programmatic use.

## Logging

Telemetry comes from `mcp-core`, which installs the subscriber and owns the request and tool-call spans; this server adds nothing of its own except what is documented below. See the [mcp-core README](https://github.com/adelie-ai/mcp-core#logging) for the full picture: the console layer, the `mcp.*` metrics recorded on every call, and the complete `OTEL_*` variable reference.

**Where it goes.** stderr, always, at every `RUST_LOG` level. The stdio transport frames JSON-RPC on stdout, so a log line there would corrupt the protocol stream.

```sh
RUST_LOG=debug fileio-mcp serve
RUST_LOG=info,fileio_mcp=debug fileio-mcp serve
```

**The level contract.** INFO carries ids, counts, durations and tool names, never content. DEBUG carries tool arguments, including every path this server touches. A denied path is no exception: this server's whole design keeps a rejection invisible to the model (see the `path_guard` module doc), so a denial is exactly where a path would most tempt a future change to log it. Never do that above DEBUG.

**This server's own metric.** `fileio.guard.rejections`, labelled by `reason` (`file` or `directory`, never the entry or the path that matched it). mcp-core's own `mcp.tools.call` counter cannot see a guard denial: the service returns a synthetic success or a plain "not found", so the call looks ordinary from the dispatch layer's side. This counter is the one place a rejection becomes observable, without telling the caller it happened.

**Exporting to a collector.** Off by default.

```toml
[features]
otel = ["mcp-core/otel"]
```

```sh
cargo build --features otel
OTEL_EXPORTER_OTLP_ENDPOINT=http://collector.example.com:4318 \
OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf \
  ./target/debug/fileio-mcp serve
```

With the feature off, `cargo tree` resolves no `opentelemetry*` crate and a default build pays nothing for it. With no collector configured, the process still writes a periodic metrics summary to stderr.

## Extending operations

To add an operation:

1. Add a new module in `src/operations/` that implements the operation logic and a small handler interface.
2. Register the operation in the server dispatch so it can be invoked over the transport.
3. Add unit tests for happy and error paths.

## Testing

Run the test suite with `cargo test`. For containerized, reproducible runs, see `.github/instructions/testing.instructions.md`.

## Contributing

Contributions are welcome. Please follow the repository coding style and include tests for new operations or behavior changes.

## License

This project uses the Apache license. See LICENSE-APACHE and NOTICE for details.
