# Run MCP integration tests in Docker.
# Requires: docker, just

set shell := ["bash", "-euo", "pipefail", "-c"]

image := "fileio-mcp-tests"
container := "fileio-mcp-tests"

# Build the test image
build:
  docker build -t {{image}} .

# Run the tests in a container (container deleted afterward)
test: build
  # Ensure we don't collide with a prior run
  docker rm -f {{container}} >/dev/null 2>&1 || true
  # --rm removes the container automatically on exit; rm -f is a safety net.
  # Pass through optional env toggles (if set on the host).
  docker run --name {{container}} --rm --env RUN_DANGEROUS --env KEEP_TEST_DIR {{image}}
  docker rm -f {{container}} >/dev/null 2>&1 || true

# --- Local verification ("local CI") ---
# Run locally instead of GitHub Actions. `install-hooks` wires `check` into a
# git pre-push hook so it runs automatically before every push.
check: fmt-check lint rust-build rust-test
fmt-check:
    cargo fmt --check
fmt:
    cargo fmt
lint:
    cargo clippy --all-targets -- -D warnings
rust-build:
    cargo build
rust-test:
    cargo test
test-integration:
    cargo test -- --ignored
premerge:
    git fetch origin
    git rebase origin/main
    just check
install-hooks:
    git config core.hooksPath .githooks
    @echo "pre-push hook active — bypass once with: git push --no-verify"
