#![deny(warnings)]

use nix::unistd::{getegid, geteuid};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use tempfile::TempDir;

struct McpStdioClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next_id: u64,
}

impl McpStdioClient {
    fn start() -> Self {
        let exe = env!("CARGO_BIN_EXE_fileio-mcp");
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));

        let mut child = Command::new(exe)
            .args(["serve", "--mode", "stdio"])
            .current_dir(repo_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn fileio-mcp serve --mode stdio");

        let stdin = child.stdin.take().expect("child stdin");
        let stdout = child.stdout.take().expect("child stdout");

        Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        }
    }

    fn send(&mut self, obj: &Value) {
        let s = serde_json::to_string(obj).expect("serialize jsonrpc");
        self.stdin
            .write_all(s.as_bytes())
            .and_then(|_| self.stdin.write_all(b"\n"))
            .and_then(|_| self.stdin.flush())
            .expect("write jsonrpc line");
    }

    fn read_msg(&mut self) -> Value {
        let mut line = String::new();
        loop {
            line.clear();
            let n = self.stdout.read_line(&mut line).expect("read line");
            if n == 0 {
                panic!("mcp server closed stdout");
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
                return v;
            }
        }
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;

        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));

        loop {
            let msg = self.read_msg();
            if msg.get("id").and_then(|v| v.as_u64()) != Some(id) {
                continue;
            }
            if let Some(err) = msg.get("error") {
                return Err(err.to_string());
            }
            return Ok(msg);
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(&json!({"jsonrpc":"2.0","method":method,"params":params}));
    }

    fn initialize(&mut self) {
        self.call(
            "initialize",
            json!({"protocolVersion":"2025-11-25","capabilities":{}}),
        )
        .expect("initialize");
        self.notify("initialized", json!({}));
    }

    fn tool_call(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        let resp = self.call("tools/call", json!({"name":name,"arguments":arguments}))?;
        resp.get("result")
            .cloned()
            .ok_or_else(|| format!("missing result field: {resp}"))
    }
}

impl Drop for McpStdioClient {
    fn drop(&mut self) {
        let _ = self.call("shutdown", json!({}));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn extract_value(tool_result: &Value) -> Value {
    let content = tool_result
        .get("content")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("expected result.content array, got: {tool_result}"));

    for entry in content {
        if entry.get("type") == Some(&Value::String("json".to_string())) {
            if let Some(v) = entry.get("value") {
                return v.clone();
            }
        }
    }

    for entry in content {
        if entry.get("type") == Some(&Value::String("text".to_string())) {
            if let Some(text) = entry.get("text").and_then(|v| v.as_str()) {
                let trimmed = text.trim();
                if trimmed.starts_with('{') || trimmed.starts_with('[') {
                    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
                        return v;
                    }
                }
                return Value::String(text.to_string());
            }
        }
    }

    panic!("no usable content entry in: {tool_result}");
}

fn running_in_container() -> bool {
    if Path::new("/.dockerenv").exists() {
        return true;
    }
    if Path::new("/run/.containerenv").exists() {
        return true;
    }
    if std::env::var_os("container").is_some() {
        return true;
    }

    if let Ok(cgroup) = fs::read_to_string("/proc/1/cgroup") {
        let markers = ["docker", "kubepods", "containerd", "podman", "libpod"];
        return markers.iter().any(|m| cgroup.contains(m));
    }

    false
}

fn dangerous_enabled() -> bool {
    if std::env::var("RUN_DANGEROUS").ok().as_deref() == Some("1") {
        return true;
    }
    running_in_container()
}

fn keep_test_dir_enabled() -> bool {
    std::env::var("KEEP_TEST_DIR").ok().as_deref() == Some("1")
}

fn expect_err_contains<T>(res: Result<T, String>, needle: &str) {
    match res {
        Ok(_) => panic!("expected error containing '{needle}', but call succeeded"),
        Err(e) => {
            let lower = e.to_lowercase();
            assert!(
                lower.contains(&needle.to_lowercase()),
                "expected error containing '{needle}', got: {e}"
            );
        }
    }
}

fn case_dir(root: &TempDir, name: &str) -> PathBuf {
    let mut safe = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            safe.push(ch);
        } else {
            safe.push('_');
        }
    }

    let dir = root.path().join(safe);
    fs::create_dir_all(&dir).expect("create case dir");
    dir
}

fn run_case(test_name: &str, f: impl FnOnce(&mut McpStdioClient, &TempDir)) {
    let test_root = TempDir::new().expect("create temp root");
    let mut client = McpStdioClient::start();
    client.initialize();
    f(&mut client, &test_root);

    if keep_test_dir_enabled() {
        let kept = test_root.keep();
        eprintln!("KEEP_TEST_DIR=1: kept test dir for {test_name}: {}", kept.display());
    }
}

// -----------------
// End-to-end MCP stdio parity suite
// -----------------

#[test]
fn fileio_write_file_overwrite() {
    run_case("fileio_write_file_overwrite", |client, root| {
        let case = case_dir(root, "fileio_write_file_overwrite");
        let path = case.join("nested/out.txt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();

        client
            .tool_call(
                "fileio_write_file",
                json!({"path": path.to_string_lossy(), "content":"hello\n", "append":false}),
            )
            .unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "hello\n");
    });
}

#[test]
fn fileio_write_file_append() {
    run_case("fileio_write_file_append", |client, root| {
        let case = case_dir(root, "fileio_write_file_append");
        let path = case.join("append.txt");
        fs::write(&path, "hello").unwrap();

        client
            .tool_call(
                "fileio_write_file",
                json!({"path": path.to_string_lossy(), "content":" world", "append":true}),
            )
            .unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "hello world");
    });
}

#[test]
fn fileio_read_lines_ok() {
    run_case("fileio_read_lines_ok", |client, root| {
        let case = case_dir(root, "fileio_read_lines_ok");
        let path = case.join("in.txt");
        fs::write(&path, "a\nb\nc\n").unwrap();

        let res = client
            .tool_call("fileio_read_lines", json!({"path": path.to_string_lossy()}))
            .unwrap();
        assert_eq!(extract_value(&res), json!(["a", "b", "c"]));
    });
}

#[test]
fn fileio_read_lines_range() {
    run_case("fileio_read_lines_range", |client, root| {
        let case = case_dir(root, "fileio_read_lines_range");
        let path = case.join("in.txt");
        fs::write(&path, "l1\nl2\nl3\nl4\n").unwrap();

        let res = client
            .tool_call(
                "fileio_read_lines",
                json!({"path": path.to_string_lossy(), "start_line": 2, "end_line": 3}),
            )
            .unwrap();
        assert_eq!(extract_value(&res), json!(["l2", "l3"]));
    });
}

