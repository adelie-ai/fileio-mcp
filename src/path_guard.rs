#![deny(warnings)]

//! Path guard: an allowlist of the filesystem roots this server may reach.
//!
//! A path inside a root is reachable. Every other path does not exist, as far
//! as this server is concerned. A refused argument comes back as "not found",
//! and a refused entry is dropped from a result before the result leaves the
//! server. Both checks are needed: an argument check on its own lets a listing
//! of a permitted directory disclose a path outside the set.
//!
//! The guard resolves a path first and compares it to the roots second, so a
//! symlink out of a root and a `..` traversal are both decided on the real
//! path. It fails closed: anything it cannot positively identify is refused.
//!
//! See `docs/path_safety.md` for the full design, including where the roots
//! come from, the fail-closed rules, and the check-then-use race this guard
//! does not close.

use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use mcp_core::telemetry::metrics::{self, Label};

/// Environment variable naming the allowlist roots, separated by `:`.
///
/// Set it to point a run at a directory made to be thrown away. A test that
/// sets it cannot reach a real home directory, whatever the code does.
pub const ALLOW_PATHS_ENV: &str = "FILEIO_MCP_ALLOW_PATHS";

/// Directories under `HOME` in the built-in default allowlist. A starting
/// point for a desktop install, not a recommendation: name the directories
/// the work needs with `--allow-path`.
const DEFAULT_HOME_ROOTS: &[&str] = &["Documents", "Downloads", "Desktop", "Projects"];

/// Sensitive paths that stay unreachable even when an operator allows a root
/// wide enough to contain them. Entries ending with `/` are directory
/// prefixes.
const DEFAULT_BLOCKS: &[&str] = &[
    "~/.ssh/",
    "~/.gnupg/",
    "~/.gpg/",
    "~/.aws/",
    "~/.config/desktop-assistant/secrets.toml",
    "~/.netrc",
    "~/.npmrc",
    "~/.docker/config.json",
    "~/.kube/config",
    "~/.config/gh/hosts.yml",
    "~/.local/share/keyrings/",
    "~/.password-store/",
    "/etc/shadow",
    "/etc/gshadow",
    "/etc/security/",
];

/// Most symbolic links one resolution may follow before the guard gives up
/// and refuses the path. A link loop is the case this bounds.
const MAX_LINK_HOPS: usize = 40;

/// A blocked entry: either an exact file or a directory prefix.
#[derive(Debug, Clone)]
enum BlockEntry {
    /// Block this exact file path.
    File(PathBuf),
    /// Block anything under this directory, the directory included.
    Directory(PathBuf),
}

/// Why the guard refused a path.
///
/// This is the bounded `reason` a refusal records, never the path and never
/// which entry matched. An allowlist root and a block entry are both paths,
/// and D10 keeps a path off every metric label and every log field alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefusalReason {
    /// The resolved path is under no allowlist root.
    OutsideAllowlist,
    /// The resolved path is inside a root but under a block entry.
    Blocked,
    /// The guard could not work out what the path is, so it refused.
    Unresolvable,
}

impl RefusalReason {
    fn as_label(self) -> &'static str {
        match self {
            RefusalReason::OutsideAllowlist => "outside_allowlist",
            RefusalReason::Blocked => "blocked",
            RefusalReason::Unresolvable => "unresolvable",
        }
    }
}

/// Metric name for a path the guard refused, labelled by [`RefusalReason`].
///
/// mcp-core's own dispatch counts every tool call by name and outcome
/// (`mcp.tools.call`), but it cannot see a refusal: the server answers "not
/// found", so the call looks ordinary from the dispatch layer's point of
/// view. This is the one place a refusal becomes observable at all.
const GUARD_REJECTIONS_METRIC: &str = "fileio.guard.rejections";

/// Record one refusal. `reason` is a three-value enum, so the label can never
/// grow past the registry's cardinality cap.
fn record_refusal(reason: RefusalReason) {
    let label = reason.as_label();
    metrics::increment(GUARD_REJECTIONS_METRIC, &[Label::new("reason", label)]);
    tracing::debug!(reason = label, "path refused by guard");
}

/// Immutable path guard built once at startup.
#[derive(Debug, Clone)]
pub struct PathGuard {
    /// Resolved allowlist roots. An empty list refuses every path.
    roots: Vec<PathBuf>,
    /// Resolved block entries, subtracted from the allowlist.
    blocks: Vec<BlockEntry>,
}

