#![deny(warnings)]

//! Acceptance tests for the path allowlist (issues #20 and #1).
//!
//! The guard permits a small set of roots and refuses everything else, on
//! arguments and on results alike. These tests drive the public tool
//! dispatch, so they prove the wiring in `tools.rs` and the decision in
//! `path_guard.rs` together.
//!
//! Every test builds its own temporary root and names it as the only
//! allowlist entry, so no test can reach a real home directory whatever the
//! code under test does. Refusals get most of the attention here: a guard
//! that never fires under test is a guard nobody has tested.

use std::fs;
use std::path::Path;

use fileio_mcp::path_guard::{PathGuard, legacy_block_flag_warning};
use fileio_mcp::tools::ToolRegistry;
use serde_json::{Value, json};
use tempfile::TempDir;

/// A guard whose only allowlist root is `root`.
fn guard_rooted_at(root: &Path) -> PathGuard {
    PathGuard::with_roots(&[root.to_string_lossy().into_owned()])
}

/// A registry that may reach `root` and nothing else.
fn registry_rooted_at(root: &Path) -> ToolRegistry {
    ToolRegistry::with_guard(guard_rooted_at(root))
}

/// The tool's response body as text. Every tool returns a single text
/// content block, whether the body is plain text or serialized JSON.
fn body_text(response: &Value) -> String {
    response["content"][0]["text"]
        .as_str()
        .expect("a tool response carries a text content block")
        .to_string()
}

/// Assert a call was refused the way an absent file is reported.
#[track_caller]
fn assert_reported_not_found(result: fileio_mcp::error::Result<Value>, what: &str) {
    match result {
        Ok(response) => panic!("{what} must be refused, got a result: {response}"),
        Err(error) => {
            let message = error.to_string().to_lowercase();
            assert!(
                message.contains("not found"),
                "{what} must be refused as 'not found', got: {message}"
            );
        }
    }
}

/// A path string under `dir`.
fn at(dir: &Path, name: &str) -> String {
    dir.join(name).to_string_lossy().into_owned()
}

// ---------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------

#[tokio::test]
async fn read_outside_the_allowlist_reports_not_found() {
    let root = TempDir::new().expect("allowed root");
    let outside = TempDir::new().expect("outside root");
    let secret = outside.path().join("outside-secret.txt");
    fs::write(&secret, "real contents").expect("write the outside file");

    let registry = registry_rooted_at(root.path());
    let result = registry
        .execute_tool(
            "fileio_read_lines",
            &json!({"path": secret.to_string_lossy()}),
        )
        .await;

    assert_reported_not_found(result, "a read outside every allowlist root");
}

#[tokio::test]
async fn write_outside_the_allowlist_reports_not_found() {
    let root = TempDir::new().expect("allowed root");
    let outside = TempDir::new().expect("outside root");
    let target = outside.path().join("outside-new.txt");

    let registry = registry_rooted_at(root.path());
    let result = registry
        .execute_tool(
            "fileio_write_file",
            &json!({"path": target.to_string_lossy(), "content": "should never land"}),
        )
        .await;

    assert_reported_not_found(result, "a write outside every allowlist root");
    assert!(!target.exists(), "a refused write must not create the file");
}

#[tokio::test]
async fn symlink_target_outside_the_allowlist_is_refused() {
    let root = TempDir::new().expect("allowed root");
    let outside = TempDir::new().expect("outside root");
    let secret = outside.path().join("outside-secret.txt");
    fs::write(&secret, "real contents").expect("write the outside file");

    let link = root.path().join("escape-link.txt");
    std::os::unix::fs::symlink(&secret, &link).expect("create the escaping symlink");

    let registry = registry_rooted_at(root.path());
    let result = registry
        .execute_tool(
            "fileio_read_lines",
            &json!({"path": link.to_string_lossy()}),
        )
        .await;

    assert_reported_not_found(result, "a read through a symlink that leaves the root");
}

