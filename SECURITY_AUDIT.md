# Security Audit — fileio-mcp

**Date:** 2026-03-31
**Scope:** File I/O MCP server

---

## Design Note

fileio-mcp intentionally provides arbitrary filesystem access within the process user's permissions. It is designed to be run as the local user, not exposed to untrusted clients.

---

## High Severity

### 1. No Path Allowlist/Denylist (HIGH) - RESOLVED

**Files:** `src/path_guard.rs`, `src/tools.rs`.

`PathGuard` now holds an allowlist of roots, and refuses everything else. It
resolves a path before it compares, so a symlink out of a root and a `..`
traversal are both decided on the real path, and it fails closed on anything it
cannot identify. The check runs on tool arguments and on the paths a tool
returns. See [docs/path_safety.md](docs/path_safety.md).

Open, and stated there: the guard does not close the race between the check and
the open.

---

## Medium Severity

### 2. Created Files Use Default Umask Permissions (MEDIUM)

**Files:** `src/operations/write_file.rs`, `src/operations/touch.rs`, `src/operations/mkdir.rs`

No explicit `chmod` after file/directory creation. If the process umask is permissive, files may be world-readable.

**Recommendation:** Document or optionally set `0o600`/`0o700`.

---

### 3. Symlink Following Inconsistencies (MEDIUM)

**Files:** `src/operations/find_in_files.rs`, `src/operations/file_find.rs`, `src/operations/list_dir.rs`

`WalkBuilder` follows symlinks by default. `stat()` uses `fs::metadata()` (follows symlinks) while `readlink()` uses `symlink_metadata()`.

**Recommendation:** Use `follow_links(false)` on WalkBuilder. Use `symlink_metadata()` consistently.

---

### 4. Unbounded Results in find_in_files (MEDIUM)

**File:** `src/operations/find_in_files.rs:150-182`

`max_count` limits matches per file but not total matches.

**Recommendation:** Add a global max results cap (e.g. 10,000).

---

### 5. Unrestricted Symlink/Hard Link Creation (MEDIUM)

**File:** `src/operations/link.rs:44, 77-148`

Symlinks and hard links can point anywhere without validation.

**Status:** Bounded by the allowlist. `fileio_create_hard_link` and
`fileio_create_symbolic_link` check both the target and the link path, so
neither end can leave an allowed root. The operations themselves still place no
limit of their own.

---

## Positive Findings

- No `unsafe` blocks
- No shell command spawning (pure Rust fs APIs)
- Atomic rename for write operations with secure temp files
- `#![deny(warnings)]` enforced
