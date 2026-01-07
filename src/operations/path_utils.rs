#![deny(warnings)]

// Path utility functions (basename, dirname, realpath, readlink)

use crate::error::{FileIoError, Result};
use std::fs;
use std::path::Path;

/// Get the basename (filename) from a path
pub fn basename(path: &str) -> Result<String> {
    let path_obj = Path::new(path);
    path_obj
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            FileIoError::InvalidPath(format!("Cannot extract basename from path: {}", path)).into()
        })
}

/// Get the dirname (directory path) from a path
pub fn dirname(path: &str) -> Result<String> {
    let path_obj = Path::new(path);
    path_obj
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| {
            FileIoError::InvalidPath(format!("Cannot extract dirname from path: {}", path)).into()
        })
}

/// Get the real (canonical) path, resolving all symlinks
pub fn realpath(path: &str) -> Result<String> {
    let path_obj = Path::new(path);

    if !path_obj.exists() {
        return Err(FileIoError::NotFound(path.to_string()).into());
    }

    let canonical = fs::canonicalize(path).map_err(|e| {
        FileIoError::ReadError(format!("Failed to canonicalize path {}: {}", path, e))
    })?;

    canonical
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| {
            FileIoError::InvalidPath(format!("Path contains invalid UTF-8: {}", canonical.display()))
                .into()
        })
}

/// Read the target of a symbolic link
pub fn readlink(path: &str) -> Result<String> {
    let path_obj = Path::new(path);

    if !path_obj.exists() {
        return Err(FileIoError::NotFound(path.to_string()).into());
    }

    if !path_obj.is_symlink() {
        return Err(FileIoError::InvalidPath(format!("{} is not a symbolic link", path)).into());
    }

    let target = fs::read_link(path).map_err(|e| {
        FileIoError::ReadError(format!("Failed to read symbolic link {}: {}", path, e))
    })?;
    target
        .to_str()
        .map(|s: &str| s.to_string())
        .ok_or_else(|| {
            FileIoError::InvalidPath(format!(
                "Symlink target contains invalid UTF-8: {}",
                target.display()
            ))
            .into()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_basename() {
        assert_eq!(basename("/path/to/file.txt").unwrap(), "file.txt");
        assert_eq!(basename("file.txt").unwrap(), "file.txt");
    }

    #[test]
    fn test_dirname() {
        assert_eq!(dirname("/path/to/file.txt").unwrap(), "/path/to");
        assert_eq!(dirname("file.txt").unwrap(), "");
    }

    #[test]
    fn test_realpath() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("file.txt");
        fs::write(&file, "content").unwrap();

        let real = realpath(file.to_str().unwrap()).unwrap();
        assert!(real.contains("file.txt"));
    }
}