#[tokio::test]
async fn path_traversal_out_of_an_allowed_root_is_refused() {
    let root = TempDir::new().expect("allowed root");
    let outside = TempDir::new().expect("outside root");
    let secret = outside.path().join("outside-secret.txt");
    fs::write(&secret, "real contents").expect("write the outside file");

    let outside_name = outside
        .path()
        .file_name()
        .expect("the outside root has a name")
        .to_string_lossy()
        .into_owned();
    // Both temporary roots are siblings, so `..` from one reaches the other.
    let traversal = format!(
        "{}/../{}/outside-secret.txt",
        root.path().display(),
        outside_name
    );

    let registry = registry_rooted_at(root.path());
    let result = registry
        .execute_tool("fileio_read_lines", &json!({"path": traversal}))
        .await;

    assert_reported_not_found(result, "a read that walks out of the root with '..'");
}

#[tokio::test]
async fn edit_file_outside_the_allowlist_reports_not_found_and_changes_nothing() {
    let root = TempDir::new().expect("allowed root");
    let outside = TempDir::new().expect("outside root");
    let target = outside.path().join("outside-secret.txt");
    fs::write(&target, "hello world").expect("write the outside file");

    let registry = registry_rooted_at(root.path());
    let result = registry
        .execute_tool(
            "fileio_edit_file",
            &json!({
                "path": target.to_string_lossy(),
                "edits": [{"op": "replace", "search": "world", "text": "rust"}],
            }),
        )
        .await;

    assert_reported_not_found(result, "an edit outside every allowlist root");
    assert_eq!(
        fs::read_to_string(&target).expect("read the outside file back"),
        "hello world",
        "a refused edit must leave the file alone"
    );
}

#[tokio::test]
async fn copy_from_outside_the_allowlist_reports_not_found_and_copies_nothing() {
    let root = TempDir::new().expect("allowed root");
    let outside = TempDir::new().expect("outside root");
    let source = outside.path().join("outside-secret.txt");
    fs::write(&source, "real contents").expect("write the outside file");
    let destination = root.path().join("copied.txt");

    let registry = registry_rooted_at(root.path());
    let result = registry
        .execute_tool(
            "fileio_copy",
            &json!({
                "source": [source.to_string_lossy()],
                "destination": destination.to_string_lossy(),
            }),
        )
        .await;

    assert_reported_not_found(result, "a copy whose source is outside every root");
    assert!(
        !destination.exists(),
        "a refused copy must not create the destination"
    );
}

#[tokio::test]
async fn copy_of_a_glob_matching_an_escaping_symlink_is_refused() {
    let root = TempDir::new().expect("allowed root");
    let outside = TempDir::new().expect("outside root");
    let secret = outside.path().join("outside-secret.txt");
    fs::write(&secret, "real contents").expect("write the outside file");

    // The glob itself never leaves the root. One of the entries it matches
    // does, so the pattern alone is not enough to decide the call.
    fs::write(root.path().join("plain.txt"), "ordinary").expect("write the inside file");
    std::os::unix::fs::symlink(&secret, root.path().join("escape-link.txt"))
        .expect("create the escaping symlink");
    let destination = root.path().join("dest");
    fs::create_dir_all(&destination).expect("create the destination directory");

    let registry = registry_rooted_at(root.path());
    let result = registry
        .execute_tool(
            "fileio_copy",
            &json!({
                "source": [format!("{}/*.txt", root.path().display())],
                "destination": destination.to_string_lossy(),
            }),
        )
        .await;

    assert_reported_not_found(result, "a copy whose glob matches an escaping symlink");
    let copied: Vec<_> = fs::read_dir(&destination)
        .expect("read the destination")
        .map(|entry| entry.expect("a destination entry").path())
        .collect();
    assert!(
        copied.is_empty(),
        "a refused copy must copy nothing, found: {copied:?}"
    );
}

