#![deny(warnings)]

//! Telemetry acceptance tests (mcp-core#40).
//!
//! Each test spawns the real `fileio-mcp` binary over stdio, drives it
//! through a fixed JSON-RPC sequence, and inspects everything the process
//! wrote to stdout and stderr. This is deliberately end to end: the level
//! contract (D10) is enforced across mcp-core's dispatch layer and this
//! server's own code together, and only a real process boundary proves
//! neither side leaked a path.

use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use tempfile::TempDir;

/// Spawn `fileio-mcp serve --mode stdio` with `extra_args` and `env`, drive
/// it through an `initialize` / `initialized` handshake followed by
/// `requests` (each assigned a fresh id), then close stdin.
///
/// Closing stdin is a clean EOF for the stdio transport's pump loop (see
/// `mcp-core`'s `runner::pump`), so the server exits on its own once it has
/// drained whatever was already written to the pipe — no explicit
/// `shutdown` call or timeout is needed. Returns everything the process
/// wrote to stdout and to stderr.
fn run_and_capture(
    extra_args: &[&str],
    env: &[(&str, &str)],
    requests: &[Value],
) -> (String, String) {
    let exe = env!("CARGO_BIN_EXE_fileio-mcp");
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut cmd = Command::new(exe);
    cmd.args(["serve", "--mode", "stdio"])
        .args(extra_args)
        .current_dir(repo_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        cmd.env(key, value);
    }

    let mut child: Child = cmd.spawn().expect("spawn fileio-mcp serve --mode stdio");
    let mut stdin = child.stdin.take().expect("child stdin");
    let mut stdout = child.stdout.take().expect("child stdout");
    let mut stderr = child.stderr.take().expect("child stderr");

    // Drain both pipes concurrently from the moment the child starts, so a
    // chatty RUST_LOG=trace run can never fill a pipe buffer and deadlock
    // against us still writing requests.
    let stdout_reader = std::thread::spawn(move || {
        let mut buf = String::new();
        stdout.read_to_string(&mut buf).expect("read child stdout");
        buf
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = String::new();
        stderr.read_to_string(&mut buf).expect("read child stderr");
        buf
    });

    write_line(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"protocolVersion": "2025-11-25", "capabilities": {}},
        }),
    );
    write_line(
        &mut stdin,
        &json!({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}),
    );

    for (offset, request) in requests.iter().enumerate() {
        let mut request = request.clone();
        request["jsonrpc"] = json!("2.0");
        request["id"] = json!(2u64 + offset as u64);
        write_line(&mut stdin, &request);
    }
    drop(stdin);

    let stdout_captured = stdout_reader.join().expect("stdout reader thread");
    let stderr_captured = stderr_reader.join().expect("stderr reader thread");
    let status = child.wait().expect("wait for child");
    assert!(
        status.success(),
        "server must exit cleanly on stdin EOF, got: {status:?}\nstderr:\n{stderr_captured}"
    );

    (stdout_captured, stderr_captured)
}

fn write_line(stdin: &mut ChildStdin, value: &Value) {
    let line = serde_json::to_string(value).expect("serialize jsonrpc");
    stdin
        .write_all(line.as_bytes())
        .expect("write jsonrpc line");
    stdin.write_all(b"\n").expect("write newline");
    stdin.flush().expect("flush stdin");
}

/// Acceptance (mcp-core#40): at the noisiest log level, stdout still carries
/// only JSON-RPC. The stdio transport frames the protocol on stdout (D1); a
/// stray log line there corrupts the stream for every client, including
/// ones that will never turn RUST_LOG up, so the property has to hold at the
/// level most likely to leak, not just the quiet default.
#[test]
fn stdout_carries_only_json_rpc_at_trace_log_level() {
    let temp = TempDir::new().expect("create temp dir");
    let file = temp.path().join("sample.txt");
    std::fs::write(&file, "a\nb\n").expect("write sample file");

    let requests = [
        json!({"method": "tools/list", "params": {}}),
        json!({
            "method": "tools/call",
            "params": {
                "name": "fileio_read_lines",
                "arguments": {"path": file.to_string_lossy()},
            },
        }),
    ];
    let (stdout, stderr) = run_and_capture(&[], &[("RUST_LOG", "trace")], &requests);

    let mut lines_seen = 0usize;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        lines_seen += 1;
        let value: Value = serde_json::from_str(trimmed).unwrap_or_else(|e| {
            panic!("stdout line is not valid JSON at RUST_LOG=trace: {e}\nline: {trimmed}")
        });
        assert_eq!(
            value.get("jsonrpc").and_then(Value::as_str),
            Some("2.0"),
            "every stdout line must be a JSON-RPC 2.0 message, got: {trimmed}"
        );
    }
    assert!(
        lines_seen >= 3,
        "expected at least 3 JSON-RPC replies on stdout (init, tools/list, tools/call), \
         saw {lines_seen}.\nstderr was:\n{stderr}"
    );
}

