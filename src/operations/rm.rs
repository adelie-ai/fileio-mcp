#![deny(warnings)]

// Remove files or directories

use crate::error::{FileIoError, Result};
use crate::operations::path_utils::{expand_glob, expand_path, is_glob_pattern};
use std::fs;
use std::path::Path;

/// Remove files or directories (supports glob patterns and arrays of paths)
#[tracing::instrument(skip_all)]
pub fn rm(paths: &[&str], recursive: bool, force: bool) -> Result<Vec<super::mv::OpResult>> {
    let mut all_paths = Vec::new();

    for path in paths {
        // Check if path contains glob patterns
        if is_glob_pattern(path) {
            // Expand glob and add matches
            let matches = expand_glob(path)?;

            if matches.is_empty() {
                if !force {
                    return Err(
                        FileIoError::NotFound(format!("No files match pattern: {}", path)).into(),
                    );
                }
            } else {
                for match_path in matches {
                    all_paths.push(match_path.to_str().unwrap().to_string());
                }
            }
        } else {
            // Expand the same way `expand_glob` expands a pattern, so the
            // path acted on is the path the guard approved.
            all_paths.push(expand_path(path)?);
        }
    }

    // Remove all collected paths and return per-path results
    let mut results = Vec::new();
    for path in &all_paths {
        match rm_single(path, recursive, force) {
            Ok(()) => results.push(super::mv::OpResult {
                path: path.clone(),
                status: "ok".to_string(),
                exists: true,
            }),
            Err(e) => {
                let is_not_found = matches!(
                    e,
                    crate::error::FileIoMcpError::FileIo(crate::error::FileIoError::NotFound(_))
                );
                results.push(super::mv::OpResult {
                    path: path.clone(),
                    status: format!("error: {}", e),
                    exists: !is_not_found,
                });
            }
        }
    }

    Ok(results)
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
            crate::error::FileIoMcpError::from(FileIoError::from_io_error("remove file", path, e))
        })?;
    } else if path_obj.is_dir() {
        if recursive {
            fs::remove_dir_all(path).map_err(|e| {
                use std::io::ErrorKind;
                match e.kind() {
                    ErrorKind::PermissionDenied => FileIoError::PermissionDenied(format!(
                        "Permission denied when removing directory: {}",
                        path
                    ))
                    .into(),
                    _ => crate::error::FileIoMcpError::from(FileIoError::from_io_error(
                        "remove directory",
                        path,
                        e,
                    )),
                }
            })?;
        } else {
            fs::remove_dir(path).map_err(|e| {
                use std::io::ErrorKind;
                match e.kind() {
                    ErrorKind::PermissionDenied => {
                        FileIoError::PermissionDenied(format!(
                            "Permission denied when removing directory: {}",
                            path
                        )).into()
                    }
                    ErrorKind::InvalidInput => {
                        FileIoError::WriteError(format!(
                            "Directory is not empty: {}. Use recursive=true to remove non-empty directories",
                            path
                        )).into()
                    }
                    _ => crate::error::FileIoMcpError::from(FileIoError::from_io_error("remove directory", path, e))
                }
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

        let results = rm(&[file.to_str().unwrap()], false, false).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, "ok");
        assert!(!file.exists());
    }

    #[test]
    fn test_rm_dir_recursive() {
        let dir = TempDir::new().unwrap();
        let subdir = dir.path().join("subdir");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("file.txt"), "content").unwrap();

        let results = rm(&[subdir.to_str().unwrap()], true, false).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, "ok");
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
        let results = rm(&[&pattern], false, false).unwrap();
        assert!(results.iter().all(|r| r.status == "ok"));

        assert!(!base.join("file1.txt").exists());
        assert!(!base.join("file2.txt").exists());
        assert!(base.join("other.log").exists());
    }
}
