#![deny(warnings)]

// Remove files or directories

use crate::error::{FileIoError, Result};
use globset::{Glob, GlobMatcher};
use std::fs;
use std::path::{Path, PathBuf};

/// Check if a string contains glob patterns
fn is_glob_pattern(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[') || s.contains('{')
}

/// Expand glob pattern to matching paths
fn expand_glob(pattern: &str) -> Result<Vec<PathBuf>> {
    let path = Path::new(pattern);
    let (base_dir, glob_str) = if let Some(parent) = path.parent() {
        if parent.as_os_str().is_empty() {
            (Path::new("."), path.file_name().and_then(|n| n.to_str()).unwrap_or(pattern))
        } else {
            (parent, path.file_name().and_then(|n| n.to_str()).unwrap_or(pattern))
        }
    } else {
        (Path::new("."), pattern)
    };

    let glob = Glob::new(glob_str)
        .map_err(|e| FileIoError::InvalidPath(format!("Invalid glob pattern {}: {}", pattern, e)))?;
    let matcher: GlobMatcher = glob.compile_matcher();

    let mut matches = Vec::new();
    let entries = fs::read_dir(base_dir).map_err(|e| {
        FileIoError::ReadError(format!("Failed to read directory {}: {}", base_dir.display(), e))
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            FileIoError::ReadError(format!("Failed to read directory entry: {}", e))
        })?;
        let entry_path = entry.path();
        if let Some(file_name) = entry_path.file_name().and_then(|n| n.to_str()) {
            if matcher.is_match(file_name) {
                matches.push(entry_path);
            }
        }
    }

    Ok(matches)
}

/// Remove a file or directory (supports glob patterns)
pub fn rm(path: &str, recursive: bool, force: bool) -> Result<()> {
    // Check if path contains glob patterns
    if is_glob_pattern(path) {
        // Expand glob and remove each match
        let matches = expand_glob(path)?;
        
        if matches.is_empty() {
            if force {
                return Ok(());
            }
            return Err(FileIoError::NotFound(format!("No files match pattern: {}", path)).into());
        }

        let mut errors = Vec::new();
        for match_path in matches {
            if let Err(e) = rm_single(match_path.to_str().unwrap(), recursive, force) {
                errors.push(format!("{}: {}", match_path.display(), e));
            }
        }

        if !errors.is_empty() {
            return Err(FileIoError::WriteError(format!(
                "Some removals failed: {}",
                errors.join("; ")
            ))
            .into());
        }

        Ok(())
    } else {
        // Single path
        rm_single(path, recursive, force)
    }
}

/// Remove a single file or directory
fn rm_single(path: &str, recursive: bool, force: bool) -> Result<()> {
    let path_obj = Path::new(path);

    if !path_obj.exists() {
        if force {
            return Ok(());
        }
        return Err(FileIoError::NotFound(path.to_string()).into());
    }

    if path_obj.is_file() || path_obj.is_symlink() {
        fs::remove_file(path).map_err(|e| {
            FileIoError::WriteError(format!("Failed to remove file {}: {}", path, e))
        })?;
    } else if path_obj.is_dir() {
        if recursive {
            fs::remove_dir_all(path).map_err(|e| {
                FileIoError::WriteError(format!("Failed to remove directory {}: {}", path, e))
            })?;
        } else {
            fs::remove_dir(path).map_err(|e| {
                FileIoError::WriteError(format!("Failed to remove directory {}: {}", path, e))
            })?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_rm_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("file.txt");
        fs::write(&file, "content").unwrap();

        rm(file.to_str().unwrap(), false, false).unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn test_rm_dir_recursive() {
        let dir = TempDir::new().unwrap();
        let subdir = dir.path().join("subdir");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("file.txt"), "content").unwrap();

        rm(subdir.to_str().unwrap(), true, false).unwrap();
        assert!(!subdir.exists());
    }

    #[test]
    fn test_rm_glob() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::write(base.join("file1.txt"), "content1").unwrap();
        fs::write(base.join("file2.txt"), "content2").unwrap();
        fs::write(base.join("other.log"), "content3").unwrap();

        let pattern = base.join("*.txt").to_str().unwrap().to_string();
        rm(&pattern, false, false).unwrap();

        assert!(!base.join("file1.txt").exists());
        assert!(!base.join("file2.txt").exists());
        assert!(base.join("other.log").exists());
    }
}
