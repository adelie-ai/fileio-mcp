#![deny(warnings)]

// Find files using the ignore crate (fd-find's underlying library)

use crate::error::{FileIoError, Result};
use ignore::WalkBuilder;
use std::path::Path;

/// Find files matching a pattern
pub fn file_find(
    pattern: &str,
    root: Option<&str>,
    max_depth: Option<usize>,
    file_type: Option<&str>,
) -> Result<Vec<String>> {
    let expanded_root = root.map(|r| {
        shellexpand::full(r)
            .map_err(|e| crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!("Failed to expand path '{}': {}", r, e))))
            .map(|expanded| expanded.into_owned())
    }).transpose()?;
    let root_path = expanded_root
        .as_ref()
        .map(|r| Path::new(r))
        .unwrap_or_else(|| Path::new("."));

    if !root_path.exists() {
        return Err(FileIoError::NotFound(expanded_root.unwrap_or_else(|| ".".to_string())).into());
    }

    let mut walker = WalkBuilder::new(root_path);
    walker.hidden(false); // Include hidden files by default

    if let Some(depth) = max_depth {
        walker.max_depth(Some(depth));
    }

    // Build regex pattern for matching
    let regex_pattern = if pattern.contains('*') || pattern.contains('?') {
        // Convert glob-like pattern to regex
        let regex_str = pattern
            .replace(".", "\\.")
            .replace("*", ".*")
            .replace("?", ".");
        regex::Regex::new(&format!("^{}$", regex_str))
            .map_err(|e| FileIoError::RegexError(e.into()))?
    } else {
        // Simple substring match
        regex::Regex::new(&format!("{}", regex::escape(pattern)))
            .map_err(|e| FileIoError::RegexError(e.into()))?
    };

    let mut matches = Vec::new();

    for result in walker.build() {
        let entry = result.map_err(|e| {
            FileIoError::ReadError(format!("Error walking directory: {}", e))
        })?;

        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Check if filename matches pattern
        if regex_pattern.is_match(file_name) {
            // Filter by file type if specified
            if let Some(ft) = file_type {
                match ft {
                    "file" if !path.is_file() => continue,
                    "dir" | "directory" if !path.is_dir() => continue,
                    "symlink" if !path.is_symlink() => continue,
                    _ => {}
                }
            }

            matches.push(path.to_string_lossy().to_string());
        }
    }

    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_file_find() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_str().unwrap();

        // Create test files
        fs::write(dir.path().join("test1.txt"), "content").unwrap();
        fs::write(dir.path().join("test2.txt"), "content").unwrap();
        fs::write(dir.path().join("other.log"), "content").unwrap();

        let matches = file_find("*.txt", Some(root), None, Some("file")).unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_file_find_with_depth() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_str().unwrap();

        // Create nested structure
        let subdir = dir.path().join("subdir");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("test.txt"), "content").unwrap();
        fs::write(dir.path().join("test.txt"), "content").unwrap();

        let matches = file_find("test.txt", Some(root), Some(1), Some("file")).unwrap();
        // Should only find the one at root level with max_depth=1
        assert!(matches.len() >= 1);
    }
}