#[tokio::test]
async fn copy_of_a_glob_matching_only_permitted_entries_succeeds() {
    let root = TempDir::new().expect("allowed root");
    fs::write(root.path().join("one.txt"), "first").expect("write the first file");
    fs::write(root.path().join("two.txt"), "second").expect("write the second file");
    let destination = root.path().join("dest");
    fs::create_dir_all(&destination).expect("create the destination directory");

    let registry = registry_rooted_at(root.path());
    registry
        .execute_tool(
            "fileio_copy",
            &json!({
                "source": [format!("{}/*.txt", root.path().display())],
                "destination": destination.to_string_lossy(),
            }),
        )
        .await
        .expect("a glob wholly inside the root must be copied");

    assert!(destination.join("one.txt").exists());
    assert!(destination.join("two.txt").exists());
}

/// A relative symlink target resolves against the link's own directory once
/// the link exists. Checking it against the process working directory decides
/// a different path from the one the link will point at.
#[tokio::test]
async fn relative_symlink_target_is_checked_against_the_link_directory() {
    let root = TempDir::new().expect("allowed root");
    let nested = root.path().join("sub");
    fs::create_dir_all(&nested).expect("create the nested directory");
    fs::write(root.path().join("inside.txt"), "reachable").expect("write the inside file");

    let registry = registry_rooted_at(root.path());

    // "../inside.txt" from <root>/sub is <root>/inside.txt, inside the root.
    // Resolved against the working directory instead, it lands outside the
    // root and the call would be refused.
    registry
        .execute_tool(
            "fileio_create_symbolic_link",
            &json!({
                "target": "../inside.txt",
                "link_path": nested.join("ok-link").to_string_lossy(),
            }),
        )
        .await
        .expect("a relative target inside the root must be allowed");
    assert!(
        nested.join("ok-link").is_symlink(),
        "the permitted link must actually be created"
    );

    // "../../escape.txt" from <root>/sub leaves the root.
    let result = registry
        .execute_tool(
            "fileio_create_symbolic_link",
            &json!({
                "target": "../../escape.txt",
                "link_path": nested.join("bad-link").to_string_lossy(),
            }),
        )
        .await;

    assert_reported_not_found(result, "a relative symlink target that leaves the root");
    assert!(
        !nested.join("bad-link").exists() && !nested.join("bad-link").is_symlink(),
        "a refused link must not be created"
    );
}

#[tokio::test]
async fn stat_refuses_the_call_when_one_path_is_outside_the_allowlist() {
    let root = TempDir::new().expect("allowed root");
    let outside = TempDir::new().expect("outside root");
    let inside = root.path().join("inside-visible.txt");
    fs::write(&inside, "visible").expect("write the inside file");
    let secret = outside.path().join("outside-secret.txt");
    fs::write(&secret, "secret").expect("write the outside file");

    let registry = registry_rooted_at(root.path());
    let result = registry
        .execute_tool(
            "fileio_stat",
            &json!({"path": [inside.to_string_lossy(), secret.to_string_lossy()]}),
        )
        .await;

    assert_reported_not_found(result, "a stat with one path outside every root");
}

#[tokio::test]
async fn create_temporary_outside_the_allowlist_reports_not_found_and_creates_nothing() {
    let root = TempDir::new().expect("allowed root");
    let outside = TempDir::new().expect("outside root");

    let registry = registry_rooted_at(root.path());
    let template = at(outside.path(), "probe-XXXXXX");
    let result = registry
        .execute_tool(
            "fileio_create_temporary",
            &json!({"type": "file", "template": template}),
        )
        .await;

    assert_reported_not_found(result, "a temporary file outside every root");
    let left_behind: Vec<_> = fs::read_dir(outside.path())
        .expect("read the outside root")
        .map(|entry| entry.expect("an outside entry").path())
        .collect();
    assert!(
        left_behind.is_empty(),
        "a refused temporary must create nothing, found: {left_behind:?}"
    );
}

// ---------------------------------------------------------------------
// Results
// ---------------------------------------------------------------------

