#![deny(warnings)]

// Write content to a file

use crate::error::{FileIoError, Result};
use std::fs;
use std::path::Path;

/// Write content to a file
pub fn write_file(path: &str, content: &str, append: bool) -> Result<()> {
    let expanded_path = shellexpand::full(path)
        .map_err(|e| crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!("Failed to expand path \'{}\': {}", path, e))))
        .map(|expanded| expanded.into_owned())?;
    let path_obj = Path::new(&expanded_path);

    // Create parent directories if they don't exist
    if let Some(parent) = path_obj.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            FileIoError::WriteError(format!(
                "Failed to create parent directories for {}: {}",
                expanded_path, e
            ))
        })?;
    }

    if append {
        use std::fs::OpenOptions;
        use std::io::Write;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&expanded_path)
            .map_err(|e| {
                crate::error::FileIoMcpError::from(FileIoError::from_io_error("open file for appending", &expanded_path, e))
            })?;

        file.write_all(content.as_bytes())
            .map_err(|e| crate::error::FileIoMcpError::from(FileIoError::from_io_error("write to file", &expanded_path, e)))?;
    } else {
        // Atomic write: write to temp file, then rename
        let temp_path = format!("{}.tmp", expanded_path);
        fs::write(&temp_path, content).map_err(|e| {
            crate::error::FileIoMcpError::from(FileIoError::from_io_error("write to temp file", &temp_path, e))
        })?;
        fs::rename(&temp_path, &expanded_path).map_err(|e| {
            use std::io::ErrorKind;
            match e.kind() {
                ErrorKind::PermissionDenied => {
                    crate::error::FileIoMcpError::from(FileIoError::PermissionDenied(format!(
                        "Permission denied when writing file: {}",
                        expanded_path
                    )))
                }
                _ => crate::error::FileIoMcpError::from(FileIoError::from_io_error("rename temp file", &format!("{} to {}", temp_path, expanded_path), e))
            }
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
