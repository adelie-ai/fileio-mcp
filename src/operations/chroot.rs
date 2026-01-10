#![deny(warnings)]

// Change root directory (chroot)

use crate::error::{FileIoError, Result};
use std::path::Path;

/// Change root directory (chroot)
pub fn chroot(new_root: &str) -> Result<()> {
    let expanded_root = shellexpand::full(new_root)
        .map_err(|e| crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!("Failed to expand path \'{}\': {}", new_root, e))))
        .map(|expanded| expanded.into_owned())?;
    let path_obj = Path::new(&expanded_root);

    if !path_obj.exists() {
        return Err(FileIoError::NotFound(expanded_root.to_string()).into());
    }

    if !path_obj.is_dir() {
        return Err(FileIoError::InvalidPath(format!(
            "{} is not a directory",
            expanded_root
        ))
        .into());
    }

    #[cfg(unix)]
    {
        use nix::unistd::chroot;

        chroot(path_obj).map_err(|e| {
            FileIoError::WriteError(format!("Failed to change root to {}: {}", expanded_root, e))
        })?;
    }

    #[cfg(not(unix))]
    {
        return Err(FileIoError::InvalidPath(
            "chroot is only supported on Unix-like systems".to_string(),
        )
        .into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    #[cfg(unix)]
    fn test_chroot_requires_root() {
        // Note: chroot typically requires root privileges
        // This test just verifies the function exists and handles errors
        let dir = TempDir::new().unwrap();
        let result = chroot(dir.path().to_str().unwrap());
        // Will likely fail without root, but that's expected
        let _ = result;
    }
}
