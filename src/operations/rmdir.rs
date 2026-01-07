#![deny(warnings)]

// Remove directory

use crate::error::{FileIoError, Result};
use crate::operations::rm;
use std::path::Path;

/// Remove a directory (wrapper around rm with recursive flag)
pub fn rmdir(path: &str, recursive: bool) -> Result<()> {
    let path_obj = Path::new(path);

    if !path_obj.exists() {
        return Err(FileIoError::NotFound(path.to_string()).into());
    }

    if !path_obj.is_dir() {
        return Err(FileIoError::InvalidPath(format!("{} is not a directory", path)).into());
    }

    rm::rm(path, recursive, false)
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
