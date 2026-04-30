#![deny(warnings)]

//! Path guard: deny-list for sensitive filesystem paths.
//!
//! Denied paths are made invisible — reads return "not found", writes silently
//! succeed, directory listings omit entries. This prevents an LLM from knowing
//! the restriction exists.

use std::path::{Path, PathBuf};

/// A deny-list entry: either an exact file or a directory prefix.
#[derive(Debug, Clone)]
enum DenyEntry {
    /// Block access to this exact file path.
    File(PathBuf),
    /// Block access to anything under this directory (inclusive).
    Directory(PathBuf),
}

/// Immutable path guard built once at startup.
#[derive(Debug, Clone)]
pub struct PathGuard {
    entries: Vec<DenyEntry>,
}

/// Hardcoded sensitive paths. Entries ending with `/` are directory prefixes.
const DEFAULT_DENY: &[&str] = &[
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

impl PathGuard {
    /// Build a PathGuard from hardcoded defaults + optional CLI extras + optional blocklist file.
    pub fn new(
        extra_paths: &[String],
        block_file: Option<&str>,
    ) -> Self {
        let mut entries = Vec::new();

        // Load hardcoded defaults
        for pattern in DEFAULT_DENY {
            Self::add_pattern(&mut entries, pattern);
        }

        // Load CLI extras
        for pattern in extra_paths {
            Self::add_pattern(&mut entries, pattern);
        }

        // Load blocklist file
        if let Some(file_path) = block_file {
            // The blocklist file itself is denied.
            let expanded = shellexpand::tilde(file_path).into_owned();
            entries.push(DenyEntry::File(PathBuf::from(&expanded)));

            if let Ok(contents) = std::fs::read_to_string(&expanded) {
                for line in contents.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    Self::add_pattern(&mut entries, line);
                }
            } else {
                eprintln!("warning: could not read block-file: {}", file_path);
            }
        }

        Self { entries }
    }

    fn add_pattern(entries: &mut Vec<DenyEntry>, pattern: &str) {
        // `shellexpand::tilde` rather than `pattern.replace('~', home)` — the
        // literal replace substitutes every `~` in the string, not just a
        // leading one, which is a footgun for patterns that contain `~` mid-path.
        let expanded = shellexpand::tilde(pattern).into_owned();
        if expanded.ends_with('/') {
            entries.push(DenyEntry::Directory(PathBuf::from(&expanded)));
        } else {
            entries.push(DenyEntry::File(PathBuf::from(&expanded)));
        }
    }

    /// Check if a path is denied.
    ///
    /// Tilde-expands the input before canonicalizing so callers can pass paths
    /// like `~/.ssh/id_rsa` directly. Without this expansion, an adversarial
    /// caller could bypass the deny-list by passing `~/...` strings — the
    /// downstream operation crate calls `shellexpand::full` after the guard
    /// check and accesses the real file (issue #2). $HOME / env-var inputs are
    /// not handled here because env vars are part of the trusted startup
    /// environment, not attacker-controlled.
    pub fn is_denied(&self, path: &str) -> bool {
        let expanded = shellexpand::tilde(path);
        let canonical = canonicalize_best_effort(expanded.as_ref());
        self.is_denied_canonical(&canonical)
    }

    /// Check if an already-canonicalized path is denied.
    pub fn is_denied_canonical(&self, canonical: &Path) -> bool {
        for entry in &self.entries {
            match entry {
                DenyEntry::File(denied) => {
                    if canonical == denied {
                        return true;
                    }
                }
                DenyEntry::Directory(denied) => {
                    if canonical.starts_with(denied) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Filter a list of paths, returning only non-denied ones.
    /// Each path should already be shell-expanded.
    pub fn filter_paths<'a>(&self, paths: &[&'a str]) -> Vec<&'a str> {
        paths
            .iter()
            .filter(|p| !self.is_denied(p))
            .copied()
            .collect()
    }
}

/// Canonicalize a path, falling back to best-effort if the path doesn't exist.
/// Walks up to the nearest existing ancestor, canonicalizes that, then appends
/// the remaining suffix.
fn canonicalize_best_effort(path: &str) -> PathBuf {
    let p = Path::new(path);

    // Fast path: file exists, full canonicalization works
    if let Ok(canonical) = std::fs::canonicalize(p) {
        return canonical;
    }

    // Walk up to find the nearest existing ancestor.
    let mut existing = p.to_path_buf();
    let mut suffix_parts: Vec<std::ffi::OsString> = Vec::new();

    while let Some(parent) = existing.parent().map(Path::to_path_buf) {
        if let Some(file_name) = existing.file_name() {
            suffix_parts.push(file_name.to_os_string());
        }
        existing = parent;
        if let Ok(canonical) = std::fs::canonicalize(&existing) {
            let mut result = canonical;
            for part in suffix_parts.into_iter().rev() {
                result.push(part);
            }
            return result;
        }
    }

    // Last resort: return the path as-is.
    p.to_path_buf()
}

impl Default for PathGuard {
    fn default() -> Self {
        Self::new(&[], None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> String {
        std::env::var("HOME").unwrap()
    }

    #[test]
    fn denies_ssh_directory() {
        let guard = PathGuard::default();
        assert!(guard.is_denied(&format!("{}/.ssh/id_ed25519", home())));
        assert!(guard.is_denied(&format!("{}/.ssh/known_hosts", home())));
        assert!(guard.is_denied(&format!("{}/.ssh/config", home())));
    }

    #[test]
    fn denies_aws_credentials() {
        let guard = PathGuard::default();
        assert!(guard.is_denied(&format!("{}/.aws/credentials", home())));
        assert!(guard.is_denied(&format!("{}/.aws/config", home())));
    }

    #[test]
    fn denies_secrets_toml() {
        let guard = PathGuard::default();
        assert!(guard.is_denied(&format!(
            "{}/.config/desktop-assistant/secrets.toml",
            home()
        )));
    }

    #[test]
    fn denies_etc_shadow() {
        let guard = PathGuard::default();
        assert!(guard.is_denied("/etc/shadow"));
    }

    #[test]
    fn allows_normal_paths() {
        let guard = PathGuard::default();
        assert!(!guard.is_denied("/tmp/test.txt"));
        assert!(!guard.is_denied(&format!("{}/projects/foo.rs", home())));
        assert!(!guard.is_denied(&format!("{}/.config/some-app/config.toml", home())));
    }

    #[test]
    fn denies_exact_file_match() {
        let guard = PathGuard::default();
        assert!(guard.is_denied(&format!("{}/.netrc", home())));
        assert!(guard.is_denied(&format!("{}/.npmrc", home())));
    }

    #[test]
    fn extra_paths_are_denied() {
        let guard = PathGuard::new(&["/tmp/secret-dir/".into(), "/tmp/secret-file.txt".into()], None);
        assert!(guard.is_denied("/tmp/secret-dir/foo.txt"));
        assert!(guard.is_denied("/tmp/secret-file.txt"));
        assert!(!guard.is_denied("/tmp/other.txt"));
    }

    #[test]
    fn blocklist_file_loaded_and_self_denied() {
        let dir = std::env::temp_dir().join("fileio_blocklist_test");
        let _ = std::fs::create_dir_all(&dir);
        let blocklist = dir.join("blocklist.txt");

        std::fs::write(
            &blocklist,
            "# comment\n/tmp/blocked-by-file/\n/tmp/blocked-file.txt\n",
        )
        .unwrap();

        let guard = PathGuard::new(&[], Some(blocklist.to_str().unwrap()));

        // Entries from the blocklist file
        assert!(guard.is_denied("/tmp/blocked-by-file/secret.key"));
        assert!(guard.is_denied("/tmp/blocked-file.txt"));

        // The blocklist file itself is denied
        assert!(guard.is_denied(blocklist.to_str().unwrap()));

        // Other paths still allowed
        assert!(!guard.is_denied("/tmp/other.txt"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn symlink_to_denied_path_is_denied() {
        let dir = std::env::temp_dir().join("fileio_symlink_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Create a real file in ~/.ssh (if it exists) or skip
        let ssh_dir = PathBuf::from(home()).join(".ssh");
        if !ssh_dir.exists() {
            eprintln!("SKIP: ~/.ssh does not exist");
            return;
        }

        // Create a symlink to ~/.ssh
        let link = dir.join("sneaky_link");
        std::os::unix::fs::symlink(&ssh_dir, &link).unwrap();

        let guard = PathGuard::default();
        let link_target = format!("{}/known_hosts", link.display());
        assert!(
            guard.is_denied(&link_target),
            "symlink to ~/.ssh should be denied"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn canonicalize_best_effort_works_for_nonexistent() {
        // /tmp exists, so /tmp/nonexistent/deep/path should canonicalize
        // the /tmp part and append the rest
        let result = canonicalize_best_effort("/tmp/nonexistent_test_xyz/deep/path.txt");
        assert!(result.to_str().unwrap().contains("nonexistent_test_xyz"));
        assert!(result.to_str().unwrap().contains("deep"));
    }

    #[test]
    fn tilde_expansion_in_deny_list() {
        let guard = PathGuard::new(&["~/custom-secret.txt".into()], None);
        assert!(guard.is_denied(&format!("{}/custom-secret.txt", home())));
    }

    /// Regression: an adversarial caller passing a tilde-prefixed *input* to
    /// `is_denied` must still be denied. Without input-side expansion, the
    /// downstream operations crate calls `shellexpand::full` after the guard
    /// check and accesses the real file (issue #2).
    #[test]
    fn denies_tilde_prefixed_input() {
        let guard = PathGuard::default();
        assert!(
            guard.is_denied("~/.ssh/id_ed25519"),
            "tilde-prefixed inputs must be expanded before matching"
        );
        assert!(guard.is_denied("~/.aws/credentials"));
        assert!(guard.is_denied("~/.config/desktop-assistant/secrets.toml"));
        assert!(guard.is_denied("~/.netrc"));
    }

    /// Regression: an extra-path pattern set via `~/...` must match both the
    /// tilde-prefixed input form *and* the absolute form.
    #[test]
    fn pattern_input_symmetry_for_tilde() {
        let guard = PathGuard::new(&["~/private/".into()], None);
        assert!(
            guard.is_denied("~/private/file.txt"),
            "tilde input must match tilde pattern"
        );
        assert!(
            guard.is_denied(&format!("{}/private/file.txt", home())),
            "absolute input must match tilde pattern"
        );
    }

    /// Allowed tilde-prefixed inputs stay allowed.
    #[test]
    fn allows_tilde_prefixed_safe_paths() {
        let guard = PathGuard::default();
        assert!(!guard.is_denied("~/projects/foo.rs"));
        assert!(!guard.is_denied("~/Documents/report.md"));
    }

    /// `~` mid-path is not expanded by `shellexpand::tilde` — it only expands
    /// a leading `~`. Verify a pattern like `/tmp/~foo/` is stored verbatim,
    /// not silently rewritten to `/tmp/<home>foo/` (which the previous
    /// `replace('~', home)` implementation would have done).
    #[test]
    fn mid_path_tilde_in_pattern_is_literal() {
        let guard = PathGuard::new(&["/tmp/~tilde-dir/".into()], None);
        assert!(guard.is_denied("/tmp/~tilde-dir/file.txt"));
        // The misexpansion that would have happened with the old code:
        let misexpanded = format!("/tmp/{}foo/", home());
        assert!(!guard.is_denied(&misexpanded));
    }
}
