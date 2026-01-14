# FileIO-MCP Result Shapes

This document describes the structured JSON shapes returned by MCP tools for multi-path operations and counters.

- LineCountResult: { path: string, status: string, lines: number | null, exists: boolean }
- WordCountResult: { path: string, status: string, words: number | null, exists: boolean }
- OpResult: { path: string, status: string, exists: boolean }
- FileStat: existing structure returned by `fileio_stat`; includes `exists: bool` and `entry_type` that may be "file", "dir", "symlink", or "not_found".

Examples:

- `fileio_count_lines` returns:
  {
    "content": [{ "type": "json", "value": [ {"path":"/tmp/a.txt","status":"ok","lines":10,"exists":true} ] }]
  }

- `fileio_remove` returns per-path OpResult array:
  {
    "content": [{ "type": "json", "value": [ {"path":"/tmp/x","status":"ok","exists":true} ] }]
  }

These shapes allow callers to inspect per-path status without failing the whole operation for "negative" results such as not-found.

More examples

- `fileio_count_words`:
  {
    "content": [{ "type": "json", "value": [ {"path":"/tmp/a.txt","status":"ok","words":42,"exists":true} ] }]
  }

- `fileio_stat` (multiple paths):
  {
    "content": [{ "type": "json", "value": [
      {"path":"/tmp/a.txt","type":"file","size":1234,"is_file":true,"is_dir":false,"is_symlink":false,"exists":true},
      {"path":"/tmp/missing","type":"not_found","size":0,"exists":false}
    ] }]
  }

- `fileio_list_directory` (recursive=false):
  {
    "content": [{ "type": "json", "value": [
      {"name":"file1.txt","path":"/tmp/dir/file1.txt","type":"file","size":10},
      {"name":"subdir","path":"/tmp/dir/subdir","type":"directory"}
    ] }]
  }

- `fileio_copy` (multiple sources):
  {
    "content": [{ "type": "json", "value": [
      {"path":"/src/a.txt","status":"ok","exists":true},
      {"path":"/src/missing.txt","status":"error: NotFound","exists":false}
    ] }]
  }

- `fileio_move` (single result):
  {
    "content": [{ "type": "json", "value": [ {"path":"/src/a.txt","status":"ok","exists":true} ] }]
  }

- `fileio_remove` (force=true idempotent):
  {
    "content": [{ "type": "json", "value": [ {"path":"/tmp/old","status":"ok","exists":true} ] }]
  }

- `fileio_find_in_files`:
  {
    "content": [{ "type": "json", "value": [
      {"file_path":"/proj/src/lib.rs","line_number":10,"column_start":5,"column_end":12,"matched_text":"unsafe"}
    ] }]
  }

Notes:
- Each tool returns a `content` array to support mixed responses; tool clients should look for entries where `type` is `json` and read the `value` field directly. This `value` is already parsed JSON (not a string), so consumers can inspect arrays/objects without additional parsing.
- For backward compatibility, some tools may still emit `type: text` entries with human-readable messages; prefer `type: json` for programmatic consumption.