/// Set up an allowed root that contains one ordinary file, one symlink to a
/// file outside the root, and one symlink to the outside directory itself.
/// Returns the allowed root and the outside root, in that order.
fn root_with_escapes(needle: &str) -> (TempDir, TempDir) {
    let root = TempDir::new().expect("allowed root");
    let outside = TempDir::new().expect("outside root");

    fs::write(root.path().join("inside-visible.txt"), needle).expect("write the inside file");
    fs::write(outside.path().join("outside-secret.txt"), needle).expect("write the outside file");

    std::os::unix::fs::symlink(
        outside.path().join("outside-secret.txt"),
        root.path().join("escape-link.txt"),
    )
    .expect("create the escaping file symlink");
    std::os::unix::fs::symlink(outside.path(), root.path().join("escape-dir"))
        .expect("create the escaping directory symlink");

    (root, outside)
}

#[track_caller]
fn assert_result_stays_inside(text: &str, tool: &str) {
    assert!(
        text.contains("inside-visible.txt"),
        "{tool} must still report the entry inside the root, got: {text}"
    );
    assert!(
        !text.contains("escape-link"),
        "{tool} must omit a file symlink that leaves the root, got: {text}"
    );
    assert!(
        !text.contains("escape-dir"),
        "{tool} must omit a directory symlink that leaves the root, got: {text}"
    );
    assert!(
        !text.contains("outside-secret"),
        "{tool} must omit every path outside the root, got: {text}"
    );
}

#[tokio::test]
async fn list_directory_omits_entries_outside_the_allowlist() {
    let (root, _outside) = root_with_escapes("NEEDLE");

    let registry = registry_rooted_at(root.path());
    let response = registry
        .execute_tool(
            "fileio_list_directory",
            &json!({"path": root.path().to_string_lossy(), "recursive": true}),
        )
        .await
        .expect("listing the allowed root must succeed");

    assert_result_stays_inside(&body_text(&response), "fileio_list_directory");
}

#[tokio::test]
async fn find_files_omits_matches_outside_the_allowlist() {
    let (root, _outside) = root_with_escapes("NEEDLE");

    let registry = registry_rooted_at(root.path());
    let response = registry
        .execute_tool(
            "fileio_find_files",
            &json!({"pattern": "*", "root": root.path().to_string_lossy()}),
        )
        .await
        .expect("finding files under the allowed root must succeed");

    assert_result_stays_inside(&body_text(&response), "fileio_find_files");
}

#[tokio::test]
async fn find_in_files_omits_matches_outside_the_allowlist() {
    let (root, _outside) = root_with_escapes("NEEDLE");

    let registry = registry_rooted_at(root.path());
    let response = registry
        .execute_tool(
            "fileio_find_in_files",
            &json!({"pattern": "NEEDLE", "path": root.path().to_string_lossy()}),
        )
        .await
        .expect("searching under the allowed root must succeed");

    assert_result_stays_inside(&body_text(&response), "fileio_find_in_files");
}

#[tokio::test]
async fn blocked_subtree_inside_an_allowed_root_is_omitted_from_listings() {
    let root = TempDir::new().expect("allowed root");
    let blocked = root.path().join("blocked");
    fs::create_dir_all(&blocked).expect("create the blocked subtree");
    fs::write(root.path().join("inside-visible.txt"), "NEEDLE").expect("write the inside file");
    fs::write(blocked.join("outside-secret.txt"), "NEEDLE").expect("write the blocked file");

    let guard = PathGuard::with_roots_and_blocks(
        &[root.path().to_string_lossy().into_owned()],
        &[format!("{}/", blocked.display())],
        None,
    );
    let registry = ToolRegistry::with_guard(guard);

    let listed = registry
        .execute_tool(
            "fileio_list_directory",
            &json!({"path": root.path().to_string_lossy(), "recursive": true}),
        )
        .await
        .expect("listing the allowed root must succeed");
    assert_result_stays_inside(&body_text(&listed), "fileio_list_directory");

    let found = registry
        .execute_tool(
            "fileio_find_files",
            &json!({"pattern": "*", "root": root.path().to_string_lossy()}),
        )
        .await
        .expect("finding files under the allowed root must succeed");
    assert_result_stays_inside(&body_text(&found), "fileio_find_files");

    let grepped = registry
        .execute_tool(
            "fileio_find_in_files",
            &json!({"pattern": "NEEDLE", "path": root.path().to_string_lossy()}),
        )
        .await
        .expect("searching under the allowed root must succeed");
    assert_result_stays_inside(&body_text(&grepped), "fileio_find_in_files");
}

