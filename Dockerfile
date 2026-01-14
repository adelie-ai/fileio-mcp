# syntax=docker/dockerfile:1

# Build the fileio-mcp binary
FROM rust:1.92-trixie AS builder
WORKDIR /repo

# Cache deps first
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY docs ./docs

RUN cargo build --release --locked


# Runtime image: run the Python MCP integration harness
FROM python:3.14-slim
WORKDIR /repo

# Copy the built server binary onto PATH
COPY --from=builder /repo/target/release/fileio-mcp /usr/local/bin/fileio-mcp

# Copy the repo files needed by the tests
COPY scripts ./scripts

# Note: the test workspace directory is created by the harness at runtime.

# Default: run the one-tool-per-test harness
CMD ["python", "-u", "scripts/test_fileio_tools.py"]
