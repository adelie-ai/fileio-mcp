# Security Audit — fileio-mcp

**Date:** 2026-03-31
**Scope:** File I/O MCP server

---

## Design Note

fileio-mcp intentionally provides arbitrary filesystem access within the process user's permissions. It is designed to be run as the local user, not exposed to untrusted clients.

---

## High Severity

### 1. No Path Allowlist/Denylist (HIGH)

**Files:** All operation files use `shellexpand::full()` without path bounds checking.

No directory allowlist or denylist is enforced. Callers can access any file the process user can reach.

**Recommendation:** Add optional configurable allowlist of base directories. Canonicalize paths and verify they remain within bounds.

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

**Recommendation:** If an allowlist is implemented, validate that link targets remain within allowed directories.

---

## Positive Findings

- No `unsafe` blocks
- No shell command spawning (pure Rust fs APIs)
- Atomic rename for write operations with secure temp files
- `#![deny(warnings)]` enforced