/// `fileio_get_current_directory` returns a path, so the allowlist applies to
/// it like any other result. With the working directory outside every root,
/// there is no reachable working directory to report, and every relative path
/// is refused anyway.
#[tokio::test]
async fn current_directory_outside_the_allowlist_reports_not_found() {
    let root = TempDir::new().expect("allowed root");
    let registry = registry_rooted_at(root.path());

    // The test process runs in the crate directory, which is not the root.
    let result = registry
        .execute_tool("fileio_get_current_directory", &json!({}))
        .await;

    assert_reported_not_found(result, "the working directory outside every root");
}

// ---------------------------------------------------------------------
// The guard's own decision
// ---------------------------------------------------------------------

#[test]
fn path_outside_every_root_is_refused() {
    let root = TempDir::new().expect("allowed root");
    let outside = TempDir::new().expect("outside root");
    let guard = guard_rooted_at(root.path());

    assert!(
        !guard.refuses(&at(root.path(), "file.txt")),
        "a path inside the root must be permitted"
    );
    assert!(
        guard.refuses(&at(outside.path(), "file.txt")),
        "a path outside every root must be refused"
    );
    assert!(
        guard.refuses("/etc/shadow"),
        "a system path outside every root must be refused"
    );
}

#[test]
fn sibling_directory_sharing_a_root_name_prefix_is_refused() {
    let parent = TempDir::new().expect("parent of both roots");
    let root = parent.path().join("workspace");
    let sibling = parent.path().join("workspace-private");
    fs::create_dir_all(&root).expect("create the allowed root");
    fs::create_dir_all(&sibling).expect("create the sibling");

    let guard = guard_rooted_at(&root);

    assert!(
        !guard.refuses(&at(&root, "notes.txt")),
        "a path inside the root must be permitted"
    );
    assert!(
        guard.refuses(&at(&sibling, "notes.txt")),
        "a sibling that only shares a name prefix must be refused"
    );
}

/// Every operation expands `$VAR` with `shellexpand::full` before it touches
/// the filesystem. The guard has to expand the same way, or a caller names one
/// path to the guard and a different one to the operation.
#[test]
fn environment_variable_in_a_path_is_expanded_before_the_check() {
    let Some(home) = std::env::var_os("HOME") else {
        eprintln!("SKIP environment_variable_in_a_path_is_expanded_before_the_check: no HOME");
        return;
    };
    let home = std::path::PathBuf::from(home);
    let working = std::env::current_dir().expect("a working directory");
    if home.starts_with(&working) {
        eprintln!(
            "SKIP environment_variable_in_a_path_is_expanded_before_the_check: \
             HOME is inside the working directory"
        );
        return;
    }

    // "$HOME/..." has no leading separator, so a guard that leaves it alone
    // resolves it under the working directory, which is the only root here.
    let guard = PathGuard::with_roots(&[working.to_string_lossy().into_owned()]);

    assert!(
        guard.refuses("$HOME/.ssh/id_ed25519"),
        "a path that starts with an environment variable must be expanded \
         before the check, not treated as a relative name"
    );
    assert!(
        guard.refuses("${HOME}/.ssh/id_ed25519"),
        "the braced form must be expanded too"
    );
}

/// The default-value form needs no environment variable at all, so it does
/// not depend on what the caller can set.
#[test]
fn default_value_expansion_in_a_path_is_expanded_before_the_check() {
    let root = TempDir::new().expect("allowed root");
    let guard = guard_rooted_at(root.path());

    // "${UNSET:-..}" expands to "..", so this walks out of the root twice.
    let payload = format!(
        "{}/${{FILEIO_UNSET_TEST_VAR:-..}}/${{FILEIO_UNSET_TEST_VAR:-..}}/etc/passwd",
        root.path().display()
    );
    assert!(
        guard.refuses(&payload),
        "a default-value expansion must be expanded before the check"
    );

    // The same shape that stays inside the root is still reachable, so the
    // refusal above is about where it lands and not about the syntax.
    let inside = format!(
        "{}/${{FILEIO_UNSET_TEST_VAR:-sub}}/file.txt",
        root.path().display()
    );
    assert!(
        !guard.refuses(&inside),
        "a default-value expansion that stays inside the root must be permitted"
    );
}

