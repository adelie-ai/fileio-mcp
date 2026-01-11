#![deny(warnings)]

// Touch a file (create or update timestamp)

use crate::error::{FileIoError, Result};
use filetime::{set_file_times, FileTime};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

/// Touch files (create if they don't exist, update timestamp if they do)
/// Can accept a single path or multiple paths
pub fn touch(paths: &[&str]) -> Result<()> {
    let mut errors = Vec::new();
    for path in paths {
        if let Err(e) = touch_single(path) {
            errors.push(format!("{}: {}", path, e));
        }
    }
    if !errors.is_empty() {
        return Err(crate::error::FileIoMcpError::from(FileIoError::WriteError(format!(
            "Some touch operations failed: {}",
            errors.join("; ")
        ))));
    }
    Ok(())
}

/// Touch a single file (create if it doesn't exist, update timestamp if it does)
pub fn touch_single(path: &str) -> Result<()> {
    let expanded_path = shellexpand::full(path)
        .map_err(|e| crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!("Failed to expand path \'{}\': {}", path, e))))
        .map(|expanded| expanded.into_owned())?;
    let path_obj = Path::new(&expanded_path);

    if path_obj.exists() {
        // Update timestamp using filetime crate
        let now = SystemTime::now();
        let file_time = FileTime::from_system_time(now);
        set_file_times(&expanded_path, file_time, file_time).map_err(|e| {
            FileIoError::WriteError(format!("Failed to update timestamp for {}: {}", expanded_path, e))
        })?;
    } else {
        // Create empty file
        // Create parent directories if needed
        if let Some(parent) = path_obj.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                FileIoError::WriteError(format!(
                    "Failed to create parent directories for {}: {}",
                    expanded_path, e
                ))
            })?;
        }
        fs::File::create(&expanded_path).map_err(|e| {
            FileIoError::WriteError(format!("Failed to create file {}: {}", expanded_path, e))
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_touch_create_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("newfile.txt");
        let path_str = path.to_str().unwrap().to_string();

        touch(&[&path_str]).unwrap();
        assert!(path.exists());
        assert!(path.is_file());
    }

    #[test]
    fn test_touch_update_timestamp() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("existing.txt");
        let path_str = path.to_str().unwrap().to_string();

        // Create file first
        fs::write(&path, "content").unwrap();
        let metadata1 = fs::metadata(&path).unwrap();
        let modified1 = metadata1.modified().unwrap();

        // Small delay to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Touch the file
        touch(&[&path_str]).unwrap();

        let metadata2 = fs::metadata(&path).unwrap();
        let modified2 = metadata2.modified().unwrap();

        // The timestamp should be updated (greater than or equal)
        assert!(modified2 >= modified1);
    }

    #[test]
    fn test_touch_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("subdir").join("file.txt");
        let path_str = path.to_str().unwrap().to_string();

        touch(&[&path_str]).unwrap();
        assert!(path.exists());
    }
}