/// Acceptance (mcp-core#40, epic D10): `tool_call_records_no_arguments`.
/// Neither a real path nor a denied path reaches a span field or an INFO
/// line, including the guard-denial path — a rejection is where a path
/// most naturally gets logged, since making the denial itself invisible is
/// this server's whole design (see `path_guard`'s module doc).
///
/// The RUST_LOG=trace run is a positive control: it proves the sentinels
/// really do surface somewhere in this harness's capture (mcp-core logs
/// tool arguments at DEBUG), so the RUST_LOG=info absence assertions above
/// are not passing because nothing was ever captured.
#[test]
fn tool_call_records_no_arguments() {
    let temp = TempDir::new().expect("create temp dir");
    let normal_dir = temp.path().join("SENTINEL-NORMAL-8f2c1");
    std::fs::create_dir_all(&normal_dir).expect("create normal dir");
    let normal_file = normal_dir.join("visible.txt");
    std::fs::write(&normal_file, "hello\n").expect("write normal file");
    let normal_path = normal_file.to_string_lossy().to_string();

    // Denied via an extra --block-path rather than a real HOME-relative
    // default entry, so the denial is deterministic regardless of the
    // environment the test runs in.
    let denied_path = "/home/sentinel/SECRET-FILE-NAME";

    let requests = [
        json!({
            "method": "tools/call",
            "params": {"name": "fileio_read_lines", "arguments": {"path": normal_path}},
        }),
        json!({
            "method": "tools/call",
            "params": {"name": "fileio_read_lines", "arguments": {"path": denied_path}},
        }),
    ];

    let (_stdout, info_stderr) = run_and_capture(
        &["--block-path", "/home/sentinel/"],
        &[("RUST_LOG", "info")],
        &requests,
    );
    assert!(
        !info_stderr.contains("SENTINEL-NORMAL"),
        "a real path reached stderr at RUST_LOG=info:\n{info_stderr}"
    );
    assert!(
        !info_stderr.contains("SECRET-FILE-NAME"),
        "a denied path reached stderr at RUST_LOG=info:\n{info_stderr}"
    );

    let (_stdout2, trace_stderr) = run_and_capture(
        &["--block-path", "/home/sentinel/"],
        &[("RUST_LOG", "trace")],
        &requests,
    );
    assert!(
        trace_stderr.contains("SENTINEL-NORMAL"),
        "expected the real path to surface at RUST_LOG=trace as a positive \
         control (tool arguments are DEBUG-level content per D10); \
         got none, so the RUST_LOG=info assertion above is not meaningful:\n{trace_stderr}"
    );
    assert!(
        trace_stderr.contains("SECRET-FILE-NAME"),
        "expected the denied path to surface at RUST_LOG=trace as a positive \
         control; got none, so the RUST_LOG=info assertion above is not \
         meaningful:\n{trace_stderr}"
    );
}

/// Acceptance (mcp-core#40 checklist, `path_guard.rs`): an unreadable
/// `--block-file` warns with the reason and the outcome, never the path.
/// The path is a sentinel-bearing name specifically so a leak is easy to
/// spot in the assertion failure message.
#[test]
fn block_file_read_failure_warns_without_path() {
    let temp = TempDir::new().expect("create temp dir");
    let missing_blocklist = temp.path().join("SENTINEL-BLOCKLIST-missing.txt");
    // Deliberately not created: the server must fail to read it at startup.

    let requests: [Value; 0] = [];
    let (_stdout, stderr) = run_and_capture(
        &[
            "--block-file",
            missing_blocklist.to_str().expect("utf8 path"),
        ],
        &[("RUST_LOG", "info")],
        &requests,
    );

    assert!(
        stderr.contains("WARN"),
        "expected a WARN line for the unreadable block-file, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("SENTINEL-BLOCKLIST"),
        "the block-file path must not reach the log:\n{stderr}"
    );
}
