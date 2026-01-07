#![deny(warnings)]

// Change file or directory ownership

use crate::error::{FileIoError, Result};
use std::path::Path;

/// Change file or directory ownership
pub fn chown(path: &str, user: Option<&str>, group: Option<&str>) -> Result<()> {
    let path_obj = Path::new(path);

    if !path_obj.exists() {
        return Err(FileIoError::NotFound(path.to_string()).into());
    }

    #[cfg(unix)]
    {
        use nix::unistd::{Gid, Uid};

        let uid = if let Some(user_str) = user {
            // Parse user ID or username
            if let Ok(uid_val) = user_str.parse::<u32>() {
                Some(Uid::from_raw(uid_val))
            } else {
                // Try to resolve username (simplified - would need nix::pwd for full implementation)
                // For now, just parse as number
                return Err(FileIoError::InvalidPath(format!(
                    "User name resolution not yet implemented. Please use numeric UID: {}",
                    user_str
                ))
                .into());
            }
        } else {
            None
        };

        let gid = if let Some(group_str) = group {
            // Parse group ID or groupname
            if let Ok(gid_val) = group_str.parse::<u32>() {
                Some(Gid::from_raw(gid_val))
            } else {
                // Try to resolve groupname (simplified)
                return Err(FileIoError::InvalidPath(format!(
                    "Group name resolution not yet implemented. Please use numeric GID: {}",
                    group_str
                ))
                .into());
            }
        } else {
            None
        };

        nix::unistd::chown(path_obj, uid, gid).map_err(|e| {
            FileIoError::WriteError(format!("Failed to change ownership of {}: {}", path, e))
        })?;
    }

    #[cfg(not(unix))]
    {
        return Err(FileIoError::InvalidPath(
            "chown is only supported on Unix-like systems".to_string(),
        )
        .into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    #[cfg(unix)]
    fn test_chown() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();

        // Get current user/group
        use nix::unistd::{getgid, getuid};
        let uid = getuid();
        let gid = getgid();

        // Change ownership to current user (should succeed)
        chown(path, Some(&uid.as_raw().to_string()), Some(&gid.as_raw().to_string())).unwrap();
    }
}
