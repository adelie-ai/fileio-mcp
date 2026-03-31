# Security Audit — fileio-mcp

**Date:** 2026-03-31
**Scope:** File I/O MCP server

---

## Design Note

fileio-mcp intentionally provides arbitrary filesystem access within the process user's permissions. It is designed to be run as the local user, not exposed to untrusted clients. The findings below identify areas where defense-in-depth could be improved.

---

## High Severity

### 1. No Path Allowlist/Denylist

**Files:** All operation files use `shellexpand::full()` without path bounds checking.

No directory allowlist or denylist is enforced. Callers can access any file the process user can reach, including `~/../../../etc/passwd`.

**Recommendation:** Add optional configurable allowlist of base directories. Canonicalize paths and verify they remain within bounds.

---

### 2. Recursive Copy Follows Symlinks

**File:** `src/operations/cp.rs:205-246`

`copy_dir_all()` uses `path.is_dir()` which follows symlinks. A symlink to `/etc` inside a copied directory would be recursively copied, exposing sensitive files.

**Recommendation:** Use `symlink_metadata()` to detect symlinks and skip or error on them during recursive operations.

---

### 3. Predictable Temp File in write_file

**File:** `src/operations/write_file.rs:56`

```rust
let temp_path = format!("{}.tmp", expanded_path);
```

Predictable temp filename enables symlink attacks. The `tempfile` crate is already in dependencies.

**Recommendation:** Use `tempfile::NamedTempFile::new_in(parent)` + `.persist()` for atomic writes with unpredictable names.

---

### 4. No File Size Limits on Reads

**File:** `src/operations/read_lines.rs:25-42`, `src/operations/find_in_files.rs:141-148`

Files are read entirely into memory with no size check. A multi-GB file causes OOM.

**Recommendation:** Check `metadata.len()` before reading and reject files above a configurable limit (e.g. 100 MiB).

---

## Medium Severity

### 5. Created Files Use Default Umask Permissions

**Files:** `src/operations/write_file.rs`, `src/operations/touch.rs`, `src/operations/mkdir.rs`

No explicit `chmod` after file/directory creation. If the process umask is permissive, files may be world-readable.

**Recommendation:** Not actionable in general (user's umask is their choice), but consider documenting or optionally setting `0o600`/`0o700`.

---

### 6. Symlink Following Inconsistencies

**Files:** `src/operations/find_in_files.rs:79`, `src/operations/file_find.rs:36`, `src/operations/list_dir.rs`

`WalkBuilder` follows symlinks by default. `stat()` uses `fs::metadata()` (follows symlinks) while `readlink()` uses `symlink_metadata()`.

**Recommendation:** Use `follow_links(false)` on WalkBuilder. Use `symlink_metadata()` consistently.

---

### 7. Unbounded Results in find_in_files

**File:** `src/operations/find_in_files.rs:150-182`

`max_count` limits matches per file but not total matches. Searching for `.` across a large codebase could return millions of results.

**Recommendation:** Add a global max results cap (e.g. 10,000).

---

### 8. Unrestricted Symlink/Hard Link Creation

**File:** `src/operations/link.rs:44, 77-148`

Symlinks and hard links can point anywhere without validation.

**Recommendation:** If an allowlist is implemented (finding #1), validate that link targets remain within allowed directories.

---

### 9. User-Supplied Regex (ReDoS)

**File:** `src/operations/edit_file.rs:296-297`

User-provided regex patterns could cause catastrophic backtracking.

**Recommendation:** The `regex` crate has built-in protections against worst-case backtracking (it uses Thompson NFA). This is low risk in practice.

---

## Positive Findings

- No `unsafe` blocks
- No shell command spawning (pure Rust fs APIs)
- Atomic rename for write operations (aside from predictable temp path)
- `#![deny(warnings)]` enforced
