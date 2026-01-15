#![deny(warnings)]

// Create hard or symbolic links

use crate::error::{FileIoError, Result};
use std::fs;
use std::path::Path;

/// Create a hard link
pub fn hard_link(target: &str, link_path: &str) -> Result<()> {
    let expanded_target = shellexpand::full(target)
        .map_err(|e| {
            crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!(
                "Failed to expand path \'{}\': {}",
                target, e
            )))
        })
        .map(|expanded| expanded.into_owned())?;
    let expanded_link = shellexpand::full(link_path)
        .map_err(|e| {
            crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!(
                "Failed to expand path \'{}\': {}",
                link_path, e
            )))
        })
        .map(|expanded| expanded.into_owned())?;
    let target_path = Path::new(&expanded_target);

    if !target_path.exists() {
        return Err(FileIoError::NotFound(expanded_target.to_string()).into());
    }

    // Create parent directories if needed
    let link_path_obj = Path::new(&expanded_link);
    if let Some(parent) = link_path_obj.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            FileIoError::WriteError(format!(
                "Failed to create parent directories for {}: {}",
                expanded_link, e
            ))
        })?;
    }

    fs::hard_link(&expanded_target, &expanded_link).map_err(|e| {
        use std::io::ErrorKind;
        match e.kind() {
            ErrorKind::PermissionDenied => {
                crate::error::FileIoMcpError::from(FileIoError::PermissionDenied(format!(
                    "Permission denied when creating hard link {} to {}: {}",
                    expanded_link, expanded_target, e
                )))
            }
            ErrorKind::NotFound => {
                crate::error::FileIoMcpError::from(FileIoError::NotFound(format!(
                    "Target not found when creating hard link: {}",
                    expanded_target
                )))
            }
            ErrorKind::AlreadyExists => {
                crate::error::FileIoMcpError::from(FileIoError::WriteError(format!(
                    "Hard link already exists: {}. Cannot create duplicate link to {}",
                    expanded_link, expanded_target
                )))
            }
            _ => crate::error::FileIoMcpError::from(FileIoError::from_io_error(
                "create hard link",
                &format!("{} to {}", expanded_link, expanded_target),
                e,
            )),
        }
    })?;

    Ok(())
}

/// Create a symbolic link
pub fn symlink(target: &str, link_path: &str) -> Result<()> {
    let expanded_link = shellexpand::full(link_path)
        .map_err(|e| {
            crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!(
                "Failed to expand path \'{}\': {}",
                link_path, e
            )))
        })
        .map(|expanded| expanded.into_owned())?;

    // Create parent directories if needed
    let link_path_obj = Path::new(&expanded_link);
    if let Some(parent) = link_path_obj.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            FileIoError::WriteError(format!(
                "Failed to create parent directories for {}: {}",
                expanded_link, e
            ))
        })?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(target, &expanded_link).map_err(|e| {
            use std::io::ErrorKind;
            match e.kind() {
                ErrorKind::PermissionDenied => {
                    crate::error::FileIoMcpError::from(FileIoError::PermissionDenied(format!(
                        "Permission denied when creating symbolic link {} to {}: {}",
                        expanded_link, target, e
                    )))
                }
                ErrorKind::AlreadyExists => {
                    crate::error::FileIoMcpError::from(FileIoError::WriteError(format!(
                        "Symbolic link already exists: {}. Cannot create duplicate link to {}",
                        expanded_link, target
                    )))
                }
                _ => crate::error::FileIoMcpError::from(FileIoError::from_io_error(
                    "create symbolic link",
                    &format!("{} to {}", expanded_link, target),
                    e,
                )),
            }
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