#[test]
fn fileio_read_lines_line_count() {
    run_case("fileio_read_lines_line_count", |client, root| {
        let case = case_dir(root, "fileio_read_lines_line_count");
        let path = case.join("in.txt");
        fs::write(&path, "l1\nl2\nl3\nl4\n").unwrap();

        let res = client
            .tool_call(
                "fileio_read_lines",
                json!({"path": path.to_string_lossy(), "start_line": 2, "line_count": 2}),
            )
            .unwrap();
        assert_eq!(extract_value(&res), json!(["l2", "l3"]));
    });
}

#[test]
fn fileio_read_lines_start_offset() {
    run_case("fileio_read_lines_start_offset", |client, root| {
        let case = case_dir(root, "fileio_read_lines_start_offset");
        let path = case.join("in.txt");
        fs::write(&path, "l1\nl2\nl3\nl4\n").unwrap();

        let res = client
            .tool_call(
                "fileio_read_lines",
                json!({"path": path.to_string_lossy(), "start_offset": 1, "line_count": 2}),
            )
            .unwrap();
        assert_eq!(extract_value(&res), json!(["l2", "l3"]));
    });
}

#[test]
fn fileio_read_lines_empty_file_returns_empty() {
    run_case("fileio_read_lines_empty_file_returns_empty", |client, root| {
        let case = case_dir(root, "fileio_read_lines_empty_file_returns_empty");
        let path = case.join("empty.txt");
        fs::write(&path, "").unwrap();

        let res = client
            .tool_call("fileio_read_lines", json!({"path": path.to_string_lossy()}))
            .unwrap();
        assert_eq!(extract_value(&res), json!([]));
    });
}

#[test]
fn fileio_read_lines_end_past_eof_clamps_end_line() {
    run_case("fileio_read_lines_end_past_eof_clamps_end_line", |client, root| {
        let case = case_dir(root, "fileio_read_lines_end_past_eof_clamps_end_line");
        let path = case.join("in.txt");
        fs::write(&path, "a\nb\nc\n").unwrap();

        let res = client
            .tool_call(
                "fileio_read_lines",
                json!({"path": path.to_string_lossy(), "start_line": 2, "end_line": 999}),
            )
            .unwrap();
        assert_eq!(extract_value(&res), json!(["b", "c"]));
    });
}

#[test]
fn fileio_read_lines_end_past_eof_clamps_line_count() {
    run_case("fileio_read_lines_end_past_eof_clamps_line_count", |client, root| {
        let case = case_dir(root, "fileio_read_lines_end_past_eof_clamps_line_count");
        let path = case.join("in.txt");
        fs::write(&path, "a\nb\nc\n").unwrap();

        let res = client
            .tool_call(
                "fileio_read_lines",
                json!({"path": path.to_string_lossy(), "start_line": 2, "line_count": 999}),
            )
            .unwrap();
        assert_eq!(extract_value(&res), json!(["b", "c"]));
    });
}

#[test]
fn fileio_read_lines_start_line_beyond_eof_errors() {
    run_case("fileio_read_lines_start_line_beyond_eof_errors", |client, root| {
        let case = case_dir(root, "fileio_read_lines_start_line_beyond_eof_errors");
        let path = case.join("in.txt");
        fs::write(&path, "a\nb\n").unwrap();

        let res = client.tool_call(
            "fileio_read_lines",
            json!({"path": path.to_string_lossy(), "start_line": 5}),
        );
        expect_err_contains(res, "exceeds");
    });
}

#[test]
fn fileio_read_lines_end_before_start_errors() {
    run_case("fileio_read_lines_end_before_start_errors", |client, root| {
        let case = case_dir(root, "fileio_read_lines_end_before_start_errors");
        let path = case.join("in.txt");
        fs::write(&path, "a\nb\n").unwrap();

        let res = client.tool_call(
            "fileio_read_lines",
            json!({"path": path.to_string_lossy(), "start_line": 2, "end_line": 1}),
        );
        expect_err_contains(res, "end_line");
    });
}

#[test]
fn fileio_read_lines_negative_numbers_rejected() {
    run_case("fileio_read_lines_negative_numbers_rejected", |client, root| {
        let case = case_dir(root, "fileio_read_lines_negative_numbers_rejected");
        let path = case.join("in.txt");
        fs::write(&path, "a\n").unwrap();

        let res = client.tool_call(
            "fileio_read_lines",
            json!({"path": path.to_string_lossy(), "start_line": -1}),
        );
        expect_err_contains(res, "non-negative");
    });
}

#[test]
fn fileio_read_lines_missing_errors() {
    run_case("fileio_read_lines_missing_errors", |client, root| {
        let case = case_dir(root, "fileio_read_lines_missing_errors");
        let missing = case.join("missing.txt");

        let res = client.tool_call("fileio_read_lines", json!({"path": missing.to_string_lossy()}));
        expect_err_contains(res, "not found");
    });
}

#[test]
fn fileio_set_permissions() {
    run_case("fileio_set_permissions", |client, root| {
        let case = case_dir(root, "fileio_set_permissions");
        let path = case.join("perm.txt");
        fs::write(&path, "x").unwrap();

        client
            .tool_call(
                "fileio_set_permissions",
                json!({"path": [path.to_string_lossy()], "mode":"700"}),
            )
            .unwrap();

        let mode = fs::metadata(&path).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(mode.mode() & 0o777, 0o700);
        }
    });
}

#[test]
fn fileio_set_permissions_multiple_paths() {
    run_case("fileio_set_permissions_multiple_paths", |client, root| {
        let case = case_dir(root, "fileio_set_permissions_multiple_paths");
        let a = case.join("a.txt");
        let b = case.join("b.txt");
        fs::write(&a, "x").unwrap();
        fs::write(&b, "y").unwrap();

        client
            .tool_call(
                "fileio_set_permissions",
                json!({"path": [a.to_string_lossy(), b.to_string_lossy()], "mode":"600"}),
            )
            .unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&a).unwrap().permissions().mode() & 0o777, 0o600);
            assert_eq!(fs::metadata(&b).unwrap().permissions().mode() & 0o777, 0o600);
        }
    });
}

#[test]
fn fileio_set_mode_alias() {
    run_case("fileio_set_mode_alias", |client, root| {
        let case = case_dir(root, "fileio_set_mode_alias");
        let path = case.join("perm2.txt");
        fs::write(&path, "x").unwrap();

        client
            .tool_call(
                "fileio_set_mode",
                json!({"path": [path.to_string_lossy()], "mode":"644"}),
            )
            .unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o644);
        }
    });
}

