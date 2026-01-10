#![deny(warnings)]

// Remove directory

use crate::error::{FileIoError, Result};
use crate::operations::rm;
use std::path::Path;

/// Remove a directory (wrapper around rm with recursive flag)
pub fn rmdir(path: &str, recursive: bool) -> Result<()> {
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

    rm::rm(&expanded_path, recursive, false)
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

        rmdir(subdir.to_str().unwrap(), false).unwrap();
        assert!(!subdir.exists());
    }

    #[test]
    fn test_rmdir_recursive() {
        let dir = TempDir::new().unwrap();
        let subdir = dir.path().join("subdir");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("file.txt"), "content").unwrap();

        rmdir(subdir.to_str().unwrap(), true).unwrap();
        assert!(!subdir.exists());
    }
}
