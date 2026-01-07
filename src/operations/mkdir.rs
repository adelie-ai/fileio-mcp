#![deny(warnings)]

// Create directories

use crate::error::{FileIoError, Result};
use std::fs;

/// Create a directory (with -p equivalent, i.e., create parent directories)
pub fn mkdir(path: &str, recursive: bool) -> Result<()> {
    if recursive {
        fs::create_dir_all(path).map_err(|e| {
            FileIoError::WriteError(format!("Failed to create directory {}: {}", path, e))
        })?;
    } else {
        fs::create_dir(path).map_err(|e| {
            FileIoError::WriteError(format!("Failed to create directory {}: {}", path, e))
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_mkdir() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("newdir").to_str().unwrap().to_string();

        mkdir(&path, false).unwrap();
        assert!(std::path::Path::new(&path).exists());
    }

    #[test]
    fn test_mkdir_recursive() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a").join("b").join("c");
        let path_str = path.to_str().unwrap().to_string();

        mkdir(&path_str, true).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_mkdir_already_exists() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("existing").to_str().unwrap().to_string();

        mkdir(&path, true).unwrap();
        // Should succeed even if directory already exists
        mkdir(&path, true).unwrap();
        assert!(std::path::Path::new(&path).exists());
    }
}
