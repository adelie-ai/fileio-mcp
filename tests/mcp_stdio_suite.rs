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

    tempfile::Builder::new()
        .prefix(&format!("{safe}_"))
        .tempdir_in(root.path())
        .expect("create case dir")
        .keep()
}

#[test]
fn mcp_stdio_integration_suite() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let test_root = TempDir::new().expect("create temp root");

    let mut client = McpStdioClient::start();
    client.initialize();

    // write_file
    {
        let case = case_dir(&test_root, "write_file");
        let path = case.join("nested/out.txt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();

        client
            .tool_call(
                "fileio_write_file",
                json!({"path": path.to_string_lossy(), "content":"hello\n", "append":false}),
            )
            .unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "hello\n");
    }

    // read_lines (happy path + edge cases)
    {
        let case = case_dir(&test_root, "read_lines");
        let path = case.join("in.txt");
        fs::write(&path, "a\nb\nc\n").unwrap();

        let res = client
            .tool_call("fileio_read_lines", json!({"path": path.to_string_lossy()}))
            .unwrap();
        assert_eq!(extract_value(&res), json!(["a", "b", "c"]));

        let res = client
            .tool_call(
                "fileio_read_lines",
                json!({"path": path.to_string_lossy(), "start_line": 2, "end_line": 999}),
            )
            .unwrap();
        assert_eq!(extract_value(&res), json!(["b", "c"]));

        let res = client.tool_call(
            "fileio_read_lines",
            json!({"path": path.to_string_lossy(), "start_line": 2, "end_line": 1}),
        );
        expect_err_contains(res, "end_line");

        let res = client.tool_call(
            "fileio_read_lines",
            json!({"path": path.to_string_lossy(), "start_line": -1}),
        );
        expect_err_contains(res, "non-negative");
    }

    // patch_file (both formats)
    {
        let case = case_dir(&test_root, "patch_file");
        let path = case.join("patch.txt");
        fs::write(&path, "line 1\nline 2\nline 3\n").unwrap();

        let patch = json!({"operations":[{"type":"add","line":2,"content":"inserted"},{"type":"remove","line":3}]});
        client
			.tool_call(
				"fileio_patch_file",
				json!({"path": path.to_string_lossy(), "patch": patch.to_string(), "format":"add_remove_lines"}),
			)
			.unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "line 1\ninserted\nline 2"
        );

        fs::write(&path, "line 1\nline 2\nline 3\n").unwrap();
        let diff = "@@ -1,3 +1,3 @@\n line 1\n-line 2\n+line two\n line 3\n";
        client
            .tool_call(
                "fileio_patch_file",
                json!({"path": path.to_string_lossy(), "patch": diff, "format":"unified_diff"}),
            )
            .unwrap();
        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines, vec!["line 1", "line two", "line 3"]);
    }

    // mkdir / list_directory
    {
        let case = case_dir(&test_root, "mkdir_list");
        let dir = case.join("a/b/c");
        client
            .tool_call(
                "fileio_make_directory",
                json!({"path": [dir.to_string_lossy()]}),
            )
            .unwrap();
        fs::write(dir.join("f.txt"), "x").unwrap();

        let res = client
            .tool_call(
                "fileio_list_directory",
                json!({"path": case.to_string_lossy(), "recursive": true}),
            )
            .unwrap();
        let v = extract_value(&res);
        assert!(v.as_array().unwrap().len() >= 2);
    }

    // find_files / find_in_files
    {
        let case = case_dir(&test_root, "find");
        fs::write(case.join("a.txt"), "hello needle\n").unwrap();
        fs::write(case.join("b.log"), "nope\n").unwrap();

        let res = client
            .tool_call(
                "fileio_find_files",
                json!({"root": case.to_string_lossy(), "pattern":"*.txt"}),
            )
            .unwrap();
        let v = extract_value(&res);
        assert!(v
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p == &Value::String(case.join("a.txt").to_string_lossy().to_string())));

        let res = client
            .tool_call(
                "fileio_find_in_files",
                json!({"path": case.to_string_lossy(), "pattern":"needle", "use_regex":false}),
            )
            .unwrap();
        let v = extract_value(&res);
        assert!(!v.as_array().unwrap().is_empty());
    }

    // copy / move / remove
    {
        let case = case_dir(&test_root, "cp_mv_rm");
        let src = case.join("src.txt");
        fs::write(&src, "data").unwrap();
        let dst_dir = case.join("dst");
        fs::create_dir_all(&dst_dir).unwrap();

        client
            .tool_call(
                "fileio_copy",
                json!({"source":[src.to_string_lossy()],"destination":dst_dir.to_string_lossy()}),
            )
            .unwrap();
        assert_eq!(fs::read_to_string(dst_dir.join("src.txt")).unwrap(), "data");

        let moved = case.join("moved.txt");
        client
            .tool_call(
                "fileio_move",
                json!({"source":[src.to_string_lossy()],"destination":moved.to_string_lossy()}),
            )
            .unwrap();
        assert!(!src.exists());
        assert!(moved.exists());

        client
            .tool_call("fileio_remove", json!({"path":[moved.to_string_lossy()]}))
            .unwrap();
        assert!(!moved.exists());
    }

    // remove_directory
    {
        let case = case_dir(&test_root, "rmdir");
        let dir = case.join("d");
        fs::create_dir_all(&dir).unwrap();
        client
            .tool_call(
                "fileio_remove_directory",
                json!({"path":[dir.to_string_lossy()]}),
            )
            .unwrap();
        assert!(!dir.exists());
    }

    // touch / stat
    {
        let case = case_dir(&test_root, "touch_stat");
        let p = case.join("t.txt");
        client
            .tool_call("fileio_touch", json!({"path":[p.to_string_lossy()]}))
            .unwrap();
        assert!(p.exists());

        let res = client
            .tool_call("fileio_stat", json!({"path":[p.to_string_lossy()]}))
            .unwrap();
        let v = extract_value(&res);
        assert!(v.as_array().unwrap()[0].get("size").is_some());
    }

    // get/set permissions
    {
        let case = case_dir(&test_root, "perms");
        let p = case.join("perm.txt");
        fs::write(&p, "x").unwrap();

        client
            .tool_call(
                "fileio_set_permissions",
                json!({"path": [p.to_string_lossy()], "mode":"700"}),
            )
            .unwrap();

        let res = client
            .tool_call(
                "fileio_get_permissions",
                json!({"path": [p.to_string_lossy()]}),
            )
            .unwrap();
        let v = extract_value(&res);
        let mode = v
            .get(p.to_string_lossy().as_ref())
            .and_then(|m| m.as_str())
            .unwrap();
        assert!(mode.ends_with("700"), "expected 700, got: {mode}");
    }

    // count_lines / count_words
    {
        let case = case_dir(&test_root, "counts");
        let p = case.join("x.txt");
        fs::write(&p, "a\n\n\nb").unwrap();

        let res = client
            .tool_call("fileio_count_lines", json!({"path":[p.to_string_lossy()]}))
            .unwrap();
        let v = extract_value(&res);
        assert_eq!(
            v.as_array().unwrap()[0]
                .get("lines")
                .and_then(|n| n.as_u64()),
            Some(4)
        );

        let res = client
            .tool_call("fileio_count_words", json!({"path":[p.to_string_lossy()]}))
            .unwrap();
        let v = extract_value(&res);
        assert!(v.as_array().unwrap()[0]
            .get("words")
            .and_then(|n| n.as_u64())
            .is_some());
    }

    // mktemp
    {
        let case = case_dir(&test_root, "mktemp");
        let res = client
            .tool_call(
                "fileio_create_temporary",
                json!({"type":"dir","template":case.to_string_lossy()}),
            )
            .unwrap();
        let p = PathBuf::from(extract_value(&res).as_str().unwrap());
        assert!(p.exists());
        assert!(p.is_dir());
    }

    // links (unix)
    {
        let case = case_dir(&test_root, "links");
        let target = case.join("t.txt");
        fs::write(&target, "x").unwrap();

        let hard = case.join("hard.txt");
        client
            .tool_call(
                "fileio_create_hard_link",
                json!({"target": target.to_string_lossy(), "link_path": hard.to_string_lossy()}),
            )
            .unwrap();
        assert_eq!(fs::read_to_string(&hard).unwrap(), "x");

        let sym = case.join("sym.txt");
        client
            .tool_call(
                "fileio_create_symbolic_link",
                json!({"target": target.to_string_lossy(), "link_path": sym.to_string_lossy()}),
            )
            .unwrap();
        assert!(sym.exists());

        let res = client
            .tool_call(
                "fileio_read_symbolic_link",
                json!({"path": sym.to_string_lossy()}),
            )
            .unwrap();
        let v = extract_value(&res);
        assert_eq!(v, Value::String(target.to_string_lossy().to_string()));
    }

    // path utils + pwd
    {
        let res = client
            .tool_call("fileio_get_basename", json!({"path":"/a/b/c.txt"}))
            .unwrap();
        assert_eq!(extract_value(&res), Value::String("c.txt".to_string()));

        let res = client
            .tool_call("fileio_get_dirname", json!({"path":"/a/b/c.txt"}))
            .unwrap();
        assert_eq!(extract_value(&res), Value::String("/a/b".to_string()));

        let res = client
            .tool_call("fileio_get_current_directory", json!({}))
            .unwrap();
        assert_eq!(
            extract_value(&res),
            Value::String(repo_root.to_string_lossy().to_string())
        );
    }

    // dangerous: chown (only if enabled)
    if dangerous_enabled() {
        let case = case_dir(&test_root, "chown");
        let p = case.join("owned.txt");
        fs::write(&p, "x").unwrap();
        let uid = geteuid().as_raw().to_string();
        let gid = getegid().as_raw().to_string();
        client
            .tool_call(
                "fileio_change_ownership",
                json!({"path":[p.to_string_lossy()],"user":uid,"group":gid}),
            )
            .unwrap();
    }
}