#[test]
fn fileio_get_permissions() {
    run_case("fileio_get_permissions", |client, root| {
        let case = case_dir(root, "fileio_get_permissions");
        let path = case.join("perm.txt");
        fs::write(&path, "x").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let res = client
            .tool_call(
                "fileio_get_permissions",
                json!({"path": [path.to_string_lossy()]}),
            )
            .unwrap();
        let v = extract_value(&res);

        let mode = if let Some(map) = v.as_object() {
            map.get(path.to_string_lossy().as_ref())
                .and_then(|m| m.as_str())
                .unwrap()
                .to_string()
        } else if let Some(arr) = v.as_array() {
            arr.get(0)
                .and_then(|x| x.as_object())
                .and_then(|o| o.get("mode"))
                .and_then(|m| m.as_str())
                .unwrap()
                .to_string()
        } else {
            panic!("unexpected get_permissions payload: {v}");
        };

        assert!(mode.ends_with("755"), "expected 755-ish, got: {mode}");
    });
}

#[test]
fn fileio_get_permissions_multiple_paths() {
    run_case("fileio_get_permissions_multiple_paths", |client, root| {
        let case = case_dir(root, "fileio_get_permissions_multiple_paths");
        let a = case.join("a.txt");
        let b = case.join("b.txt");
        fs::write(&a, "x").unwrap();
        fs::write(&b, "y").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&a, fs::Permissions::from_mode(0o700)).unwrap();
            fs::set_permissions(&b, fs::Permissions::from_mode(0o644)).unwrap();
        }

        let res = client
            .tool_call(
                "fileio_get_permissions",
                json!({"path": [a.to_string_lossy(), b.to_string_lossy()]}),
            )
            .unwrap();
        let v = extract_value(&res);
        let map = v.as_object().expect("expected mapping");
        assert!(
            map.get(a.to_string_lossy().as_ref())
                .and_then(|m| m.as_str())
                .unwrap()
                .ends_with("700")
        );
        assert!(
            map.get(b.to_string_lossy().as_ref())
                .and_then(|m| m.as_str())
                .unwrap()
                .ends_with("644")
        );
    });
}

#[test]
fn fileio_touch() {
    run_case("fileio_touch", |client, root| {
        let case = case_dir(root, "fileio_touch");
        let path = case.join("touched.txt");
        let _ = fs::remove_file(&path);

        client
            .tool_call("fileio_touch", json!({"path": [path.to_string_lossy()]}))
            .unwrap();
        assert!(path.exists());
    });
}