#[test]
fn empty_allowlist_refuses_every_path() {
    let root = TempDir::new().expect("a directory that is not allowed");
    let empty: [String; 0] = [];
    let guard = PathGuard::with_roots(&empty);

    assert!(
        guard.refuses(&at(root.path(), "file.txt")),
        "an empty allowlist must refuse every path"
    );
    assert!(guard.refuses("/"), "an empty allowlist must refuse '/'");
}

#[test]
fn path_under_an_unreadable_directory_is_refused() {
    if nix::unistd::geteuid().is_root() {
        eprintln!("SKIP path_under_an_unreadable_directory_is_refused: root ignores file modes");
        return;
    }

    use std::os::unix::fs::PermissionsExt;

    let root = TempDir::new().expect("allowed root");
    let locked = root.path().join("locked");
    fs::create_dir_all(&locked).expect("create the locked directory");
    let hidden = locked.join("hidden.txt");
    fs::write(&hidden, "contents").expect("write inside the locked directory");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000))
        .expect("make the directory unreadable");

    let guard = guard_rooted_at(root.path());
    let refused = guard.refuses(&hidden.to_string_lossy());

    // Restore the mode so the temporary directory can be removed.
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o700))
        .expect("restore the directory mode");

    assert!(
        refused,
        "a path the guard cannot resolve must be refused, even inside a root"
    );
}

#[test]
fn legacy_block_path_flag_is_accepted_with_a_deprecation_warning() {
    let root = TempDir::new().expect("allowed root");
    let blocked = root.path().join("blocked");
    fs::create_dir_all(&blocked).expect("create the blocked subtree");

    let block_paths = vec![format!("{}/", blocked.display())];
    let warning = legacy_block_flag_warning(&block_paths, None)
        .expect("using --block-path must produce a deprecation warning");
    assert!(
        warning.to_lowercase().contains("deprecated"),
        "the warning must say the flag is deprecated, got: {warning}"
    );
    assert!(
        warning.contains("--allow-path"),
        "the warning must name the replacement flag, got: {warning}"
    );
    assert!(
        legacy_block_flag_warning(&[], None).is_none(),
        "no warning is due when neither legacy flag is used"
    );

    let guard = PathGuard::with_roots_and_blocks(
        &[root.path().to_string_lossy().into_owned()],
        &block_paths,
        None,
    );
    assert!(
        guard.refuses(&at(&blocked, "secret.txt")),
        "a legacy --block-path entry must still be refused inside an allowed root"
    );
    assert!(
        !guard.refuses(&at(root.path(), "notes.txt")),
        "the rest of the allowed root must stay reachable"
    );
}

// ---------------------------------------------------------------------
// Positive control
// ---------------------------------------------------------------------

/// Without this, every refusal test above would also pass for a guard that
/// refuses everything.
#[tokio::test]
async fn paths_inside_the_allowlist_are_reachable() {
    let root = TempDir::new().expect("allowed root");
    let target = root.path().join("notes.txt");
    let registry = registry_rooted_at(root.path());

    registry
        .execute_tool(
            "fileio_write_file",
            &json!({"path": target.to_string_lossy(), "content": "hello\n"}),
        )
        .await
        .expect("a write inside the root must succeed");
    assert_eq!(
        fs::read_to_string(&target).expect("read the written file"),
        "hello\n"
    );

    let read = registry
        .execute_tool(
            "fileio_read_lines",
            &json!({"path": target.to_string_lossy()}),
        )
        .await
        .expect("a read inside the root must succeed");
    assert!(body_text(&read).contains("hello"));

    let listed = registry
        .execute_tool(
            "fileio_list_directory",
            &json!({"path": root.path().to_string_lossy()}),
        )
        .await
        .expect("a listing inside the root must succeed");
    assert!(body_text(&listed).contains("notes.txt"));
}

