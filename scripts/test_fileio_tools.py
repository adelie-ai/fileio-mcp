#!/usr/bin/env python3
"""One-tool-per-test MCP integration harness for fileio-mcp.

Philosophy (per your request):
- Python stdlib sets the precondition for the tool under test.
- Exactly ONE MCP tool call is made in the test.
- Python stdlib validates only that tool's result and/or side-effects.

Notes:
- Some tools return `type: json` results, others `type: text`.
  The harness handles both, and will JSON-decode text payloads that look like JSON.
- Dangerous tools (`fileio_change_ownership`) are skipped unless:
    - `RUN_DANGEROUS=1` is set, OR
    - the harness detects it is running as root inside Docker.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple


ROOT_DIR = Path(__file__).resolve().parent.parent
WORKSPACE_DIR = ROOT_DIR

# Per-run temporary workspace directory for all test cases.
# Set KEEP_TEST_DIR=1 to preserve it after the run.
TEST_ROOT: Optional[Path] = None

# ANSI color codes
GREEN = "\033[92m"
RED = "\033[91m"
YELLOW = "\033[93m"
RESET = "\033[0m"
BOLD = "\033[1m"


class McpStdioClient:
    def __init__(self, cmd: List[str], cwd: Path):
        self._cmd = cmd
        self._cwd = cwd
        self._proc: Optional[subprocess.Popen[str]] = None
        self._next_id = 1

    def __enter__(self) -> "McpStdioClient":
        self._proc = subprocess.Popen(
            self._cmd,
            cwd=str(self._cwd),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        if self._proc is None:
            return
        try:
            if self._proc.poll() is None:
                try:
                    self.call("shutdown", {})
                except Exception:
                    pass
        finally:
            if self._proc.poll() is None:
                self._proc.terminate()
                try:
                    self._proc.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    self._proc.kill()

    def _send(self, obj: Dict[str, Any]) -> None:
        if self._proc is None or self._proc.stdin is None:
            raise RuntimeError("MCP process not started")
        self._proc.stdin.write(json.dumps(obj) + "\n")
        self._proc.stdin.flush()

    def _read_line(self) -> str:
        if self._proc is None or self._proc.stdout is None:
            raise RuntimeError("MCP process not started")
        line = self._proc.stdout.readline()
        if line == "":
            stderr = ""
            if self._proc.stderr is not None:
                try:
                    stderr = self._proc.stderr.read() or ""
                except Exception:
                    stderr = ""
            raise RuntimeError(f"MCP process closed stdout. stderr: {stderr}")
        return line.strip()

    def notify(self, method: str, params: Dict[str, Any]) -> None:
        self._send({"jsonrpc": "2.0", "method": method, "params": params})

    def call(self, method: str, params: Dict[str, Any]) -> Dict[str, Any]:
        request_id = self._next_id
        self._next_id += 1
        self._send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})

        while True:
            raw = self._read_line()
            try:
                msg = json.loads(raw)
            except json.JSONDecodeError:
                continue

            if msg.get("id") == request_id:
                if "error" in msg:
                    raise RuntimeError(f"JSON-RPC error for {method}: {msg['error']}")
                return msg

    def initialize(self, protocol_version: str = "2025-11-25") -> None:
        _ = self.call(
            "initialize",
            {"protocolVersion": protocol_version, "capabilities": {}},
        )
        self.notify("initialized", {})

    def tool_call(self, name: str, arguments: Dict[str, Any]) -> Dict[str, Any]:
        resp = self.call("tools/call", {"name": name, "arguments": arguments})
        return resp["result"]


def _extract_value(tool_result: Dict[str, Any]) -> Any:
    content = tool_result.get("content")
    if not isinstance(content, list):
        raise AssertionError(f"Expected result.content list, got: {tool_result}")

    for entry in content:
        if isinstance(entry, dict) and entry.get("type") == "json":
            return entry.get("value")

    for entry in content:
        if isinstance(entry, dict) and entry.get("type") == "text":
            text = entry.get("text")
            if isinstance(text, str):
                stripped = text.strip()
                if stripped.startswith("{") or stripped.startswith("["):
                    try:
                        return json.loads(stripped)
                    except json.JSONDecodeError:
                        return text
            return text

    raise AssertionError(f"No usable content entry in: {tool_result}")


def _pick_server_cmd() -> List[str]:
    local_debug = WORKSPACE_DIR / "target" / "debug" / "fileio-mcp"
    if local_debug.exists():
        return [str(local_debug), "serve", "--mode", "stdio"]

    installed = shutil.which("fileio-mcp")
    if installed is not None:
        return [installed, "serve", "--mode", "stdio"]

    return ["cargo", "run", "--quiet", "--", "serve", "--mode", "stdio"]


def _assert(cond: bool, msg: str) -> None:
    if not cond:
        raise AssertionError(msg)


def _assert_raises(substr: str, fn) -> None:
    try:
        fn()
    except Exception as e:
        msg = str(e)
        _assert(substr.lower() in msg.lower(), f"Expected error containing '{substr}', got: {msg}")
        return
    raise AssertionError(f"Expected error containing '{substr}', but no error was raised")


def _new_case_dir(case_name: str) -> Path:
    if TEST_ROOT is None:
        raise RuntimeError("TEST_ROOT not initialized")
    safe = "".join(ch if ch.isalnum() or ch in ("-", "_") else "_" for ch in case_name)
    return Path(tempfile.mkdtemp(prefix=f"{safe}_", dir=str(TEST_ROOT)))


def _create_test_root() -> Path:
    # Use mkdtemp (not TemporaryDirectory) so we can optionally keep it.
    return Path(tempfile.mkdtemp(prefix="fileio-mcp-tests_"))


@dataclass(frozen=True)
class TestCase:
    name: str
    tool: str
    fn: Any
    dangerous: bool = False


# -----------------
# Per-tool test cases
# -----------------

def test_fileio_write_file_overwrite(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_write_file_overwrite")
    path = case / "nested" / "out.txt"
    if path.exists():
        path.unlink()
    path.parent.mkdir(parents=True, exist_ok=True)
    # Precondition: remove file and parent exists. Now overwrite via tool.
    client.tool_call("fileio_write_file", {"path": str(path), "content": "hello\n", "append": False})
    _assert(path.exists(), "Expected file created")
    _assert(path.read_text() == "hello\n", "Unexpected file content")


def test_fileio_write_file_append(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_write_file_append")
    path = case / "append.txt"
    path.write_text("hello")
    client.tool_call("fileio_write_file", {"path": str(path), "content": " world", "append": True})
    _assert(path.read_text() == "hello world", "Append did not produce expected content")


def test_fileio_read_lines_ok(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_read_lines_ok")
    path = case / "in.txt"
    path.write_text("a\nb\nc\n")
    res = client.tool_call("fileio_read_lines", {"path": str(path)})
    val = _extract_value(res)
    if isinstance(val, list) and (not val or isinstance(val[0], str)):
        got = val
    elif isinstance(val, list) and (not val or isinstance(val[0], dict)):
        got = [x.get("content") for x in val]
    else:
        raise AssertionError(f"Unexpected read_lines payload: {val}")
    _assert(got == ["a", "b", "c"], f"Unexpected lines: {got}")


def test_fileio_read_lines_range(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_read_lines_range")
    path = case / "in.txt"
    path.write_text("l1\nl2\nl3\nl4\n")
    res = client.tool_call("fileio_read_lines", {"path": str(path), "start_line": 2, "end_line": 3})
    val = _extract_value(res)
    _assert(val == ["l2", "l3"], f"Unexpected range read: {val}")


def test_fileio_read_lines_line_count(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_read_lines_line_count")
    path = case / "in.txt"
    path.write_text("l1\nl2\nl3\nl4\n")
    res = client.tool_call("fileio_read_lines", {"path": str(path), "start_line": 2, "line_count": 2})
    val = _extract_value(res)
    _assert(val == ["l2", "l3"], f"Unexpected line_count read: {val}")


def test_fileio_read_lines_start_offset(client: McpStdioClient) -> None:
    # NOTE: despite schema saying "byte offset", implementation treats this as line index offset.
    case = _new_case_dir("fileio_read_lines_start_offset")
    path = case / "in.txt"
    path.write_text("l1\nl2\nl3\nl4\n")
    res = client.tool_call("fileio_read_lines", {"path": str(path), "start_offset": 1, "line_count": 2})
    val = _extract_value(res)
    _assert(val == ["l2", "l3"], f"Unexpected start_offset read: {val}")


def test_fileio_read_lines_empty_file_returns_empty(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_read_lines_empty")
    path = case / "empty.txt"
    path.write_text("")
    res = client.tool_call("fileio_read_lines", {"path": str(path)})
    val = _extract_value(res)
    _assert(val == [], f"Expected empty list, got: {val}")


def test_fileio_read_lines_end_past_eof_clamps(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_read_lines_end_past_eof")
    path = case / "in.txt"
    path.write_text("a\nb\nc\n")

    res = client.tool_call("fileio_read_lines", {"path": str(path), "start_line": 2, "end_line": 999})
    val = _extract_value(res)
    _assert(val == ["b", "c"], f"Unexpected clamp-to-EOF behavior: {val}")

    res = client.tool_call("fileio_read_lines", {"path": str(path), "start_line": 2, "line_count": 999})
    val = _extract_value(res)
    _assert(val == ["b", "c"], f"Unexpected clamp-to-EOF behavior (count): {val}")


def test_fileio_read_lines_start_line_beyond_eof_errors(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_read_lines_start_beyond_eof")
    path = case / "in.txt"
    path.write_text("a\nb\n")
    _assert_raises(
        "exceeds file length",
        lambda: client.tool_call("fileio_read_lines", {"path": str(path), "start_line": 5}),
    )


def test_fileio_read_lines_end_before_start_errors(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_read_lines_end_before_start")
    path = case / "in.txt"
    path.write_text("a\nb\n")
    _assert_raises(
        "end_line",
        lambda: client.tool_call("fileio_read_lines", {"path": str(path), "start_line": 2, "end_line": 1}),
    )


def test_fileio_read_lines_negative_numbers_rejected(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_read_lines_negative_numbers")
    path = case / "in.txt"
    path.write_text("a\n")
    _assert_raises(
        "non-negative",
        lambda: client.tool_call("fileio_read_lines", {"path": str(path), "start_line": -1}),
    )


def test_fileio_read_lines_missing_errors(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_read_lines_missing")
    missing = case / "missing.txt"
    if missing.exists():
        missing.unlink()
    _assert_raises("not found", lambda: client.tool_call("fileio_read_lines", {"path": str(missing)}))


def test_fileio_set_permissions(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_set_permissions")
    path = case / "perm.txt"
    path.write_text("x")
    client.tool_call("fileio_set_permissions", {"path": [str(path)], "mode": "700"})
    mode = oct(path.stat().st_mode & 0o777)
    _assert(mode == "0o700", f"Expected 700, got: {mode}")


def test_fileio_set_permissions_multiple_paths(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_set_permissions_multiple_paths")
    a = case / "a.txt"
    b = case / "b.txt"
    a.write_text("x")
    b.write_text("y")
    client.tool_call("fileio_set_permissions", {"path": [str(a), str(b)], "mode": "600"})
    _assert(oct(a.stat().st_mode & 0o777) == "0o600", "Expected 600 for a.txt")
    _assert(oct(b.stat().st_mode & 0o777) == "0o600", "Expected 600 for b.txt")


def test_fileio_set_mode_alias(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_set_mode")
    path = case / "perm2.txt"
    path.write_text("x")
    client.tool_call("fileio_set_mode", {"path": [str(path)], "mode": "644"})
    mode = oct(path.stat().st_mode & 0o777)
    _assert(mode == "0o644", f"Expected 644, got: {mode}")


def test_fileio_get_permissions(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_get_permissions")
    path = case / "perm.txt"
    path.write_text("x")
    os.chmod(path, 0o755)
    res = client.tool_call("fileio_get_permissions", {"path": [str(path)]})
    val = _extract_value(res)
    if isinstance(val, dict):
        mode = val.get(str(path))
    elif isinstance(val, list) and val and isinstance(val[0], dict):
        mode = val[0].get("mode")
    else:
        raise AssertionError(f"Unexpected get_permissions payload: {val}")
    _assert(str(mode).endswith("755"), f"Expected 755-ish, got: {mode}")


def test_fileio_get_permissions_multiple_paths(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_get_permissions_multiple_paths")
    a = case / "a.txt"
    b = case / "b.txt"
    a.write_text("x")
    b.write_text("y")
    os.chmod(a, 0o700)
    os.chmod(b, 0o644)
    res = client.tool_call("fileio_get_permissions", {"path": [str(a), str(b)]})
    val = _extract_value(res)
    _assert(isinstance(val, dict), f"Expected mapping, got: {val}")
    _assert(str(val.get(str(a), "")).endswith("700"), f"Unexpected a mode: {val}")
    _assert(str(val.get(str(b), "")).endswith("644"), f"Unexpected b mode: {val}")


def test_fileio_touch(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_touch")
    path = case / "touched.txt"
    if path.exists():
        path.unlink()
    client.tool_call("fileio_touch", {"path": [str(path)]})
    _assert(path.exists(), "Expected touched file to exist")


def test_fileio_stat(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_stat")
    exists = case / "a.txt"
    exists.write_text("hello")
    missing = case / "missing.txt"
    if missing.exists():
        missing.unlink()

    res = client.tool_call("fileio_stat", {"path": [str(exists), str(missing)]})
    val = _extract_value(res)
    _assert(isinstance(val, list), f"Unexpected stat payload: {val}")
    by_path = {x.get("path"): x for x in val if isinstance(x, dict)}
    _assert(by_path[str(exists)].get("exists") is True, "Expected exists=true")
    _assert(by_path[str(missing)].get("exists") is False, "Expected exists=false")


def test_fileio_make_directory(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_make_directory")
    target = case / "a" / "b" / "c"
    if target.exists():
        shutil.rmtree(target)
    client.tool_call("fileio_make_directory", {"path": [str(target)], "recursive": True})
    _assert(target.exists() and target.is_dir(), "Expected directory created")


def test_fileio_make_directory_non_recursive_errors(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_make_directory_non_recursive_errors")
    target = case / "missing_parent" / "child"
    if target.exists():
        shutil.rmtree(target)
    _assert_raises(
        "Some directory creations failed",
        lambda: client.tool_call("fileio_make_directory", {"path": [str(target)], "recursive": False}),
    )


def test_fileio_list_directory(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_list_directory")
    (case / "f1.txt").write_text("x")
    (case / "sub").mkdir()

    res = client.tool_call(
        "fileio_list_directory",
        {"path": str(case), "recursive": False, "include_hidden": False},
    )
    val = _extract_value(res)
    _assert(isinstance(val, list), f"Unexpected list_directory payload: {val}")
    names = {x.get("name") for x in val if isinstance(x, dict)}
    _assert("f1.txt" in names, f"Expected f1.txt in list, got: {sorted(names)}")
    _assert("sub" in names, f"Expected sub in list, got: {sorted(names)}")


def test_fileio_list_directory_recursive(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_list_directory_recursive")
    (case / "sub").mkdir()
    nested = case / "sub" / "nested.txt"
    nested.write_text("x")
    res = client.tool_call("fileio_list_directory", {"path": str(case), "recursive": True, "include_hidden": False})
    val = _extract_value(res)
    _assert(isinstance(val, list), f"Unexpected list_directory payload: {val}")
    paths = {x.get("path") for x in val if isinstance(x, dict)}
    _assert(str(nested) in paths, f"Expected nested file in recursive listing, got: {paths}")


def test_fileio_list_directory_include_hidden(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_list_directory_include_hidden")
    (case / ".hidden").write_text("x")
    res = client.tool_call("fileio_list_directory", {"path": str(case), "recursive": False, "include_hidden": True})
    val = _extract_value(res)
    names = {x.get("name") for x in val if isinstance(x, dict)}
    _assert(".hidden" in names, f"Expected .hidden in list, got: {sorted(names)}")


def test_fileio_list_directory_missing_returns_empty(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_list_directory_missing_returns_empty")
    missing_dir = case / "missing"
    if missing_dir.exists():
        shutil.rmtree(missing_dir)
    res = client.tool_call("fileio_list_directory", {"path": str(missing_dir), "recursive": False, "include_hidden": False})
    val = _extract_value(res)
    _assert(val == [], f"Expected empty list for missing dir, got: {val}")


def test_fileio_find_files(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_find_files")
    (case / "a.log").write_text("x")
    (case / "b.log").write_text("x")
    (case / "c.txt").write_text("x")
    res = client.tool_call("fileio_find_files", {"root": str(case), "pattern": "*.log"})
    val = _extract_value(res)
    _assert(isinstance(val, list), f"Unexpected find_files payload: {val}")
    found = {Path(p).name for p in val}
    _assert({"a.log", "b.log"}.issubset(found), f"Expected a.log and b.log, got: {sorted(found)}")


def test_fileio_find_files_file_type_dir(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_find_files_file_type_dir")
    d = case / "matchdir"
    d.mkdir()
    res = client.tool_call("fileio_find_files", {"root": str(case), "pattern": "matchdir", "file_type": "dir"})
    val = _extract_value(res)
    _assert(isinstance(val, list) and any(Path(p).name == "matchdir" for p in val), f"Expected matchdir, got: {val}")


def test_fileio_find_files_missing_root_errors(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_find_files_missing_root_errors")
    missing = case / "nope"
    if missing.exists():
        shutil.rmtree(missing)
    _assert_raises("not found", lambda: client.tool_call("fileio_find_files", {"root": str(missing), "pattern": "*.txt"}))


def test_fileio_find_in_files(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_find_in_files")
    hay = case / "hay.txt"
    hay.write_text("needle\nother\n")
    res = client.tool_call("fileio_find_in_files", {"path": str(case), "pattern": "needle", "use_regex": False})
    val = _extract_value(res)
    _assert(isinstance(val, list), f"Unexpected find_in_files payload: {val}")
    _assert(any(m.get("file_path") == str(hay) for m in val if isinstance(m, dict)), "Expected match in hay.txt")


def test_fileio_find_in_files_case_insensitive(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_find_in_files_case_insensitive")
    hay = case / "hay.txt"
    hay.write_text("Needle\n")
    res = client.tool_call(
        "fileio_find_in_files",
        {"path": str(case), "pattern": "needle", "use_regex": False, "case_sensitive": False},
    )
    val = _extract_value(res)
    _assert(any(m.get("file_path") == str(hay) for m in val if isinstance(m, dict)), "Expected case-insensitive match")


def test_fileio_find_in_files_whole_word(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_find_in_files_whole_word")
    hay = case / "hay.txt"
    hay.write_text("testing test tested\n")
    res = client.tool_call(
        "fileio_find_in_files",
        {"path": str(case), "pattern": "test", "use_regex": False, "whole_word": True},
    )
    val = _extract_value(res)
    _assert(any(m.get("file_path") == str(hay) for m in val if isinstance(m, dict)), "Expected whole-word match")


def test_fileio_patch_file_add_remove(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_patch_file_add_remove")
    path = case / "patch.txt"
    path.write_text("line 1\nline 2\nline 3\n")
    patch = json.dumps({"operations": [{"type": "add", "line": 2, "content": "inserted"}, {"type": "remove", "line": 3}]})
    client.tool_call("fileio_patch_file", {"path": str(path), "patch": patch, "format": "add_remove_lines"})
    _assert(path.read_text().splitlines() == ["line 1", "inserted", "line 2"], "Unexpected patched content")


def test_fileio_patch_file_unified_diff(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_patch_file_unified_diff")
    path = case / "patch.txt"
    path.write_text("line 1\nline 2\nline 3\n")
    diff = """@@ -1,3 +1,3 @@
 line 1
