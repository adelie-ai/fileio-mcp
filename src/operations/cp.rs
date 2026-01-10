#![deny(warnings)]

// Copy files or directories

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
    let expanded_pattern = shellexpand::full(pattern)
        .map_err(|e| crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!("Failed to expand path \'{}\': {}", pattern, e))))
        .map(|expanded| expanded.into_owned())?;
    let path = Path::new(&expanded_pattern);
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

/// Copy a file or directory (supports glob patterns)
pub fn cp(source: &str, destination: &str, recursive: bool) -> Result<()> {
    let expanded_dest = shellexpand::full(destination)
        .map_err(|e| crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!("Failed to expand path \'{}\': {}", destination, e))))
        .map(|expanded| expanded.into_owned())?;
    let dest_path = Path::new(&expanded_dest);
    let dest_is_dir = dest_path.exists() && dest_path.is_dir();

    // Check if source contains glob patterns
    if is_glob_pattern(source) {
        // Expand glob and copy each match
        let matches = expand_glob(source)?;
        
        if matches.is_empty() {
            return Err(FileIoError::NotFound(format!("No files match pattern: {}", source)).into());
        }

        if !dest_is_dir && matches.len() > 1 {
            return Err(FileIoError::InvalidPath(
                format!("Multiple files match pattern '{}' but destination '{}' is not a directory", source, destination)
            ).into());
        }

        for match_path in matches {
            let dest = if dest_is_dir {
                dest_path.join(match_path.file_name().unwrap())
            } else {
                dest_path.to_path_buf()
            };

            cp_single(match_path.to_str().unwrap(), dest.to_str().unwrap(), recursive)?;
        }

        Ok(())
    } else {
        // Single path
        cp_single(source, destination, recursive)
    }
}

/// Copy a single file or directory
fn cp_single(source: &str, destination: &str, recursive: bool) -> Result<()> {
    let source_path = Path::new(source);
    let dest_path = Path::new(destination);

    if !source_path.exists() {
        return Err(FileIoError::NotFound(source.to_string()).into());
    }

    if source_path.is_file() {
        // Copy file
        fs::copy(source, destination).map_err(|e| {
            FileIoError::WriteError(format!("Failed to copy file {} to {}: {}", source, destination, e))
        })?;
    } else if source_path.is_dir() {
        if !recursive {
            return Err(FileIoError::InvalidPath(
                "Cannot copy directory without recursive flag".to_string(),
            )
            .into());
        }
        copy_dir_all(source_path, dest_path)?;
    } else {
        return Err(FileIoError::InvalidPath(format!(
            "Source {} is not a file or directory",
            source
        ))
        .into());
    }

    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).map_err(|e| {
        FileIoError::WriteError(format!("Failed to create destination directory {}: {}", dst.display(), e))
    })?;

    let entries = fs::read_dir(src).map_err(|e| {
        FileIoError::ReadError(format!("Failed to read source directory {}: {}", src.display(), e))
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            FileIoError::ReadError(format!("Failed to read directory entry: {}", e))
        })?;
        let path = entry.path();
        let file_name = entry.file_name();

        let dest_path = dst.join(&file_name);

        if path.is_dir() {
            copy_dir_all(&path, &dest_path)?;
        } else {
            fs::copy(&path, &dest_path).map_err(|e| {
                FileIoError::WriteError(format!(
                    "Failed to copy file {} to {}: {}",
                    path.display(),
                    dest_path.display(),
                    e
                ))
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
    fn test_cp_file() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("source.txt");
        let dst = dir.path().join("dest.txt");

        fs::write(&src, "content").unwrap();
        cp(src.to_str().unwrap(), dst.to_str().unwrap(), false).unwrap();

        assert!(dst.exists());
        assert_eq!(fs::read_to_string(&dst).unwrap(), "content");
    }

    #[test]
    fn test_cp_dir_recursive() {
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        let dst_dir = dir.path().join("dst");

        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("file.txt"), "content").unwrap();

        cp(src_dir.to_str().unwrap(), dst_dir.to_str().unwrap(), true).unwrap();

        assert!(dst_dir.exists());
        assert!(dst_dir.join("file.txt").exists());
    }

    #[test]
    fn test_cp_glob() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::write(base.join("file1.txt"), "content1").unwrap();
        fs::write(base.join("file2.txt"), "content2").unwrap();
        fs::write(base.join("other.log"), "content3").unwrap();

        let dst_dir = base.join("dest");
        fs::create_dir_all(&dst_dir).unwrap();

        let pattern = base.join("*.txt").to_str().unwrap().to_string();
        cp(&pattern, dst_dir.to_str().unwrap(), false).unwrap();

        assert!(dst_dir.join("file1.txt").exists());
        assert!(dst_dir.join("file2.txt").exists());
        assert!(!dst_dir.join("other.log").exists());
    }
}