impl PathGuard {
    /// Build the guard from the server's own flags and environment.
    ///
    /// The allowlist comes from the first source that is set: `allow_paths`
    /// (the `--allow-path` flag), then [`ALLOW_PATHS_ENV`], then the built-in
    /// default set. An environment variable that is set but empty yields an
    /// empty allowlist, which refuses every path rather than falling back to
    /// a wider default.
    ///
    /// `block_paths` and `block_file` are the deprecated `--block-path` and
    /// `--block-file` flags. They still subtract from the allowlist, so an
    /// existing deployment keeps its restrictions.
    pub fn from_flags(
        allow_paths: &[String],
        block_paths: &[String],
        block_file: Option<&str>,
    ) -> Self {
        Self {
            roots: resolve_roots(&configured_roots(allow_paths)),
            blocks: collect_blocks(block_paths, block_file),
        }
    }

    /// Build a guard whose allowlist is exactly `roots`, with only the
    /// built-in block entries subtracted.
    ///
    /// Neither the environment nor the default root set is consulted, so a
    /// caller that names a temporary directory here cannot reach a real path.
    pub fn with_roots<S: AsRef<str>>(roots: &[S]) -> Self {
        Self {
            roots: resolve_roots(roots),
            blocks: collect_blocks(&[], None),
        }
    }

    /// Build a guard whose allowlist is exactly `roots`, minus the legacy
    /// block entries as well as the built-in ones.
    pub fn with_roots_and_blocks<S: AsRef<str>>(
        roots: &[S],
        block_paths: &[String],
        block_file: Option<&str>,
    ) -> Self {
        Self {
            roots: resolve_roots(roots),
            blocks: collect_blocks(block_paths, block_file),
        }
    }

    /// Whether the guard refuses `path`.
    ///
    /// The path is resolved first and compared second. A path the guard
    /// cannot resolve is refused, because the guard cannot tell what it is.
    ///
    /// This is the single decision point for every caller in this crate, on
    /// arguments and on results alike, so it is the one place that records
    /// the refusal rather than each of the call sites across `tools.rs`.
    pub fn refuses(&self, path: &str) -> bool {
        let Some(resolved) = resolve(path) else {
            record_refusal(RefusalReason::Unresolvable);
            return true;
        };

        if !self.roots.iter().any(|root| resolved.starts_with(root)) {
            record_refusal(RefusalReason::OutsideAllowlist);
            return true;
        }

        let blocked = self.blocks.iter().any(|entry| match entry {
            BlockEntry::File(blocked) => resolved == *blocked,
            BlockEntry::Directory(blocked) => resolved.starts_with(blocked),
        });
        if blocked {
            record_refusal(RefusalReason::Blocked);
            return true;
        }

        false
    }
}

impl Default for PathGuard {
    fn default() -> Self {
        Self::from_flags(&[], &[], None)
    }
}

/// The deprecation notice due when a caller still uses `--block-path` or
/// `--block-file`, or `None` when neither is set.
///
/// A pure function rather than a log call inside the guard, so the notice is
/// testable and the binary decides where it goes.
pub fn legacy_block_flag_warning(
    block_paths: &[String],
    block_file: Option<&str>,
) -> Option<String> {
    if block_paths.is_empty() && block_file.is_none() {
        return None;
    }
    Some(
        "--block-path and --block-file are deprecated. The guard is an allowlist now: \
         name the directories this server may reach with --allow-path, or with the \
         FILEIO_MCP_ALLOW_PATHS environment variable. The block entries still apply \
         on top of the allowlist. A later release refuses these flags."
            .to_string(),
    )
}

/// The allowlist roots as configured, before resolution.
fn configured_roots(allow_paths: &[String]) -> Vec<String> {
    if !allow_paths.is_empty() {
        return allow_paths.to_vec();
    }
    if let Some(value) = std::env::var_os(ALLOW_PATHS_ENV) {
        return parse_root_list(&value.to_string_lossy());
    }
    default_roots()
}

/// Split a `:`-separated root list. An entry that is empty or only spaces is
/// dropped, so a trailing separator does not become a root.
fn parse_root_list(value: &str) -> Vec<String> {
    value
        .split(':')
        .map(str::trim)
        .filter(|root| !root.is_empty())
        .map(str::to_string)
        .collect()
}

/// The built-in default allowlist.
fn default_roots() -> Vec<String> {
    let mut roots = vec![std::env::temp_dir().to_string_lossy().into_owned()];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        for name in DEFAULT_HOME_ROOTS {
            roots.push(home.join(name).to_string_lossy().into_owned());
        }
    }
    roots
}