// ---------------------------------------------------------------------
// Where an operation acts is not always where the argument points
// ---------------------------------------------------------------------

/// `mktemp` creates in the template's parent directory. A template with no
/// separator has an empty parent, which is the process working directory,
/// not the system temporary directory.
#[tokio::test]
async fn create_temporary_with_a_bare_template_is_refused_when_the_working_directory_is_outside() {
    let temp_root = std::env::temp_dir();
    let working = std::env::current_dir().expect("a working directory");
    if working.starts_with(&temp_root) {
        eprintln!("SKIP bare-template test: the working directory is inside the temp root");
        return;
    }

    // The system temporary directory is allowed; the working directory is not.
    let registry = registry_rooted_at(&temp_root);
    let before = entries_of(&working);

    let result = registry
        .execute_tool(
            "fileio_create_temporary",
            &json!({"type": "file", "template": "probe-XXXXXX"}),
        )
        .await;

    // Remove anything the call left behind before asserting, so a failure
    // does not litter the directory the test ran in.
    for path in entries_of(&working) {
        if !before.contains(&path) {
            let _ = fs::remove_file(&path);
            let _ = fs::remove_dir_all(&path);
        }
    }

    assert_reported_not_found(
        result,
        "a bare mktemp template with the working directory outside",
    );
}

fn entries_of(dir: &Path) -> Vec<std::path::PathBuf> {
    fs::read_dir(dir)
        .expect("read the directory")
        .map(|entry| entry.expect("a directory entry").path())
        .collect()
}

/// `readlink` returns the link's immediate target. The argument check
/// resolves the whole chain, so a link whose final target is inside a root
/// can still name an immediate target outside one.
#[tokio::test]
async fn read_symbolic_link_target_outside_the_allowlist_is_refused() {
    let root = TempDir::new().expect("allowed root");
    let outside = TempDir::new().expect("outside root");

    fs::write(root.path().join("readme.md"), "inside").expect("write the inside file");
    // An ordinary convenience symlink outside the root, pointing back into it.
    std::os::unix::fs::symlink(root.path(), outside.path().join("back"))
        .expect("create the outside symlink");
    // The chain ends inside the root, so the argument check permits it.
    let link = root.path().join("hop");
    std::os::unix::fs::symlink(outside.path().join("back").join("readme.md"), &link)
        .expect("create the hopping symlink");

    let registry = registry_rooted_at(root.path());
    let result = registry
        .execute_tool(
            "fileio_read_symbolic_link",
            &json!({"path": link.to_string_lossy()}),
        )
        .await;

    assert_reported_not_found(result, "a link whose immediate target is outside the root");
}

/// `dirname` of an allowlist root is the root's parent, which is outside the
/// set. A result is checked like any other path.
#[tokio::test]
async fn dirname_of_an_allowlist_root_is_refused() {
    let root = TempDir::new().expect("allowed root");
    let registry = registry_rooted_at(root.path());

    let result = registry
        .execute_tool(
            "fileio_get_dirname",
            &json!({"path": root.path().to_string_lossy()}),
        )
        .await;

    assert_reported_not_found(result, "the parent of an allowlist root");
}

/// A symbolic link has to point at the path the guard approved, not at the
/// text the caller typed.
#[tokio::test]
async fn symbolic_link_points_at_the_expanded_target_the_guard_checked() {
    let root = TempDir::new().expect("allowed root");
    let target = root.path().join("real.txt");
    fs::write(&target, "contents").expect("write the target file");
    let link = root.path().join("link");

    let unexpanded = format!(
        "${{FILEIO_UNSET_TEST_VAR:-{}}}/real.txt",
        root.path().display()
    );

    let registry = registry_rooted_at(root.path());
    registry
        .execute_tool(
            "fileio_create_symbolic_link",
            &json!({"target": unexpanded, "link_path": link.to_string_lossy()}),
        )
        .await
        .expect("a target inside the root must be allowed");

    assert_eq!(
        fs::read_link(&link).expect("read the link back"),
        target,
        "the link must point at the expanded target, not at the text typed"
    );
}
