#![deny(warnings)]

// Write content to a file

use crate::error::{FileIoError, Result};
use std::fs;
use std::path::Path;

/// Write content to a file
pub fn write_file(path: &str, content: &str, append: bool) -> Result<()> {
    let path_obj = Path::new(path);

    // Create parent directories if they don't exist
    if let Some(parent) = path_obj.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            FileIoError::WriteError(format!(
                "Failed to create parent directories for {}: {}",
                path, e
            ))
        })?;
    }

    if append {
        use std::fs::OpenOptions;
        use std::io::Write;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| {
                FileIoError::WriteError(format!("Failed to open file {} for appending: {}", path, e))
            })?;

        file.write_all(content.as_bytes())
            .map_err(|e| FileIoError::WriteError(format!("Failed to write to file {}: {}", path, e)))?;
    } else {
        // Atomic write: write to temp file, then rename
        let temp_path = format!("{}.tmp", path);
        fs::write(&temp_path, content).map_err(|e| {
            FileIoError::WriteError(format!("Failed to write to temp file {}: {}", temp_path, e))
        })?;
        fs::rename(&temp_path, path).map_err(|e| {
            FileIoError::WriteError(format!(
                "Failed to rename temp file {} to {}: {}",
                temp_path, path, e
            ))
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_write_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt").to_str().unwrap().to_string();

        write_file(&path, "hello world", false).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_write_file_append() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt").to_str().unwrap().to_string();

        write_file(&path, "hello", false).unwrap();
        write_file(&path, " world", true).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_write_file_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("subdir").join("test.txt");
        let path_str = path.to_str().unwrap().to_string();

        write_file(&path_str, "content", false).unwrap();

        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "content");
    }
}
