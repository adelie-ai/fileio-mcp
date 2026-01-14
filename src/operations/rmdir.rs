#![deny(warnings)]

// Remove directory

use crate::error::{FileIoError, Result};
use crate::operations::rm;
use std::path::Path;

/// Remove directories (wrapper around rm with recursive flag)
/// Can accept a single path or multiple paths
pub fn rmdir(paths: &[&str], recursive: bool) -> Result<Vec<super::mv::OpResult>> {
    let mut results = Vec::new();
    for path in paths {
        match rmdir_single(path, recursive) {
            Ok(()) => results.push(super::mv::OpResult { path: path.to_string(), status: "ok".to_string(), exists: true }),
            Err(e) => {
                let is_not_found = matches!(e, crate::error::FileIoMcpError::FileIo(crate::error::FileIoError::NotFound(_)));
                results.push(super::mv::OpResult { path: path.to_string(), status: format!("error: {}", e), exists: !is_not_found });
            }
        }
    }
    Ok(results)
}

/// Remove a single directory (wrapper around rm with recursive flag)
pub fn rmdir_single(path: &str, recursive: bool) -> Result<()> {
    let expanded_path = shellexpand::full(path)
        .map_err(|e| crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!("Failed to expand path \'{}\': {}", path, e))))
        .map(|expanded| expanded.into_owned())?;
    let path_obj = Path::new(&expanded_path);

    if !path_obj.exists() {
        return Err(FileIoError::NotFound(expanded_path.to_string()).into());
    }

    if !path_obj.is_dir() {
        return Err(FileIoError::InvalidPath(format!("Path is not a directory: {}", expanded_path)).into());
    }

    // Check if directory is empty when recursive=false
    if !recursive {
        let mut entries = std::fs::read_dir(&expanded_path).map_err(|e| {
            crate::error::FileIoMcpError::from(FileIoError::ReadError(format!("Failed to read directory {}: {}", expanded_path, e)))
        })?;
        if entries.next().is_some() {
            return Err(FileIoError::WriteError(format!(
                "Directory is not empty: {}. Use recursive=true to remove non-empty directories",
                expanded_path
            )).into());
        }
    }

    // Use rm::rm which now returns per-path results; translate single-entry result to Result<()> for callers
    let results = rm::rm(&[&expanded_path], recursive, false)?;
    if let Some(r) = results.get(0) {
        if r.status == "ok" {
            Ok(())
        } else {
            Err(crate::error::FileIoMcpError::from(FileIoError::WriteError(format!("Removal failed: {}: {}", r.path, r.status))))
        }
    } else {
        Err(crate::error::FileIoMcpError::from(FileIoError::WriteError("No result from rm".to_string())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_rmdir() {
        let dir = TempDir::new().unwrap();
        let subdir = dir.path().join("subdir");
        fs::create_dir_all(&subdir).unwrap();

        let results = rmdir(&[subdir.to_str().unwrap()], false).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, "ok");
        assert!(!subdir.exists());
    }

    #[test]
    fn test_rmdir_recursive() {
        let dir = TempDir::new().unwrap();
        let subdir = dir.path().join("subdir");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("file.txt"), "content").unwrap();

        let results = rmdir(&[subdir.to_str().unwrap()], true).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, "ok");
        assert!(!subdir.exists());
    }
}