/// Resolve every root, dropping any the guard cannot identify.
///
/// A root kept as an unresolved string could match a path it does not name,
/// so a root that cannot be resolved is dropped rather than trusted.
fn resolve_roots<S: AsRef<str>>(roots: &[S]) -> Vec<PathBuf> {
    let mut resolved = Vec::with_capacity(roots.len());
    for root in roots {
        let root = root.as_ref().trim();
        if root.is_empty() {
            continue;
        }
        match resolve(root) {
            Some(path) => resolved.push(path),
            // The reason and the outcome belong on this line; the root does
            // not (D10 - a path is content, not an id, a count or a duration).
            None => tracing::warn!(
                outcome = "allow_root_dropped",
                "an allowlist root could not be resolved; dropping it"
            ),
        }
    }
    if resolved.is_empty() {
        tracing::warn!(
            outcome = "allowlist_empty",
            "the path allowlist is empty; every path is refused"
        );
    }
    resolved
}

/// The built-in block entries, plus the deprecated flag entries.
fn collect_blocks(block_paths: &[String], block_file: Option<&str>) -> Vec<BlockEntry> {
    let mut entries = Vec::new();

    for pattern in DEFAULT_BLOCKS {
        add_block(&mut entries, pattern);
    }
    for pattern in block_paths {
        add_block(&mut entries, pattern);
    }

    if let Some(file_path) = block_file {
        // The block file itself is blocked, so its contents stay unreadable.
        add_block(&mut entries, file_path);

        // Read the resolved path, so the file the guard blocks and the file it
        // reads are the same one.
        let Some(resolved) = resolve(file_path) else {
            tracing::warn!(
                outcome = "block_file_not_loaded",
                "the configured block-file path could not be resolved; \
                 continuing without its entries"
            );
            return entries;
        };

        match std::fs::read_to_string(&resolved) {
            Ok(contents) => {
                for line in contents.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    add_block(&mut entries, line);
                }
            }
            // `outcome` names what the server does about it: it keeps running
            // with the built-in blocks and any --block-path entries, just
            // without this file's.
            Err(e) => tracing::warn!(
                reason = ?e.kind(),
                outcome = "block_file_not_loaded",
                "could not read the configured block-file; continuing without its entries"
            ),
        }
    }

    entries
}

/// Add one block pattern. A trailing `/` makes it a directory prefix.
fn add_block(entries: &mut Vec<BlockEntry>, pattern: &str) {
    // A `~` pattern with no HOME to expand would resolve against the working
    // directory and block a path nobody named. Drop it instead.
    if pattern.starts_with('~') && std::env::var_os("HOME").is_none() {
        return;
    }
    let is_directory = pattern.ends_with('/');
    let Some(resolved) = resolve(pattern) else {
        tracing::warn!(
            outcome = "block_entry_dropped",
            "a block entry could not be resolved; dropping it"
        );
        return;
    };
    entries.push(if is_directory {
        BlockEntry::Directory(resolved)
    } else {
        BlockEntry::File(resolved)
    });
}

/// One step of a path, once `.` has been dropped.
enum Step {
    /// `..`
    Parent,
    /// An ordinary component.
    Name(OsString),
}