-line 2
+line two
 line 3
"""
    client.tool_call("fileio_patch_file", {"path": str(path), "patch": diff, "format": "unified_diff"})
    _assert(path.read_text().splitlines() == ["line 1", "line two", "line 3"], "Unexpected unified_diff patch result")


def test_fileio_patch_file_unified_diff_empty_file_add_line(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_patch_file_unified_diff_empty_add")
    path = case / "patch.txt"
    path.write_text("")
    diff = "--- a\n+++ b\n@@\n+first"
    client.tool_call("fileio_patch_file", {"path": str(path), "patch": diff, "format": "unified_diff"})
    _assert(path.read_text() == "first", f"Unexpected unified_diff patched content: {path.read_text()!r}")


def test_fileio_patch_file_unified_diff_add_at_end(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_patch_file_unified_diff_add_end")
    path = case / "patch.txt"
    path.write_text("line 1\nline 2\n")
    diff = "--- a\n+++ b\n@@\n line 1\n line 2\n+line 3"
    client.tool_call("fileio_patch_file", {"path": str(path), "patch": diff, "format": "unified_diff"})
    _assert(
        path.read_text() == "line 1\nline 2\nline 3",
        f"Unexpected unified_diff patched content: {path.read_text()!r}",
    )


def test_fileio_patch_file_add_remove_lines_empty_file_add_first_line(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_patch_file_add_remove_empty_add")
    path = case / "patch.txt"
    path.write_text("")
    patch = json.dumps({"operations": [{"type": "add", "line": 1, "content": "first"}]})
    client.tool_call("fileio_patch_file", {"path": str(path), "patch": patch, "format": "add_remove_lines"})
    _assert(path.read_text() == "first", f"Unexpected add_remove_lines patched content: {path.read_text()!r}")


def test_fileio_patch_file_add_remove_lines_invalid_line_beyond_end_errors(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_patch_file_add_remove_invalid_line")
    path = case / "patch.txt"
    path.write_text("")
    patch = json.dumps({"operations": [{"type": "add", "line": 2, "content": "x"}]})
    _assert_raises(
        "Invalid line number",
        lambda: client.tool_call("fileio_patch_file", {"path": str(path), "patch": patch, "format": "add_remove_lines"}),
    )


def test_fileio_patch_file_add_remove_lines_negative_line_rejected(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_patch_file_add_remove_negative_line")
    path = case / "patch.txt"
    path.write_text("a\n")
    patch = '{"operations": [{"type": "remove", "line": -1}]}'
    _assert_raises(
        "numeric",
        lambda: client.tool_call("fileio_patch_file", {"path": str(path), "patch": patch, "format": "add_remove_lines"}),
    )


def test_fileio_copy(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_copy")
    src = case / "src.txt"
    src.write_text("copyme")
    dst = case / "dst"
    dst.mkdir()
    client.tool_call("fileio_copy", {"source": [str(src)], "destination": str(dst)})
    copied = dst / "src.txt"
    _assert(copied.exists(), "Expected copied file")
    _assert(copied.read_text() == "copyme", "Copied content mismatch")


def test_fileio_copy_dir_recursive(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_copy_dir_recursive")
    src_dir = case / "src"
    (src_dir / "nested").mkdir(parents=True)
    (src_dir / "nested" / "f.txt").write_text("x")
    dst_dir = case / "dst"
    client.tool_call("fileio_copy", {"source": [str(src_dir)], "destination": str(dst_dir), "recursive": True})
    _assert((dst_dir / "nested" / "f.txt").exists(), "Expected directory copied recursively")


def test_fileio_copy_dir_without_recursive_errors(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_copy_dir_without_recursive_errors")
    src_dir = case / "src"
    src_dir.mkdir()
    dst_dir = case / "dst"
    dst_dir.mkdir()
    res = client.tool_call("fileio_copy", {"source": [str(src_dir)], "destination": str(dst_dir), "recursive": False})
    val = _extract_value(res)
    _assert(isinstance(val, list) and val and "error" in str(val[0].get("status", "")).lower(), f"Expected per-source error, got: {val}")


def test_fileio_copy_glob(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_copy_glob")
    (case / "a.txt").write_text("a")
    (case / "b.txt").write_text("b")
    (case / "c.log").write_text("c")
    dst = case / "dst"
    dst.mkdir()
    res = client.tool_call("fileio_copy", {"source": [str(case / "*.txt")], "destination": str(dst)})
    val = _extract_value(res)
    _assert(isinstance(val, list) and all(r.get("status") == "ok" for r in val), f"Unexpected copy results: {val}")
    _assert((dst / "a.txt").exists() and (dst / "b.txt").exists(), "Expected txt files copied")
    _assert(not (dst / "c.log").exists(), "Did not expect log file copied")


def test_fileio_copy_glob_no_match_errors(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_copy_glob_no_match_errors")
    dst = case / "dst"
    dst.mkdir()
    _assert_raises(
        "No files match pattern",
        lambda: client.tool_call("fileio_copy", {"source": [str(case / "*.nope")], "destination": str(dst)}),
    )


def test_fileio_move(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_move")
    src = case / "src.txt"
    src.write_text("moveme")
    dst = case / "moved.txt"
    if dst.exists():
        dst.unlink()
    client.tool_call("fileio_move", {"source": [str(src)], "destination": str(dst)})
    _assert(not src.exists(), "Expected source removed")
    _assert(dst.exists() and dst.read_text() == "moveme", "Move result mismatch")


def test_fileio_move_glob(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_move_glob")
    (case / "a.txt").write_text("a")
    (case / "b.txt").write_text("b")
    (case / "c.log").write_text("c")
    dst = case / "dst"
    dst.mkdir()
    res = client.tool_call("fileio_move", {"source": [str(case / "*.txt")], "destination": str(dst)})
    val = _extract_value(res)
    _assert(isinstance(val, list) and all(r.get("status") == "ok" for r in val), f"Unexpected move results: {val}")
    _assert(not (case / "a.txt").exists() and not (case / "b.txt").exists(), "Expected txt files moved")
    _assert((dst / "a.txt").exists() and (dst / "b.txt").exists(), "Expected moved files in dst")
    _assert((case / "c.log").exists(), "Did not expect c.log moved")


def test_fileio_move_glob_no_match_errors(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_move_glob_no_match_errors")
    dst = case / "dst"
    dst.mkdir()
    _assert_raises(
        "No files match pattern",
        lambda: client.tool_call("fileio_move", {"source": [str(case / "*.nope")], "destination": str(dst)}),
    )


def test_fileio_remove(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_remove")
    path = case / "rm.txt"
    path.write_text("x")
    client.tool_call("fileio_remove", {"path": [str(path)], "force": False})
    _assert(not path.exists(), "Expected file removed")


def test_fileio_remove_recursive_dir(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_remove_recursive_dir")
    d = case / "d"
    (d / "nested").mkdir(parents=True)
    (d / "nested" / "f.txt").write_text("x")
    res = client.tool_call("fileio_remove", {"path": [str(d)], "recursive": True, "force": False})
    val = _extract_value(res)
    _assert(isinstance(val, list) and val and val[0].get("status") == "ok", f"Unexpected remove results: {val}")
    _assert(not d.exists(), "Expected directory removed recursively")


def test_fileio_remove_glob(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_remove_glob")
    (case / "a.tmp").write_text("x")
    (case / "b.tmp").write_text("x")
    (case / "c.log").write_text("x")
    res = client.tool_call("fileio_remove", {"path": [str(case / "*.tmp")], "force": False})
    val = _extract_value(res)
    _assert(isinstance(val, list) and all(r.get("status") == "ok" for r in val), f"Unexpected rm glob results: {val}")
    _assert(not (case / "a.tmp").exists() and not (case / "b.tmp").exists(), "Expected tmp files removed")
    _assert((case / "c.log").exists(), "Did not expect log removed")


def test_fileio_remove_glob_no_match_errors(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_remove_glob_no_match_errors")
    _assert_raises(
        "No files match pattern",
        lambda: client.tool_call("fileio_remove", {"path": [str(case / "*.nope")], "force": False}),
    )


def test_fileio_remove_force_missing_ok(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_remove_force_missing_ok")
    missing = case / "missing.txt"
    if missing.exists():
        missing.unlink()
    # Should not error.
    client.tool_call("fileio_remove", {"path": [str(missing)], "force": True})


def test_fileio_remove_directory(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_remove_directory")
    d = case / "dir"
    (d / "nested").mkdir(parents=True)
    (d / "nested" / "f.txt").write_text("x")
    client.tool_call("fileio_remove_directory", {"path": [str(d)], "recursive": True})
    _assert(not d.exists(), "Expected directory removed")


def test_fileio_remove_directory_non_recursive_reports_error(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_remove_directory_non_recursive_reports_error")
    d = case / "dir"
    d.mkdir()
    (d / "f.txt").write_text("x")
    res = client.tool_call("fileio_remove_directory", {"path": [str(d)], "recursive": False})
    val = _extract_value(res)
    _assert(isinstance(val, list) and val, f"Unexpected rmdir result: {val}")
    status = str(val[0].get("status", ""))
    _assert("Directory is not empty" in status or "not empty" in status.lower(), f"Expected not-empty error status, got: {status}")
    _assert(d.exists(), "Expected directory to still exist")


def test_fileio_create_hard_link(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_create_hard_link")
    target = case / "target.txt"
    target.write_text("x")
    link = case / "hard.txt"
    if link.exists():
        link.unlink()
    client.tool_call("fileio_create_hard_link", {"target": str(target), "link_path": str(link)})
    _assert(link.exists(), "Expected hard link")
    _assert(os.stat(target).st_ino == os.stat(link).st_ino, "Expected same inode")


def test_fileio_create_hard_link_missing_target_errors(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_create_hard_link_missing_target_errors")
    target = case / "missing.txt"
    if target.exists():
        target.unlink()
    link = case / "hard.txt"
    if link.exists():
        link.unlink()
    _assert_raises(
        "not found",
        lambda: client.tool_call("fileio_create_hard_link", {"target": str(target), "link_path": str(link)}),
    )


def test_fileio_create_symbolic_link(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_create_symbolic_link")
    target = case / "target.txt"
    target.write_text("x")
    link = case / "sym.txt"
    if link.exists() or link.is_symlink():
        link.unlink()
    client.tool_call("fileio_create_symbolic_link", {"target": str(target), "link_path": str(link)})
    _assert(link.is_symlink(), "Expected symlink")
    _assert(os.readlink(link) == str(target), "Symlink target mismatch")


def test_fileio_create_symbolic_link_broken_target_ok(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_create_symbolic_link_broken_target_ok")
    missing_target = case / "missing.txt"
    if missing_target.exists():
        missing_target.unlink()
    link = case / "broken.txt"
    if link.exists() or link.is_symlink():
        link.unlink()
    client.tool_call("fileio_create_symbolic_link", {"target": str(missing_target), "link_path": str(link)})
    _assert(link.is_symlink(), "Expected symlink created even if target missing")
    _assert(os.readlink(link) == str(missing_target), "Broken symlink target mismatch")


def test_fileio_read_symbolic_link_ok(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_read_symbolic_link_ok")
    target = case / "t.txt"
    target.write_text("x")
    link = case / "l.txt"
    os.symlink(str(target), str(link))
    res = client.tool_call("fileio_read_symbolic_link", {"path": str(link)})
    val = _extract_value(res)
    _assert(str(val) == str(target), f"Unexpected readlink: {val}")


def test_fileio_read_symbolic_link_relative_target(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_read_symbolic_link_relative_target")
    (case / "target.txt").write_text("x")
    link = case / "link.txt"
    # precondition: create relative symlink
    os.symlink("target.txt", str(link))
    res = client.tool_call("fileio_read_symbolic_link", {"path": str(link)})
    val = _extract_value(res)
    _assert(str(val) == "target.txt", f"Expected relative target.txt, got: {val}")


def test_fileio_read_symbolic_link_non_symlink_errors(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_read_symbolic_link_non_symlink")
    f = case / "file.txt"
    f.write_text("x")
    _assert_raises("not a symbolic link", lambda: client.tool_call("fileio_read_symbolic_link", {"path": str(f)}))


def test_fileio_get_basename(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_get_basename")
    p = case / "a" / "b" / "c.txt"
    val = _extract_value(client.tool_call("fileio_get_basename", {"path": str(p)}))
    _assert(val == "c.txt", f"Unexpected basename: {val}")


def test_fileio_get_basename_trailing_slash(client: McpStdioClient) -> None:
    # Use a stable path shape; basename should be last component even with trailing slash.
    val = _extract_value(client.tool_call("fileio_get_basename", {"path": "/usr/bin/"}))
    _assert(val == "bin", f"Unexpected basename for /usr/bin/: {val}")


def test_fileio_get_dirname(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_get_dirname")
    p = case / "a" / "b" / "c.txt"
    val = _extract_value(client.tool_call("fileio_get_dirname", {"path": str(p)}))
    _assert(Path(val) == p.parent, f"Unexpected dirname: {val}")


def test_fileio_get_dirname_no_dir_component(client: McpStdioClient) -> None:
    val = _extract_value(client.tool_call("fileio_get_dirname", {"path": "file.txt"}))
    _assert(val == "", f"Expected empty dirname for file.txt, got: {val}")


def test_fileio_get_canonical_path_ok(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_get_canonical_path_ok")
    p = case / "x.txt"
    p.write_text("x")
    val = _extract_value(client.tool_call("fileio_get_canonical_path", {"path": str(p)}))
    _assert(Path(val) == p.resolve(), f"Unexpected canonical: {val}")


def test_fileio_get_canonical_path_missing_errors(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_get_canonical_path_missing")
    p = case / "missing.txt"
    if p.exists():
        p.unlink()
    _assert_raises("not found", lambda: client.tool_call("fileio_get_canonical_path", {"path": str(p)}))


def test_fileio_get_current_directory(client: McpStdioClient) -> None:
    val = _extract_value(client.tool_call("fileio_get_current_directory", {}))
    _assert(Path(val) == WORKSPACE_DIR, f"Expected cwd {WORKSPACE_DIR}, got: {val}")


def test_fileio_create_temporary_file(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_create_temporary_file")
    val = _extract_value(client.tool_call("fileio_create_temporary", {"type": "file", "template": str(case)}))
    _assert(Path(val).exists() and Path(val).is_file(), f"Expected temp file, got: {val}")


def test_fileio_create_temporary_file_no_template(client: McpStdioClient) -> None:
    val = _extract_value(client.tool_call("fileio_create_temporary", {"type": "file"}))
    _assert(Path(val).exists() and Path(val).is_file(), f"Expected temp file, got: {val}")


def test_fileio_create_temporary_dir(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_create_temporary_dir")
    val = _extract_value(client.tool_call("fileio_create_temporary", {"type": "dir", "template": str(case)}))
    _assert(Path(val).exists() and Path(val).is_dir(), f"Expected temp dir, got: {val}")


def test_fileio_count_lines_ok(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_count_lines_ok")
    p = case / "c.txt"
    p.write_text("a\nb\n")
    val = _extract_value(client.tool_call("fileio_count_lines", {"path": [str(p)]}))
    _assert(isinstance(val, list) and int(val[0].get("lines")) == 2, f"Unexpected count_lines: {val}")


def test_fileio_count_lines_string_path_errors(client: McpStdioClient) -> None:
    # The schema suggests `path` can be a string OR array, but the current implementation
    # requires an array of strings.
    case = _new_case_dir("fileio_count_lines_string_path_errors")
    p = case / "c.txt"
    p.write_text("a\n")
    _assert_raises(
        "Path must be an array of strings",
        lambda: client.tool_call("fileio_count_lines", {"path": str(p)}),
    )


def test_fileio_count_lines_missing_status(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_count_lines_missing")
    p = case / "missing.txt"
    if p.exists():
        p.unlink()
    val = _extract_value(client.tool_call("fileio_count_lines", {"path": [str(p)]}))
    _assert(isinstance(val, list) and val and val[0].get("exists") is False, f"Unexpected count_lines missing: {val}")


def test_fileio_count_words_ok(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_count_words_ok")
    p = case / "w.txt"
    p.write_text("hello world\nfoo")
    val = _extract_value(client.tool_call("fileio_count_words", {"path": [str(p)]}))
    _assert(isinstance(val, list) and int(val[0].get("words")) == 3, f"Unexpected count_words: {val}")


def test_fileio_count_words_missing_status(client: McpStdioClient) -> None:
    case = _new_case_dir("fileio_count_words_missing")
    p = case / "missing.txt"
    if p.exists():
        p.unlink()
    val = _extract_value(client.tool_call("fileio_count_words", {"path": [str(p)]}))
    _assert(isinstance(val, list) and val and val[0].get("exists") is False, f"Unexpected count_words missing: {val}")


def test_fileio_change_ownership_skipped_unless_enabled(client: McpStdioClient) -> None:
    # Intentionally gated (RUN_DANGEROUS=1, or root-in-Docker auto-enable).
    case = _new_case_dir("fileio_change_ownership")
    p = case / "owned.txt"
    p.write_text("x")
    # Choose current uid/gid so it doesn't require privilege.
    uid = str(os.getuid())
    gid = str(os.getgid())
    client.tool_call("fileio_change_ownership", {"path": [str(p)], "user": uid, "group": gid})
    st = p.stat()
    _assert(str(st.st_uid) == uid and str(st.st_gid) == gid, "Ownership did not match expected")


def _running_in_docker() -> bool:
    # Heuristics: /.dockerenv is common; also check cgroup markers.
    try:
        if Path("/.dockerenv").exists():
            return True
    except Exception:
        pass

    # Podman commonly creates this file.
    try:
        if Path("/run/.containerenv").exists():
            return True
    except Exception:
        pass

    # Some runtimes set an env var like: container=docker|podman
    try:
        if os.environ.get("container"):
            return True
    except Exception:
        pass

    try:
        cgroup = Path("/proc/1/cgroup")
        if cgroup.exists():
            text = cgroup.read_text(errors="ignore")
            markers = ("docker", "kubepods", "containerd", "podman", "libpod")
            return any(m in text for m in markers)
    except Exception:
        pass

    return False


def _run_dangerous_enabled() -> bool:
    if os.environ.get("RUN_DANGEROUS") == "1":
        return True

    # Safe default: auto-enable only in containers (Docker/Podman).
    # Outside containers, require explicit opt-in.
    return _running_in_docker()


def main() -> int:
    print(f"{BOLD}{'='*60}")
    print("FILEIO MCP TOOLS TEST SUITE (one tool per test)")
    print(f"{'='*60}{RESET}")

    print(f"Repo root: {ROOT_DIR}")
    global TEST_ROOT
    TEST_ROOT = _create_test_root()
    print(f"Test directory: {TEST_ROOT}")

    server_cmd = _pick_server_cmd()
    print(f"Server command: {' '.join(server_cmd)}")

    tests: List[TestCase] = [
        TestCase("fileio_write_file_overwrite", "fileio_write_file", test_fileio_write_file_overwrite),
        TestCase("fileio_write_file_append", "fileio_write_file", test_fileio_write_file_append),
        TestCase("fileio_read_lines_ok", "fileio_read_lines", test_fileio_read_lines_ok),
        TestCase("fileio_read_lines_range", "fileio_read_lines", test_fileio_read_lines_range),
        TestCase("fileio_read_lines_line_count", "fileio_read_lines", test_fileio_read_lines_line_count),
        TestCase("fileio_read_lines_start_offset", "fileio_read_lines", test_fileio_read_lines_start_offset),
        TestCase("fileio_read_lines_empty_file_returns_empty", "fileio_read_lines", test_fileio_read_lines_empty_file_returns_empty),
        TestCase("fileio_read_lines_end_past_eof_clamps", "fileio_read_lines", test_fileio_read_lines_end_past_eof_clamps),
        TestCase("fileio_read_lines_start_line_beyond_eof_errors", "fileio_read_lines", test_fileio_read_lines_start_line_beyond_eof_errors),
        TestCase("fileio_read_lines_end_before_start_errors", "fileio_read_lines", test_fileio_read_lines_end_before_start_errors),
        TestCase("fileio_read_lines_negative_numbers_rejected", "fileio_read_lines", test_fileio_read_lines_negative_numbers_rejected),
        TestCase("fileio_read_lines_missing_errors", "fileio_read_lines", test_fileio_read_lines_missing_errors),
        TestCase("fileio_set_permissions", "fileio_set_permissions", test_fileio_set_permissions),
        TestCase("fileio_set_permissions_multiple_paths", "fileio_set_permissions", test_fileio_set_permissions_multiple_paths),
        TestCase("fileio_set_mode_alias", "fileio_set_mode", test_fileio_set_mode_alias),
        TestCase("fileio_get_permissions", "fileio_get_permissions", test_fileio_get_permissions),
        TestCase("fileio_get_permissions_multiple_paths", "fileio_get_permissions", test_fileio_get_permissions_multiple_paths),
        TestCase("fileio_touch", "fileio_touch", test_fileio_touch),
        TestCase("fileio_stat", "fileio_stat", test_fileio_stat),
        TestCase("fileio_make_directory", "fileio_make_directory", test_fileio_make_directory),
        TestCase("fileio_make_directory_non_recursive_errors", "fileio_make_directory", test_fileio_make_directory_non_recursive_errors),
        TestCase("fileio_list_directory", "fileio_list_directory", test_fileio_list_directory),
        TestCase("fileio_list_directory_recursive", "fileio_list_directory", test_fileio_list_directory_recursive),
        TestCase("fileio_list_directory_include_hidden", "fileio_list_directory", test_fileio_list_directory_include_hidden),
        TestCase("fileio_list_directory_missing_returns_empty", "fileio_list_directory", test_fileio_list_directory_missing_returns_empty),
        TestCase("fileio_find_files", "fileio_find_files", test_fileio_find_files),
        TestCase("fileio_find_files_file_type_dir", "fileio_find_files", test_fileio_find_files_file_type_dir),
        TestCase("fileio_find_files_missing_root_errors", "fileio_find_files", test_fileio_find_files_missing_root_errors),
        TestCase("fileio_find_in_files", "fileio_find_in_files", test_fileio_find_in_files),
        TestCase("fileio_find_in_files_case_insensitive", "fileio_find_in_files", test_fileio_find_in_files_case_insensitive),
        TestCase("fileio_find_in_files_whole_word", "fileio_find_in_files", test_fileio_find_in_files_whole_word),
        TestCase("fileio_patch_file_add_remove", "fileio_patch_file", test_fileio_patch_file_add_remove),
        TestCase("fileio_patch_file_unified_diff", "fileio_patch_file", test_fileio_patch_file_unified_diff),
        TestCase("fileio_patch_file_unified_diff_empty_file_add_line", "fileio_patch_file", test_fileio_patch_file_unified_diff_empty_file_add_line),
        TestCase("fileio_patch_file_unified_diff_add_at_end", "fileio_patch_file", test_fileio_patch_file_unified_diff_add_at_end),
        TestCase("fileio_patch_file_add_remove_lines_empty_file_add_first_line", "fileio_patch_file", test_fileio_patch_file_add_remove_lines_empty_file_add_first_line),
        TestCase("fileio_patch_file_add_remove_lines_invalid_line_beyond_end_errors", "fileio_patch_file", test_fileio_patch_file_add_remove_lines_invalid_line_beyond_end_errors),
        TestCase("fileio_patch_file_add_remove_lines_negative_line_rejected", "fileio_patch_file", test_fileio_patch_file_add_remove_lines_negative_line_rejected),
        TestCase("fileio_copy", "fileio_copy", test_fileio_copy),
        TestCase("fileio_copy_dir_recursive", "fileio_copy", test_fileio_copy_dir_recursive),
        TestCase("fileio_copy_dir_without_recursive_errors", "fileio_copy", test_fileio_copy_dir_without_recursive_errors),
        TestCase("fileio_copy_glob", "fileio_copy", test_fileio_copy_glob),
        TestCase("fileio_copy_glob_no_match_errors", "fileio_copy", test_fileio_copy_glob_no_match_errors),
        TestCase("fileio_move", "fileio_move", test_fileio_move),
        TestCase("fileio_move_glob", "fileio_move", test_fileio_move_glob),
        TestCase("fileio_move_glob_no_match_errors", "fileio_move", test_fileio_move_glob_no_match_errors),
        TestCase("fileio_remove", "fileio_remove", test_fileio_remove),
        TestCase("fileio_remove_recursive_dir", "fileio_remove", test_fileio_remove_recursive_dir),
        TestCase("fileio_remove_glob", "fileio_remove", test_fileio_remove_glob),
        TestCase("fileio_remove_glob_no_match_errors", "fileio_remove", test_fileio_remove_glob_no_match_errors),
        TestCase("fileio_remove_force_missing_ok", "fileio_remove", test_fileio_remove_force_missing_ok),
        TestCase("fileio_remove_directory", "fileio_remove_directory", test_fileio_remove_directory),
        TestCase("fileio_remove_directory_non_recursive_reports_error", "fileio_remove_directory", test_fileio_remove_directory_non_recursive_reports_error),
        TestCase("fileio_create_hard_link", "fileio_create_hard_link", test_fileio_create_hard_link),
        TestCase("fileio_create_hard_link_missing_target_errors", "fileio_create_hard_link", test_fileio_create_hard_link_missing_target_errors),
        TestCase("fileio_create_symbolic_link", "fileio_create_symbolic_link", test_fileio_create_symbolic_link),
        TestCase("fileio_create_symbolic_link_broken_target_ok", "fileio_create_symbolic_link", test_fileio_create_symbolic_link_broken_target_ok),
        TestCase("fileio_read_symbolic_link_ok", "fileio_read_symbolic_link", test_fileio_read_symbolic_link_ok),
        TestCase("fileio_read_symbolic_link_relative_target", "fileio_read_symbolic_link", test_fileio_read_symbolic_link_relative_target),
        TestCase("fileio_read_symbolic_link_non_symlink_errors", "fileio_read_symbolic_link", test_fileio_read_symbolic_link_non_symlink_errors),
        TestCase("fileio_get_basename", "fileio_get_basename", test_fileio_get_basename),
        TestCase("fileio_get_basename_trailing_slash", "fileio_get_basename", test_fileio_get_basename_trailing_slash),
        TestCase("fileio_get_dirname", "fileio_get_dirname", test_fileio_get_dirname),
        TestCase("fileio_get_dirname_no_dir_component", "fileio_get_dirname", test_fileio_get_dirname_no_dir_component),
        TestCase("fileio_get_canonical_path_ok", "fileio_get_canonical_path", test_fileio_get_canonical_path_ok),
        TestCase("fileio_get_canonical_path_missing_errors", "fileio_get_canonical_path", test_fileio_get_canonical_path_missing_errors),
        TestCase("fileio_get_current_directory", "fileio_get_current_directory", test_fileio_get_current_directory),
        TestCase("fileio_create_temporary_file", "fileio_create_temporary", test_fileio_create_temporary_file),
        TestCase("fileio_create_temporary_file_no_template", "fileio_create_temporary", test_fileio_create_temporary_file_no_template),
        TestCase("fileio_create_temporary_dir", "fileio_create_temporary", test_fileio_create_temporary_dir),
        TestCase("fileio_count_lines_ok", "fileio_count_lines", test_fileio_count_lines_ok),
        TestCase("fileio_count_lines_string_path_errors", "fileio_count_lines", test_fileio_count_lines_string_path_errors),
        TestCase("fileio_count_lines_missing_status", "fileio_count_lines", test_fileio_count_lines_missing_status),
        TestCase("fileio_count_words_ok", "fileio_count_words", test_fileio_count_words_ok),
        TestCase("fileio_count_words_missing_status", "fileio_count_words", test_fileio_count_words_missing_status),
        TestCase(
            "fileio_change_ownership",
            "fileio_change_ownership",
            test_fileio_change_ownership_skipped_unless_enabled,
            dangerous=True,
        ),
    ]

    results: List[Tuple[str, str]] = []  # (name, PASS|FAIL|SKIP)
    keep = os.environ.get("KEEP_TEST_DIR") == "1"
    exit_code = 1

    try:
        with McpStdioClient(server_cmd, cwd=WORKSPACE_DIR) as client:
            client.initialize()

            for tc in tests:
                print(f"\n{BOLD}Test: {tc.name} ({tc.tool}){RESET}")
                if tc.dangerous and not _run_dangerous_enabled():
                    print(
                        f"{YELLOW}SKIP (set RUN_DANGEROUS=1, or run as root in Docker to auto-enable){RESET}"
                    )
                    results.append((tc.name, "SKIP"))
                    continue

                try:
                    tc.fn(client)
                    print(f"{GREEN}✓ PASS{RESET}")
                    results.append((tc.name, "PASS"))
                except Exception as e:
                    print(f"{RED}✗ FAIL: {e}{RESET}")
                    results.append((tc.name, "FAIL"))

        print(f"\n{BOLD}{'='*60}")
        print("TEST SUMMARY")
        print(f"{'='*60}{RESET}")

        passed = sum(1 for _, s in results if s == "PASS")
        failed = sum(1 for _, s in results if s == "FAIL")
        skipped = sum(1 for _, s in results if s == "SKIP")

        for name, status in results:
            if status == "PASS":
                label = f"{GREEN}PASS{RESET}"
            elif status == "SKIP":
                label = f"{YELLOW}SKIP{RESET}"
            else:
                label = f"{RED}FAIL{RESET}"
            print(f"{label} - {name}")

        total = len(results)
        print(
            f"\n{BOLD}Total: {passed} passed, {failed} failed, {skipped} skipped (of {total}){RESET}"
        )

        exit_code = 0 if failed == 0 else 1
    finally:
        # Always attempt cleanup, even if an exception occurs mid-run.
        if TEST_ROOT is not None:
            if keep:
                print(f"{YELLOW}Keeping test directory (KEEP_TEST_DIR=1): {TEST_ROOT}{RESET}")
            else:
                shutil.rmtree(TEST_ROOT, ignore_errors=True)

    return exit_code


if __name__ == "__main__":
    sys.exit(main())
