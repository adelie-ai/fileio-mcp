# fileio-mcp

A small, fast, and modular Rust service and library that exposes common filesystem operations over a simple IPC/RPC transport for automation, remote tooling, and embedded workflows.

## Why use fileio-mcp

- **Automate file operations safely**: Centralize file system actions (copy, move, read, write, mkdir, remove, stat, etc.) behind a single, auditable service.
- **Integrate with tools and editors**: Expose file operations to editors, CI systems, or sandboxes that can't perform certain filesystem tasks directly.
- **Reduce duplication**: Reuse a single, well-tested implementation of file primitives instead of reimplementing cross-platform behavior in multiple tools.
- **Rust safety and performance**: Implemented in Rust for predictable performance, strong error handling, and minimal runtime overhead.

## End-user benefits

- Single integration point for file I/O: fewer edge cases and consistent error semantics.
- Clear separation of responsibility: the service performs file work while clients remain lightweight.
- Easier permission and audit controls: operations are centralized and can be observed or limited at the transport layer.
- Extensible operation set: add or override operations as requirements evolve.

## What it is

`fileio-mcp` is both a library and a small server/CLI. It implements a set of canonical filesystem operations as modular, testable units (see the `operations/` folder), and offers transport and server layers to expose those operations to clients via IPC/RPC.

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

## Extending operations

To add an operation:

1. Add a new module in `src/operations/` that implements the operation logic and a small handler interface.
2. Register the operation in the server dispatch so it can be invoked over the transport.
3. Add unit tests for happy and error paths.

## Contributing

Contributions are welcome. Please follow the repository coding style and include tests for new operations or behavior changes.

## License

This project uses the Apache license. See LICENSE-APACHE and NOTICE for details.