/// Queue the steps of `path`, dropping `.`, the root and any prefix. The
/// caller decides where the walk starts.
fn queue_steps(queue: &mut VecDeque<Step>, path: &Path) {
    for component in path.components() {
        match component {
            Component::Normal(name) => queue.push_back(Step::Name(name.to_os_string())),
            Component::ParentDir => queue.push_back(Step::Parent),
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }
}

/// Resolve `path` to an absolute path with no symlink, `.` or `..` left in
/// it, or `None` when the guard cannot say what the path is.
///
/// A leading `~` and any `$VAR` expand first, the same way every operation
/// expands them.
///
/// `std::fs::canonicalize` cannot do this job on its own, because a path the
/// caller means to create does not exist yet. This walks the path one
/// component at a time instead:
///
/// - `..` removes the last resolved component, and does nothing at `/`, as in
///   the kernel. Resolving as we go is what makes this correct: `..` after a
///   symlink pops the link's target, not the link's parent.
/// - A symlink is replaced by its target, which is then resolved in turn. An
///   absolute target restarts the walk at `/`.
/// - A component that does not exist is kept as written. It cannot be a
///   symlink, so nothing is missed.
///
/// Anything else - an unreadable parent directory, a link loop, a working
/// directory that cannot be read, an expansion that fails - returns `None`,
/// and the caller refuses.
fn resolve(path: &str) -> Option<PathBuf> {
    // `shellexpand::full`, and not `tilde`, because that is what every
    // function under `src/operations` calls before it touches the filesystem.
    // Expanding less than the operations do would let a caller name one path
    // to the guard and a different one to the operation: `$HOME/.ssh/id_rsa`
    // reads as a relative name to `tilde` and as an absolute path to `full`.
    // An expansion that fails names no path the guard can identify, so it
    // refuses.
    let expanded = shellexpand::full(path).ok()?.into_owned();
    let input = Path::new(&expanded);

    let mut resolved = if input.is_absolute() {
        PathBuf::from("/")
    } else {
        std::fs::canonicalize(std::env::current_dir().ok()?).ok()?
    };

    let mut pending: VecDeque<Step> = VecDeque::new();
    queue_steps(&mut pending, input);

    let mut hops = 0usize;
    while let Some(step) = pending.pop_front() {
        let name = match step {
            Step::Parent => {
                resolved.pop();
                continue;
            }
            Step::Name(name) => name,
        };

        let candidate = resolved.join(&name);
        match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                hops += 1;
                if hops > MAX_LINK_HOPS {
                    return None;
                }
                let target = std::fs::read_link(&candidate).ok()?;
                if target.is_absolute() {
                    resolved = PathBuf::from("/");
                }
                let mut target_steps = VecDeque::new();
                queue_steps(&mut target_steps, &target);
                while let Some(step) = target_steps.pop_back() {
                    pending.push_front(step);
                }
            }
            // Exists and is not a link, or does not exist yet. Either way the
            // component is what it says it is.
            Ok(_) => resolved = candidate,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => resolved = candidate,
            // Most often EACCES on an unreadable parent: the guard cannot tell
            // whether this component is a symlink, so it refuses.
            Err(_) => return None,
        }
    }

    Some(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// The guard resolves before it compares, so `.` and `..` are gone by the
    /// time the comparison happens.
    #[test]
    fn resolve_normalizes_dot_and_parent() {
        let root = TempDir::new().expect("a temporary root");
        let nested = root.path().join("a").join("b");
        std::fs::create_dir_all(&nested).expect("create the nested directories");

        let awkward = format!("{}/a/./b/../b/c.txt", root.path().display());
        let expected = std::fs::canonicalize(root.path())
            .expect("canonical root")
            .join("a")
            .join("b")
            .join("c.txt");

        assert_eq!(resolve(&awkward), Some(expected));
    }

    /// `..` at `/` stays at `/`, as in the kernel, rather than underflowing.
    #[test]
    fn resolve_stops_at_the_filesystem_root() {
        assert_eq!(resolve("/../../../etc"), Some(PathBuf::from("/etc")));
    }

    /// A component that does not exist is kept as written. A write names a
    /// path before it exists, and the guard still has to decide about it.
    #[test]
    fn resolve_keeps_a_component_that_does_not_exist() {
        let root = TempDir::new().expect("a temporary root");
        let target = root.path().join("not-yet").join("file.txt");
        let expected = std::fs::canonicalize(root.path())
            .expect("canonical root")
            .join("not-yet")
            .join("file.txt");

        assert_eq!(resolve(&target.to_string_lossy()), Some(expected));
    }

    /// A relative symlink target resolves against the link's own directory,
    /// not against the working directory.
    #[test]
    fn resolve_follows_a_relative_symlink_target() {
        let root = TempDir::new().expect("a temporary root");
        let real = root.path().join("real.txt");
        std::fs::write(&real, "contents").expect("write the target file");
        let link = root.path().join("link.txt");
        std::os::unix::fs::symlink("real.txt", &link).expect("create the relative symlink");

        let expected = std::fs::canonicalize(&real).expect("canonical target");
        assert_eq!(resolve(&link.to_string_lossy()), Some(expected));
    }

    /// A link loop has no answer, so the guard gives up and the caller
    /// refuses the path.
    #[test]
    fn resolve_gives_up_on_a_symlink_loop() {
        let root = TempDir::new().expect("a temporary root");
        let first = root.path().join("first");
        let second = root.path().join("second");
        std::os::unix::fs::symlink(&second, &first).expect("create the first link");
        std::os::unix::fs::symlink(&first, &second).expect("create the second link");

        assert_eq!(resolve(&first.to_string_lossy()), None);
    }

    /// A `~` only expands at the front of a path. A `~` in the middle is an
    /// ordinary directory name. A `$VAR` expands wherever it appears, because
    /// that is what the operations do.
    #[test]
    fn resolve_expands_a_leading_tilde_and_environment_variables() {
        let Some(home) = std::env::var_os("HOME") else {
            eprintln!("SKIP resolve_expands_a_leading_tilde_and_environment_variables: no HOME");
            return;
        };
        let home = PathBuf::from(home);
        let expected_home = resolve(&home.join("does-not-exist-fileio-test").to_string_lossy());

        assert_eq!(resolve("~/does-not-exist-fileio-test"), expected_home);
        assert_eq!(resolve("$HOME/does-not-exist-fileio-test"), expected_home);
        assert_eq!(resolve("${HOME}/does-not-exist-fileio-test"), expected_home);

        // An undefined variable names no path the guard can identify.
        assert_eq!(resolve("$FILEIO_MCP_UNDEFINED_TEST_VAR/x"), None);

        let root = TempDir::new().expect("a temporary root");
        let mid = root.path().join("~tilde").join("file.txt");
        let expected = std::fs::canonicalize(root.path())
            .expect("canonical root")
            .join("~tilde")
            .join("file.txt");
        assert_eq!(resolve(&mid.to_string_lossy()), Some(expected));
    }

    /// A trailing separator, or a stray space, must not become a root that
    /// resolves to the working directory.
    #[test]
    fn parse_root_list_drops_empty_entries() {
        assert_eq!(
            parse_root_list("/one: /two :"),
            vec!["/one".to_string(), "/two".to_string()]
        );
        assert!(parse_root_list("").is_empty());
        assert!(parse_root_list(":::").is_empty());
    }

    /// The default set has to cover the system temporary directory, because
    /// that is where a caller puts scratch work.
    #[test]
    fn default_roots_cover_the_system_temporary_directory() {
        let guard = PathGuard::from_flags(&[], &[], None);
        let scratch = std::env::temp_dir().join("fileio-default-root-check.txt");
        assert!(
            !guard.refuses(&scratch.to_string_lossy()),
            "the default allowlist must cover the system temporary directory"
        );
    }

    /// A block file names more entries, and the file itself stays unreadable
    /// so the list cannot be read back.
    #[test]
    fn block_file_entries_and_the_file_itself_are_blocked() {
        let root = TempDir::new().expect("a temporary root");
        let blocked_dir = root.path().join("blocked");
        std::fs::create_dir_all(&blocked_dir).expect("create the blocked directory");
        let list = root.path().join("blocks.txt");
        std::fs::write(&list, format!("# a comment\n{}/\n", blocked_dir.display()))
            .expect("write the block file");

        let roots = [root.path().to_string_lossy().into_owned()];
        let guard = PathGuard::with_roots_and_blocks(&roots, &[], list.to_str());

        assert!(
            guard.refuses(&blocked_dir.join("secret.txt").to_string_lossy()),
            "an entry from the block file must be refused"
        );
        assert!(
            guard.refuses(&list.to_string_lossy()),
            "the block file itself must be refused"
        );
        assert!(
            !guard.refuses(&root.path().join("notes.txt").to_string_lossy()),
            "the rest of the root must stay reachable"
        );
    }

    /// Sum, across every label combination, how many times
    /// `GUARD_REJECTIONS_METRIC` has fired so far. A snapshot delta rather
    /// than an exact read: this binary's other unit tests share the same
    /// process-global registry (mcp-core's re-exported facade has no
    /// per-test handle to inject), and several of them run concurrently and
    /// also refuse paths. Only ever-increasing, so a `>=` comparison against
    /// a known number of refusals this test caused is exact enough to prove
    /// the wiring without being flaky under `cargo test`'s default
    /// parallelism.
    fn guard_refusal_total() -> u64 {
        mcp_core::telemetry::metrics::global()
            .snapshot()
            .counters
            .iter()
            .filter(|c| c.name == GUARD_REJECTIONS_METRIC)
            .map(|c| c.total)
            .sum()
    }

    /// Acceptance: a refusal increments the bounded `reason`-labelled counter,
    /// so an operator can see the guard working without the caller ever
    /// finding out. A permitted path must not move it.
    #[test]
    fn guard_refusal_metric_counts_refusals() {
        let root = TempDir::new().expect("a temporary root");
        let outside = TempDir::new().expect("a directory outside the root");
        let blocked = root.path().join("blocked");
        std::fs::create_dir_all(&blocked).expect("create the blocked directory");

        let roots = [root.path().to_string_lossy().into_owned()];
        let guard =
            PathGuard::with_roots_and_blocks(&roots, &[format!("{}/", blocked.display())], None);

        let before = guard_refusal_total();

        assert!(guard.refuses(&outside.path().join("a.txt").to_string_lossy()));
        assert!(guard.refuses(&blocked.join("b.txt").to_string_lossy()));
        assert!(!guard.refuses(&root.path().join("c.txt").to_string_lossy()));

        let after = guard_refusal_total();
        assert!(
            after >= before + 2,
            "expected the refusal counter to rise by at least 2 \
             (one outside the allowlist, one blocked), before={before} after={after}"
        );
    }
}