#[test]
fn fileio_stat() {
    run_case("fileio_stat", |client, root| {
        let case = case_dir(root, "fileio_stat");
        let exists = case.join("a.txt");
        fs::write(&exists, "hello").unwrap();
        let missing = case.join("missing.txt");
        let _ = fs::remove_file(&missing);

        let res = client
            .tool_call(
                "fileio_stat",
                json!({"path": [exists.to_string_lossy(), missing.to_string_lossy()]}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("expected stat array");
        let mut by_path = std::collections::HashMap::new();
        for entry in arr {
            let obj = entry.as_object().expect("stat entry object");
            let p = obj.get("path").and_then(|x| x.as_str()).unwrap().to_string();
            by_path.insert(p, obj.clone());
        }

        assert_eq!(
            by_path
                .get(exists.to_string_lossy().as_ref())
                .and_then(|o| o.get("exists"))
                .and_then(|x| x.as_bool()),
            Some(true)
        );
        assert_eq!(
            by_path
                .get(missing.to_string_lossy().as_ref())
                .and_then(|o| o.get("exists"))
                .and_then(|x| x.as_bool()),
            Some(false)
        );
        assert_eq!(
            by_path
                .get(missing.to_string_lossy().as_ref())
                .and_then(|o| o.get("type"))
                .and_then(|x| x.as_str()),
            Some("not_found")
        );
    });
}

#[test]
fn fileio_make_directory() {
    run_case("fileio_make_directory", |client, root| {
        let case = case_dir(root, "fileio_make_directory");
        let target = case.join("a/b/c");

        client
            .tool_call(
                "fileio_make_directory",
                json!({"path": [target.to_string_lossy()], "recursive": true}),
            )
            .unwrap();
        assert!(target.is_dir());
    });
}

#[test]
fn fileio_make_directory_non_recursive_errors() {
    run_case("fileio_make_directory_non_recursive_errors", |client, root| {
        let case = case_dir(root, "fileio_make_directory_non_recursive_errors");
        let target = case.join("missing_parent/child");

        let res = client.tool_call(
            "fileio_make_directory",
            json!({"path": [target.to_string_lossy()], "recursive": false}),
        );
        expect_err_contains(res, "Some directory creations failed");
    });
}

#[test]
fn fileio_list_directory() {
    run_case("fileio_list_directory", |client, root| {
        let case = case_dir(root, "fileio_list_directory");
        fs::write(case.join("f1.txt"), "x").unwrap();
        fs::create_dir_all(case.join("sub")).unwrap();

        let res = client
            .tool_call(
                "fileio_list_directory",
                json!({"path": case.to_string_lossy(), "recursive": false, "include_hidden": false}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("list_directory array");
        let mut names = std::collections::HashSet::new();
        for entry in arr {
            if let Some(name) = entry.get("name").and_then(|x| x.as_str()) {
                names.insert(name.to_string());
            }
        }
        assert!(names.contains("f1.txt"));
        assert!(names.contains("sub"));
    });
}

#[test]
fn fileio_list_directory_recursive() {
    run_case("fileio_list_directory_recursive", |client, root| {
        let case = case_dir(root, "fileio_list_directory_recursive");
        fs::create_dir_all(case.join("sub")).unwrap();
        let nested = case.join("sub/nested.txt");
        fs::write(&nested, "x").unwrap();

        let res = client
            .tool_call(
                "fileio_list_directory",
                json!({"path": case.to_string_lossy(), "recursive": true, "include_hidden": false}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("list_directory array");
        let mut paths = std::collections::HashSet::new();
        for entry in arr {
            if let Some(p) = entry.get("path").and_then(|x| x.as_str()) {
                paths.insert(p.to_string());
            }
        }
        assert!(paths.contains(nested.to_string_lossy().as_ref()));
    });
}

#[test]
fn fileio_list_directory_include_hidden() {
    run_case("fileio_list_directory_include_hidden", |client, root| {
        let case = case_dir(root, "fileio_list_directory_include_hidden");
        fs::write(case.join(".hidden"), "x").unwrap();

        let res = client
            .tool_call(
                "fileio_list_directory",
                json!({"path": case.to_string_lossy(), "recursive": false, "include_hidden": true}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("list_directory array");
        let mut names = std::collections::HashSet::new();
        for entry in arr {
            if let Some(name) = entry.get("name").and_then(|x| x.as_str()) {
                names.insert(name.to_string());
            }
        }
        assert!(names.contains(".hidden"));
    });
}

#[test]
fn fileio_list_directory_missing_returns_empty() {
    run_case("fileio_list_directory_missing_returns_empty", |client, root| {
        let case = case_dir(root, "fileio_list_directory_missing_returns_empty");
        let missing_dir = case.join("missing");
        let _ = fs::remove_dir_all(&missing_dir);

        let res = client
            .tool_call(
                "fileio_list_directory",
                json!({"path": missing_dir.to_string_lossy(), "recursive": false, "include_hidden": false}),
            )
            .unwrap();
        assert_eq!(extract_value(&res), json!([]));
    });
}

#[test]
fn fileio_find_files() {
    run_case("fileio_find_files", |client, root| {
        let case = case_dir(root, "fileio_find_files");
        fs::write(case.join("a.log"), "x").unwrap();
        fs::write(case.join("b.log"), "x").unwrap();
        fs::write(case.join("c.txt"), "x").unwrap();

        let res = client
            .tool_call(
                "fileio_find_files",
                json!({"root": case.to_string_lossy(), "pattern": "*.log"}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("find_files array");
        let found: std::collections::HashSet<String> = arr
            .iter()
            .filter_map(|p| p.as_str())
            .filter_map(|p| Path::new(p).file_name().and_then(|n| n.to_str()).map(|s| s.to_string()))
            .collect();
        assert!(found.contains("a.log"));
        assert!(found.contains("b.log"));
    });
}

#[test]
fn fileio_find_files_file_type_dir() {
    run_case("fileio_find_files_file_type_dir", |client, root| {
        let case = case_dir(root, "fileio_find_files_file_type_dir");
        let d = case.join("matchdir");
        fs::create_dir_all(&d).unwrap();

        let res = client
            .tool_call(
                "fileio_find_files",
                json!({"root": case.to_string_lossy(), "pattern":"matchdir", "file_type":"dir"}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("find_files array");
        assert!(arr.iter().any(|p| {
            p.as_str()
                .and_then(|s| Path::new(s).file_name().and_then(|n| n.to_str()))
                == Some("matchdir")
        }));
    });
}

#[test]
fn fileio_find_files_missing_root_errors() {
    run_case("fileio_find_files_missing_root_errors", |client, root| {
        let case = case_dir(root, "fileio_find_files_missing_root_errors");
        let missing = case.join("nope");
        let _ = fs::remove_dir_all(&missing);

        let res = client.tool_call(
            "fileio_find_files",
            json!({"root": missing.to_string_lossy(), "pattern":"*.txt"}),
        );
        expect_err_contains(res, "not found");
    });
}

#[test]
fn fileio_find_in_files() {
    run_case("fileio_find_in_files", |client, root| {
        let case = case_dir(root, "fileio_find_in_files");
        let hay = case.join("hay.txt");
        fs::write(&hay, "needle\nother\n").unwrap();

        let res = client
            .tool_call(
                "fileio_find_in_files",
                json!({"path": case.to_string_lossy(), "pattern":"needle", "use_regex":false}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("find_in_files array");
        assert!(arr.iter().any(|m| {
            m.get("file_path").and_then(|p| p.as_str()) == Some(hay.to_string_lossy().as_ref())
        }));
    });
}

#[test]
fn fileio_find_files_directory_alias() {
    run_case("fileio_find_files_directory_alias", |client, root| {
        let case = case_dir(root, "fileio_find_files_directory_alias");
        let sub = case.join("subdir");
        fs::create_dir_all(&sub).unwrap();

        let res = client
            .tool_call(
                "fileio_find_files",
                json!({"pattern": "subdir", "root": case.to_string_lossy(), "file_type": "directory"}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("file_find array");
        assert!(arr.iter().any(|p| p.as_str() == Some(sub.to_string_lossy().as_ref())));
    });
}

#[test]
fn fileio_find_in_files_case_insensitive() {
    run_case("fileio_find_in_files_case_insensitive", |client, root| {
        let case = case_dir(root, "fileio_find_in_files_case_insensitive");
        let hay = case.join("hay.txt");
        fs::write(&hay, "Needle\n").unwrap();

        let res = client
            .tool_call(
                "fileio_find_in_files",
                json!({"path": case.to_string_lossy(), "pattern":"needle", "use_regex":false, "case_sensitive": false}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("find_in_files array");
        assert!(arr.iter().any(|m| {
            m.get("file_path").and_then(|p| p.as_str()) == Some(hay.to_string_lossy().as_ref())
        }));
    });
}

#[test]
fn fileio_find_in_files_whole_word() {
    run_case("fileio_find_in_files_whole_word", |client, root| {
        let case = case_dir(root, "fileio_find_in_files_whole_word");
        let hay = case.join("hay.txt");
        fs::write(&hay, "testing test tested\n").unwrap();

        let res = client
            .tool_call(
                "fileio_find_in_files",
                json!({"path": case.to_string_lossy(), "pattern":"test", "use_regex":false, "whole_word": true}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("find_in_files array");
        assert!(arr.iter().any(|m| {
            m.get("file_path").and_then(|p| p.as_str()) == Some(hay.to_string_lossy().as_ref())
        }));
    });
}

#[test]
fn fileio_find_in_files_column_zero_based() {
    run_case("fileio_find_in_files_column_zero_based", |client, root| {
        let case = case_dir(root, "fileio_find_in_files_column_zero_based");
        let hay = case.join("hay.txt");
        fs::write(&hay, "abc\n").unwrap();

        let res = client
            .tool_call(
                "fileio_find_in_files",
                json!({"path": case.to_string_lossy(), "pattern":"a", "use_regex":false}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("find_in_files array");
        let m = arr.iter().find(|m| {
            m.get("file_path").and_then(|p| p.as_str()) == Some(hay.to_string_lossy().as_ref())
        });
        let m = m.expect("expected match for hay.txt");
        assert_eq!(m.get("column_start").and_then(|c| c.as_u64()), Some(0));
    });
}

#[test]
fn fileio_edit_file_insert_after_anchor() {
    run_case("fileio_edit_file_insert_after_anchor", |client, root| {
        let case = case_dir(root, "fileio_edit_file_insert_after_anchor");
        let path = case.join("Cargo.toml");
        fs::write(
            &path,
            "[package]\nname=\"x\"\n\n[dependencies]\nanyhow=\"1\"\n",
        )
        .unwrap();

        client
            .tool_call(
                "fileio_edit_file",
                json!({
                    "path": path.to_string_lossy(),
                    "edits": [
                        {
                            "op": "insert_after",
                            "search": "[dependencies]\n",
                            "text": "rusqlite = { version = \"0.31\", features = [\"bundled\"] }\n"
                        }
                    ]
                }),
            )
            .unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(
            content,
            "[package]\nname=\"x\"\n\n[dependencies]\nrusqlite = { version = \"0.31\", features = [\"bundled\"] }\nanyhow=\"1\"\n"
        );
    });
}

#[test]
fn fileio_edit_file_replace_lines() {
    run_case("fileio_edit_file_replace_lines", |client, root| {
        let case = case_dir(root, "fileio_edit_file_replace_lines");
        let path = case.join("x.txt");
        fs::write(&path, "a\nb\nc\n").unwrap();

        client
            .tool_call(
                "fileio_edit_file",
                json!({
                    "path": path.to_string_lossy(),
                    "edits": [
                        {"op":"replace_lines", "start_line": 2, "end_line": 2, "text": "B"}
                    ]
                }),
            )
            .unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "a\nB\nc\n");
    });
}

#[test]
fn fileio_edit_file_delete_regex() {
    run_case("fileio_edit_file_delete_regex", |client, root| {
        let case = case_dir(root, "fileio_edit_file_delete_regex");
        let path = case.join("x.txt");
        fs::write(&path, "foo=1\nbar=2\n").unwrap();

        client
            .tool_call(
                "fileio_edit_file",
                json!({
                    "path": path.to_string_lossy(),
                    "edits": [
                        {"op":"delete", "search": "foo=.*\\n", "use_regex": true}
                    ]
                }),
            )
            .unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "bar=2\n");
    });
}

#[test]
fn fileio_edit_file_missing_anchor_errors() {
    run_case("fileio_edit_file_missing_anchor_errors", |client, root| {
        let case = case_dir(root, "fileio_edit_file_missing_anchor_errors");
        let path = case.join("x.txt");
        fs::write(&path, "hello\n").unwrap();

        let res = client.tool_call(
            "fileio_edit_file",
            json!({
                "path": path.to_string_lossy(),
                "edits": [
                    {"op":"insert_after", "search":"NOPE", "text":"x"}
                ]
            }),
        );
        expect_err_contains(res, "not found");
    });
}

#[test]
fn fileio_copy() {
    run_case("fileio_copy", |client, root| {
        let case = case_dir(root, "fileio_copy");
        let src = case.join("src.txt");
        fs::write(&src, "copyme").unwrap();
        let dst = case.join("dst");
        fs::create_dir_all(&dst).unwrap();

        client
            .tool_call(
                "fileio_copy",
                json!({"source": [src.to_string_lossy()], "destination": dst.to_string_lossy()}),
            )
            .unwrap();
        let copied = dst.join("src.txt");
        assert!(copied.exists());
        assert_eq!(fs::read_to_string(copied).unwrap(), "copyme");
    });
}

#[test]
fn fileio_copy_dir_recursive() {
    run_case("fileio_copy_dir_recursive", |client, root| {
        let case = case_dir(root, "fileio_copy_dir_recursive");
        let src_dir = case.join("src");
        fs::create_dir_all(src_dir.join("nested")).unwrap();
        fs::write(src_dir.join("nested/f.txt"), "x").unwrap();
        let dst_dir = case.join("dst");

        client
            .tool_call(
                "fileio_copy",
                json!({"source": [src_dir.to_string_lossy()], "destination": dst_dir.to_string_lossy(), "recursive": true}),
            )
            .unwrap();
        assert!(dst_dir.join("nested/f.txt").exists());
    });
}

#[test]
fn fileio_copy_dir_without_recursive_errors() {
    run_case("fileio_copy_dir_without_recursive_errors", |client, root| {
        let case = case_dir(root, "fileio_copy_dir_without_recursive_errors");
        let src_dir = case.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        let dst_dir = case.join("dst");
        fs::create_dir_all(&dst_dir).unwrap();

        let res = client
            .tool_call(
                "fileio_copy",
                json!({"source": [src_dir.to_string_lossy()], "destination": dst_dir.to_string_lossy(), "recursive": false}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("copy results array");
        let status = arr
            .get(0)
            .and_then(|x| x.get("status"))
            .and_then(|s| s.as_str())
            .unwrap_or("");
        assert!(status.to_lowercase().contains("error"), "expected per-source error, got: {v}");
    });
}

#[test]
fn fileio_copy_glob() {
    run_case("fileio_copy_glob", |client, root| {
        let case = case_dir(root, "fileio_copy_glob");
        fs::write(case.join("a.txt"), "a").unwrap();
        fs::write(case.join("b.txt"), "b").unwrap();
        fs::write(case.join("c.log"), "c").unwrap();
        let dst = case.join("dst");
        fs::create_dir_all(&dst).unwrap();

        let pattern = case.join("*.txt").to_string_lossy().to_string();
        let res = client
            .tool_call(
                "fileio_copy",
                json!({"source": [pattern], "destination": dst.to_string_lossy()}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("copy results array");
        assert!(arr.iter().all(|r| r.get("status") == Some(&Value::String("ok".to_string()))));
        assert!(dst.join("a.txt").exists());
        assert!(dst.join("b.txt").exists());
        assert!(!dst.join("c.log").exists());
    });
}

#[test]
fn fileio_copy_glob_no_match_errors() {
    run_case("fileio_copy_glob_no_match_errors", |client, root| {
        let case = case_dir(root, "fileio_copy_glob_no_match_errors");
        let dst = case.join("dst");
        fs::create_dir_all(&dst).unwrap();
        let pattern = case.join("*.nope").to_string_lossy().to_string();

        let res = client.tool_call(
            "fileio_copy",
            json!({"source": [pattern], "destination": dst.to_string_lossy()}),
        );
        expect_err_contains(res, "No files match pattern");
    });
}

#[test]
fn fileio_move() {
    run_case("fileio_move", |client, root| {
        let case = case_dir(root, "fileio_move");
        let src = case.join("src.txt");
        fs::write(&src, "moveme").unwrap();
        let dst = case.join("moved.txt");
        let _ = fs::remove_file(&dst);

        client
            .tool_call(
                "fileio_move",
                json!({"source": [src.to_string_lossy()], "destination": dst.to_string_lossy()}),
            )
            .unwrap();
        assert!(!src.exists());
        assert_eq!(fs::read_to_string(dst).unwrap(), "moveme");
    });
}

#[test]
fn fileio_move_glob() {
    run_case("fileio_move_glob", |client, root| {
        let case = case_dir(root, "fileio_move_glob");
        fs::write(case.join("a.txt"), "a").unwrap();
        fs::write(case.join("b.txt"), "b").unwrap();
        fs::write(case.join("c.log"), "c").unwrap();
        let dst = case.join("dst");
        fs::create_dir_all(&dst).unwrap();
        let pattern = case.join("*.txt").to_string_lossy().to_string();

        let res = client
            .tool_call(
                "fileio_move",
                json!({"source": [pattern], "destination": dst.to_string_lossy()}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("move results array");
        assert!(arr.iter().all(|r| r.get("status") == Some(&Value::String("ok".to_string()))));
        assert!(!case.join("a.txt").exists());
        assert!(!case.join("b.txt").exists());
        assert!(case.join("c.log").exists());
        assert!(dst.join("a.txt").exists());
        assert!(dst.join("b.txt").exists());
    });
}

#[test]
fn fileio_move_glob_no_match_errors() {
    run_case("fileio_move_glob_no_match_errors", |client, root| {
        let case = case_dir(root, "fileio_move_glob_no_match_errors");
        let dst = case.join("dst");
        fs::create_dir_all(&dst).unwrap();
        let pattern = case.join("*.nope").to_string_lossy().to_string();

        let res = client.tool_call(
            "fileio_move",
            json!({"source": [pattern], "destination": dst.to_string_lossy()}),
        );
        expect_err_contains(res, "No files match pattern");
    });
}

#[test]
fn fileio_remove() {
    run_case("fileio_remove", |client, root| {
        let case = case_dir(root, "fileio_remove");
        let path = case.join("rm.txt");
        fs::write(&path, "x").unwrap();

        client
            .tool_call(
                "fileio_remove",
                json!({"path": [path.to_string_lossy()], "force": false}),
            )
            .unwrap();
        assert!(!path.exists());
    });
}

#[test]
fn fileio_remove_recursive_dir() {
    run_case("fileio_remove_recursive_dir", |client, root| {
        let case = case_dir(root, "fileio_remove_recursive_dir");
        let d = case.join("d");
        fs::create_dir_all(d.join("nested")).unwrap();
        fs::write(d.join("nested/f.txt"), "x").unwrap();

        let res = client
            .tool_call(
                "fileio_remove",
                json!({"path": [d.to_string_lossy()], "recursive": true, "force": false}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("remove results array");
        assert_eq!(arr[0].get("status").and_then(|s| s.as_str()), Some("ok"));
        assert!(!d.exists());
    });
}

#[test]
fn fileio_remove_glob() {
    run_case("fileio_remove_glob", |client, root| {
        let case = case_dir(root, "fileio_remove_glob");
        fs::write(case.join("a.tmp"), "x").unwrap();
        fs::write(case.join("b.tmp"), "x").unwrap();
        fs::write(case.join("c.log"), "x").unwrap();
        let pattern = case.join("*.tmp").to_string_lossy().to_string();

        let res = client
            .tool_call(
                "fileio_remove",
                json!({"path": [pattern], "force": false}),
            )
            .unwrap();
        let v = extract_value(&res);
        let arr = v.as_array().expect("remove results array");
        assert!(arr.iter().all(|r| r.get("status") == Some(&Value::String("ok".to_string()))));
        assert!(!case.join("a.tmp").exists());
        assert!(!case.join("b.tmp").exists());
        assert!(case.join("c.log").exists());
    });
}

#[test]
fn fileio_remove_glob_no_match_errors() {
    run_case("fileio_remove_glob_no_match_errors", |client, root| {
        let case = case_dir(root, "fileio_remove_glob_no_match_errors");
        let pattern = case.join("*.nope").to_string_lossy().to_string();

        let res = client.tool_call(
            "fileio_remove",
            json!({"path": [pattern], "force": false}),
        );
        expect_err_contains(res, "No files match pattern");
    });
}

#[test]
fn fileio_remove_force_missing_ok() {
    run_case("fileio_remove_force_missing_ok", |client, root| {
        let case = case_dir(root, "fileio_remove_force_missing_ok");
        let missing = case.join("missing.txt");
        let _ = fs::remove_file(&missing);

        client
            .tool_call(
                "fileio_remove",
                json!({"path": [missing.to_string_lossy()], "force": true}),
            )
            .unwrap();
    });
}

#[test]
fn fileio_remove_directory() {
    run_case("fileio_remove_directory", |client, root| {
        let case = case_dir(root, "fileio_remove_directory");
        let d = case.join("dir");
        fs::create_dir_all(d.join("nested")).unwrap();
        fs::write(d.join("nested/f.txt"), "x").unwrap();

        client
            .tool_call(
                "fileio_remove_directory",
                json!({"path": [d.to_string_lossy()], "recursive": true}),
            )
            .unwrap();
        assert!(!d.exists());
    });
}

#[test]
fn fileio_remove_directory_non_recursive_reports_error() {
    run_case(
        "fileio_remove_directory_non_recursive_reports_error",
        |client, root| {
            let case = case_dir(root, "fileio_remove_directory_non_recursive_reports_error");
            let d = case.join("dir");
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("f.txt"), "x").unwrap();

            let res = client
                .tool_call(
                    "fileio_remove_directory",
                    json!({"path": [d.to_string_lossy()], "recursive": false}),
                )
                .unwrap();
            let v = extract_value(&res);
            let arr = v.as_array().expect("rmdir results array");
            let status = arr
                .get(0)
                .and_then(|x| x.get("status"))
                .and_then(|s| s.as_str())
                .unwrap_or("");
            assert!(
                status.to_lowercase().contains("not empty"),
                "expected not-empty status, got: {status}"
            );
            assert!(d.exists());
        },
    );
}

#[test]
fn fileio_create_hard_link() {
    run_case("fileio_create_hard_link", |client, root| {
        let case = case_dir(root, "fileio_create_hard_link");
        let target = case.join("target.txt");
        fs::write(&target, "x").unwrap();
        let link = case.join("hard.txt");
        let _ = fs::remove_file(&link);

        client
            .tool_call(
                "fileio_create_hard_link",
                json!({"target": target.to_string_lossy(), "link_path": link.to_string_lossy()}),
            )
            .unwrap();
        assert!(link.exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(fs::metadata(&target).unwrap().ino(), fs::metadata(&link).unwrap().ino());
        }
    });
}

#[test]
fn fileio_create_hard_link_missing_target_errors() {
    run_case("fileio_create_hard_link_missing_target_errors", |client, root| {
        let case = case_dir(root, "fileio_create_hard_link_missing_target_errors");
        let target = case.join("missing.txt");
        let _ = fs::remove_file(&target);
        let link = case.join("hard.txt");
        let _ = fs::remove_file(&link);

        let res = client.tool_call(
            "fileio_create_hard_link",
            json!({"target": target.to_string_lossy(), "link_path": link.to_string_lossy()}),
        );
        expect_err_contains(res, "not found");
    });
}

#[test]
fn fileio_create_symbolic_link() {
    run_case("fileio_create_symbolic_link", |client, root| {
        let case = case_dir(root, "fileio_create_symbolic_link");
        let target = case.join("target.txt");
        fs::write(&target, "x").unwrap();
        let link = case.join("sym.txt");
        let _ = fs::remove_file(&link);

        client
            .tool_call(
                "fileio_create_symbolic_link",
                json!({"target": target.to_string_lossy(), "link_path": link.to_string_lossy()}),
            )
            .unwrap();

        #[cfg(unix)]
        {
            assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
            let stored = fs::read_link(&link).unwrap();
            assert_eq!(stored.to_string_lossy(), target.to_string_lossy());
        }
    });
}

#[test]
fn fileio_create_symbolic_link_broken_target_ok() {
    run_case("fileio_create_symbolic_link_broken_target_ok", |client, root| {
        let case = case_dir(root, "fileio_create_symbolic_link_broken_target_ok");
        let missing_target = case.join("missing.txt");
        let _ = fs::remove_file(&missing_target);
        let link = case.join("broken.txt");
        let _ = fs::remove_file(&link);

        client
            .tool_call(
                "fileio_create_symbolic_link",
                json!({"target": missing_target.to_string_lossy(), "link_path": link.to_string_lossy()}),
            )
            .unwrap();

        #[cfg(unix)]
        {
            assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
            let stored = fs::read_link(&link).unwrap();
            assert_eq!(stored.to_string_lossy(), missing_target.to_string_lossy());
        }
    });
}

#[test]
fn fileio_read_symbolic_link_ok() {
    run_case("fileio_read_symbolic_link_ok", |client, root| {
        let case = case_dir(root, "fileio_read_symbolic_link_ok");
        let target = case.join("t.txt");
        fs::write(&target, "x").unwrap();
        let link = case.join("l.txt");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(&target, &link).unwrap();
        }

        let res = client
            .tool_call(
                "fileio_read_symbolic_link",
                json!({"path": link.to_string_lossy()}),
            )
            .unwrap();
        assert_eq!(extract_value(&res), Value::String(target.to_string_lossy().to_string()));
    });
}

#[test]
fn fileio_read_symbolic_link_relative_target() {
    run_case("fileio_read_symbolic_link_relative_target", |client, root| {
        let case = case_dir(root, "fileio_read_symbolic_link_relative_target");
        fs::write(case.join("target.txt"), "x").unwrap();
        let link = case.join("link.txt");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink("target.txt", &link).unwrap();
        }

        let res = client
            .tool_call(
                "fileio_read_symbolic_link",
                json!({"path": link.to_string_lossy()}),
            )
            .unwrap();
        assert_eq!(extract_value(&res), Value::String("target.txt".to_string()));
    });
}

#[test]
fn fileio_read_symbolic_link_non_symlink_errors() {
    run_case("fileio_read_symbolic_link_non_symlink_errors", |client, root| {
        let case = case_dir(root, "fileio_read_symbolic_link_non_symlink_errors");
        let f = case.join("file.txt");
        fs::write(&f, "x").unwrap();

        let res = client
            .tool_call(
                "fileio_read_symbolic_link",
                json!({"path": f.to_string_lossy()}),
            );
        expect_err_contains(res, "not a symbolic link");
    });
}

#[test]
fn fileio_get_basename() {
    run_case("fileio_get_basename", |client, root| {
        let case = case_dir(root, "fileio_get_basename");
        let p = case.join("a/b/c.txt");

        let res = client
            .tool_call("fileio_get_basename", json!({"path": p.to_string_lossy()}))
            .unwrap();
        assert_eq!(extract_value(&res), Value::String("c.txt".to_string()));
    });
}

#[test]
fn fileio_get_basename_trailing_slash() {
    run_case("fileio_get_basename_trailing_slash", |client, _root| {
        let res = client
            .tool_call("fileio_get_basename", json!({"path": "/usr/bin/"}))
            .unwrap();
        assert_eq!(extract_value(&res), Value::String("bin".to_string()));
    });
}

#[test]
fn fileio_get_dirname() {
    run_case("fileio_get_dirname", |client, root| {
        let case = case_dir(root, "fileio_get_dirname");
        let p = case.join("a/b/c.txt");

        let res = client
            .tool_call("fileio_get_dirname", json!({"path": p.to_string_lossy()}))
            .unwrap();
        assert_eq!(
            extract_value(&res),
            Value::String(p.parent().unwrap().to_string_lossy().to_string())
        );
    });
}

#[test]
fn fileio_get_dirname_no_dir_component() {
    run_case("fileio_get_dirname_no_dir_component", |client, _root| {
        let res = client
            .tool_call("fileio_get_dirname", json!({"path": "file.txt"}))
            .unwrap();
        assert_eq!(extract_value(&res), Value::String("".to_string()));
    });
}

#[test]
fn fileio_get_canonical_path_ok() {
    run_case("fileio_get_canonical_path_ok", |client, root| {
        let case = case_dir(root, "fileio_get_canonical_path_ok");
        let p = case.join("x.txt");
        fs::write(&p, "x").unwrap();

        let res = client
            .tool_call("fileio_get_canonical_path", json!({"path": p.to_string_lossy()}))
            .unwrap();
        assert_eq!(
            PathBuf::from(extract_value(&res).as_str().unwrap()),
            p.canonicalize().unwrap()
        );
    });
}

#[test]
fn fileio_get_canonical_path_missing_errors() {
    run_case("fileio_get_canonical_path_missing_errors", |client, root| {
        let case = case_dir(root, "fileio_get_canonical_path_missing_errors");
        let p = case.join("missing.txt");
        let _ = fs::remove_file(&p);

        let res = client.tool_call("fileio_get_canonical_path", json!({"path": p.to_string_lossy()}));
        expect_err_contains(res, "not found");
    });
}

#[test]
fn fileio_get_current_directory() {
    run_case("fileio_get_current_directory", |client, _root| {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let res = client
            .tool_call("fileio_get_current_directory", json!({}))
            .unwrap();
        assert_eq!(
            extract_value(&res),
            Value::String(repo_root.to_string_lossy().to_string())
        );
    });
}

#[test]
fn fileio_create_temporary_file() {
    run_case("fileio_create_temporary_file", |client, root| {
        let case = case_dir(root, "fileio_create_temporary_file");

        let res = client
            .tool_call(
                "fileio_create_temporary",
                json!({"type": "file", "template": case.to_string_lossy()}),
            )
            .unwrap();
        let p = PathBuf::from(extract_value(&res).as_str().unwrap());
        assert!(p.exists() && p.is_file());
    });
}

#[test]
fn fileio_create_temporary_file_no_template() {
    run_case("fileio_create_temporary_file_no_template", |client, _root| {
        let res = client
            .tool_call("fileio_create_temporary", json!({"type": "file"}))
            .unwrap();
        let p = PathBuf::from(extract_value(&res).as_str().unwrap());
        assert!(p.exists() && p.is_file());
    });
}

#[test]
fn fileio_create_temporary_dir() {
    run_case("fileio_create_temporary_dir", |client, root| {
        let case = case_dir(root, "fileio_create_temporary_dir");

        let res = client
            .tool_call(
                "fileio_create_temporary",
                json!({"type": "dir", "template": case.to_string_lossy()}),
            )
            .unwrap();
        let p = PathBuf::from(extract_value(&res).as_str().unwrap());
        assert!(p.exists() && p.is_dir());
    });
}

#[test]
fn fileio_count_lines_ok() {
    run_case("fileio_count_lines_ok", |client, root| {
        let case = case_dir(root, "fileio_count_lines_ok");
        let p = case.join("c.txt");
        fs::write(&p, "a\nb\n").unwrap();

        let res = client
            .tool_call("fileio_count_lines", json!({"path": [p.to_string_lossy()]}))
            .unwrap();
        let v = extract_value(&res);
        assert_eq!(
            v.as_array().unwrap()[0]
                .get("lines")
                .and_then(|n| n.as_u64()),
            Some(2)
        );
        assert_eq!(
            v.as_array().unwrap()[0]
                .get("status")
                .and_then(|s| s.as_str()),
            Some("ok")
        );
        assert_eq!(
            v.as_array().unwrap()[0]
                .get("exists")
                .and_then(|b| b.as_bool()),
            Some(true)
        );
    });
}

#[test]
fn fileio_count_lines_empty_file_zero() {
    run_case("fileio_count_lines_empty_file_zero", |client, root| {
        let case = case_dir(root, "fileio_count_lines_empty_file_zero");
        let p = case.join("empty.txt");
        fs::write(&p, "").unwrap();

        let res = client
            .tool_call("fileio_count_lines", json!({"path": [p.to_string_lossy()]}))
            .unwrap();
        let v = extract_value(&res);
        assert_eq!(
            v.as_array().unwrap()[0]
                .get("lines")
                .and_then(|n| n.as_u64()),
            Some(0)
        );
    });
}

#[test]
fn fileio_count_lines_single_line_no_newline() {
    run_case("fileio_count_lines_single_line_no_newline", |client, root| {
        let case = case_dir(root, "fileio_count_lines_single_line_no_newline");
        let p = case.join("single.txt");
        fs::write(&p, "single line").unwrap();

        let res = client
            .tool_call("fileio_count_lines", json!({"path": [p.to_string_lossy()]}))
            .unwrap();
        let v = extract_value(&res);
        assert_eq!(
            v.as_array().unwrap()[0]
                .get("lines")
                .and_then(|n| n.as_u64()),
            Some(1)
        );
    });
}

#[test]
fn fileio_count_lines_string_path_errors() {
    run_case("fileio_count_lines_string_path_errors", |client, root| {
        let case = case_dir(root, "fileio_count_lines_string_path_errors");
        let p = case.join("c.txt");
        fs::write(&p, "a\n").unwrap();

        let res = client.tool_call("fileio_count_lines", json!({"path": p.to_string_lossy()}));
        expect_err_contains(res, "Path must be an array of strings");
    });
}

#[test]
fn fileio_count_lines_missing_status() {
    run_case("fileio_count_lines_missing_status", |client, root| {
        let case = case_dir(root, "fileio_count_lines_missing_status");
        let p = case.join("missing.txt");
        let _ = fs::remove_file(&p);

        let res = client
            .tool_call("fileio_count_lines", json!({"path": [p.to_string_lossy()]}))
            .unwrap();
        let v = extract_value(&res);
        assert_eq!(
            v.as_array().unwrap()[0]
                .get("exists")
                .and_then(|b| b.as_bool()),
            Some(false)
        );
    });
}

#[test]
fn fileio_count_words_ok() {
    run_case("fileio_count_words_ok", |client, root| {
        let case = case_dir(root, "fileio_count_words_ok");
        let p = case.join("w.txt");
        fs::write(&p, "hello world\nfoo").unwrap();

        let res = client
            .tool_call("fileio_count_words", json!({"path": [p.to_string_lossy()]}))
            .unwrap();
        let v = extract_value(&res);
        assert_eq!(
            v.as_array().unwrap()[0]
                .get("words")
                .and_then(|n| n.as_u64()),
            Some(3)
        );
        assert_eq!(
            v.as_array().unwrap()[0]
                .get("status")
                .and_then(|s| s.as_str()),
            Some("ok")
        );
        assert_eq!(
            v.as_array().unwrap()[0]
                .get("exists")
                .and_then(|b| b.as_bool()),
            Some(true)
        );
    });
}

#[test]
fn fileio_count_words_missing_status() {
    run_case("fileio_count_words_missing_status", |client, root| {
        let case = case_dir(root, "fileio_count_words_missing_status");
        let p = case.join("missing.txt");
        let _ = fs::remove_file(&p);

        let res = client
            .tool_call("fileio_count_words", json!({"path": [p.to_string_lossy()]}))
            .unwrap();
        let v = extract_value(&res);
        assert_eq!(
            v.as_array().unwrap()[0]
                .get("exists")
                .and_then(|b| b.as_bool()),
            Some(false)
        );
    });
}

#[test]
fn fileio_change_ownership_skipped_unless_enabled() {
    if !dangerous_enabled() {
        return;
    }

    run_case("fileio_change_ownership", |client, root| {
        let case = case_dir(root, "fileio_change_ownership");
        let p = case.join("owned.txt");
        fs::write(&p, "x").unwrap();
        let uid = geteuid().as_raw().to_string();
        let gid = getegid().as_raw().to_string();

        client
            .tool_call(
                "fileio_change_ownership",
                json!({"path": [p.to_string_lossy()], "user": uid, "group": gid}),
            )
            .unwrap();
    });
}

#[test]
fn fileio_change_ownership_noop_without_user_group() {
    if !dangerous_enabled() {
        return;
    }

    run_case("fileio_change_ownership_noop_without_user_group", |client, root| {
        let case = case_dir(root, "fileio_change_ownership_noop_without_user_group");
        let p = case.join("owned.txt");
        fs::write(&p, "x").unwrap();

        client
            .tool_call(
                "fileio_change_ownership",
                json!({"path": [p.to_string_lossy()]}),
            )
            .unwrap();
        assert!(p.exists());
    });
}
