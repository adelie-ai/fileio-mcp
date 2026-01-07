#![deny(warnings)]

// Create hard or symbolic links

use crate::error::{FileIoError, Result};
use std::fs;
use std::path::Path;

/// Create a hard link
pub fn hard_link(target: &str, link_path: &str) -> Result<()> {
    let target_path = Path::new(target);

    if !target_path.exists() {
        return Err(FileIoError::NotFound(target.to_string()).into());
    }

    // Create parent directories if needed
    let link_path_obj = Path::new(link_path);
    if let Some(parent) = link_path_obj.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            FileIoError::WriteError(format!(
                "Failed to create parent directories for {}: {}",
                link_path, e
            ))
        })?;
    }

    fs::hard_link(target, link_path).map_err(|e| {
        FileIoError::WriteError(format!("Failed to create hard link {} to {}: {}", link_path, target, e))
    })?;

    Ok(())
}

/// Create a symbolic link
pub fn symlink(target: &str, link_path: &str) -> Result<()> {

    // Create parent directories if needed
    let link_path_obj = Path::new(link_path);
    if let Some(parent) = link_path_obj.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            FileIoError::WriteError(format!(
                "Failed to create parent directories for {}: {}",
                link_path, e
            ))
        })?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(target, link_path).map_err(|e| {
            FileIoError::WriteError(format!(
                "Failed to create symbolic link {} to {}: {}",
                link_path, target, e
            ))
        })?;
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::symlink_file;
        let target_path = Path::new(target);
        if target_path.is_dir() {
            use std::os::windows::fs::symlink_dir;
            symlink_dir(target, link_path).map_err(|e| {
                FileIoError::WriteError(format!(
                    "Failed to create symbolic link {} to {}: {}",
                    link_path, target, e
                ))
            })?;
        } else {
            symlink_file(target, link_path).map_err(|e| {
                FileIoError::WriteError(format!(
                    "Failed to create symbolic link {} to {}: {}",
                    link_path, target, e
                ))
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
    fn test_hard_link() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("target.txt");
        let link = dir.path().join("link.txt");

        fs::write(&target, "content").unwrap();
        hard_link(target.to_str().unwrap(), link.to_str().unwrap()).unwrap();

        assert!(link.exists());
        assert_eq!(fs::read_to_string(&link).unwrap(), "content");
    }

    #[test]
    fn test_symlink() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("target.txt");
        let link = dir.path().join("link.txt");

        fs::write(&target, "content").unwrap();
        symlink(target.to_str().unwrap(), link.to_str().unwrap()).unwrap();

        assert!(link.is_symlink());
    }
}
