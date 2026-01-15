#![deny(warnings)]

// Path utility functions (basename, dirname, realpath, readlink)

use crate::error::{FileIoError, Result};
use std::fs;
use std::path::Path;

/// Get the basename (filename) from a path
pub fn basename(path: &str) -> Result<String> {
    let expanded_path = shellexpand::full(path)
        .map_err(|e| {
            crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!(
                "Failed to expand path '{}'': {}",
                path, e
            )))
        })
        .map(|expanded| expanded.into_owned())?;
    let path_obj = Path::new(&expanded_path);

    path_obj
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            FileIoError::InvalidPath(format!(
                "Cannot extract basename from path: {}",
                expanded_path
            ))
            .into()
        })
}

/// Get the dirname (directory path) from a path
pub fn dirname(path: &str) -> Result<String> {
    let expanded_path = shellexpand::full(path)
        .map_err(|e| {
            crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!(
                "Failed to expand path '{}'': {}",
                path, e
            )))
        })
        .map(|expanded| expanded.into_owned())?;
    let path_obj = Path::new(&expanded_path);
    path_obj
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| {
            FileIoError::InvalidPath(format!(
                "Cannot extract dirname from path: {}",
                expanded_path
            ))
            .into()
        })
}

/// Get the real (canonical) path, resolving all symlinks
pub fn realpath(path: &str) -> Result<String> {
    let expanded_path = shellexpand::full(path)
        .map_err(|e| {
            crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!(
                "Failed to expand path '{}'': {}",
                path, e
            )))
        })
        .map(|expanded| expanded.into_owned())?;
    let path_obj = Path::new(&expanded_path);

    if !path_obj.exists() {
        return Err(FileIoError::NotFound(expanded_path.to_string()).into());
    }

    let canonical = fs::canonicalize(&expanded_path).map_err(|e| {
        crate::error::FileIoMcpError::from(FileIoError::from_io_error(
            "canonicalize path",
            &expanded_path,
            e,
        ))
    })?;

    canonical.to_str().map(|s| s.to_string()).ok_or_else(|| {
        FileIoError::InvalidPath(format!(
            "Path contains invalid UTF-8: {}",
            canonical.display()
        ))
        .into()
    })
}

/// Read the target of a symbolic link
pub fn readlink(path: &str) -> Result<String> {
    let expanded_path = shellexpand::full(path)
        .map_err(|e| {
            crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!(
                "Failed to expand path '{}'': {}",
                path, e
            )))
        })
        .map(|expanded| expanded.into_owned())?;
    // Use symlink_metadata so we don't follow the symlink. This lets us
    // observe and read broken symlinks (they may point at non-existent targets).
    let metadata = fs::symlink_metadata(&expanded_path).map_err(|e| {
        crate::error::FileIoMcpError::from(FileIoError::from_io_error(
            "lstat path",
            &expanded_path,
            e,
        ))
    })?;

    if !metadata.file_type().is_symlink() {
        return Err(
            FileIoError::InvalidPath(format!("{} is not a symbolic link", expanded_path)).into(),
        );
    }

    let target = fs::read_link(&expanded_path).map_err(|e| {
        crate::error::FileIoMcpError::from(FileIoError::from_io_error(
            "read symbolic link",
            &expanded_path,
            e,
        ))
    })?;
    target.to_str().map(|s: &str| s.to_string()).ok_or_else(|| {
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

    // New test: reading a broken symlink should return the stored target path
    #[test]
    #[cfg(unix)]
    fn test_readlink_broken_symlink() {
        use std::os::unix::fs::symlink;
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("no_such_file.txt");
        let link = dir.path().join("broken_link");

        // Create a symlink pointing to a non-existent target
        symlink(&target, &link).unwrap();

        // Expectation: readlink should return the stored target path even if it doesn't exist
        let read = readlink(link.to_str().unwrap()).unwrap();
        assert_eq!(read, target.to_str().unwrap());
    }

    #[test]
    fn test_readlink_non_symlink_errors() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("file.txt");
        fs::write(&file, "content").unwrap();

        let res = readlink(file.to_str().unwrap());
        assert!(res.is_err());
    }
}
