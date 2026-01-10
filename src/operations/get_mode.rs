#![deny(warnings)]

// Get file permissions (mode)

use crate::error::{FileIoError, Result};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Get file mode (permissions) as octal string
pub fn get_file_mode(path: &str) -> Result<String> {
    let expanded_path = shellexpand::full(path)
        .map_err(|e| crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!("Failed to expand path \'{}\': {}", path, e))))
        .map(|expanded| expanded.into_owned())?;
    let metadata = fs::metadata(&expanded_path).map_err(|e| {
        FileIoError::ReadError(format!("Failed to read metadata for {}: {}", expanded_path, e))
    })?;

    #[cfg(unix)]
    {
        let permissions = metadata.permissions();
        let mode = permissions.mode();
        // Return as octal string with leading zero
        Ok(format!("{:04o}", mode & 0o7777))
    }

    #[cfg(not(unix))]
    {
        // On non-Unix systems, return a placeholder
        // In practice, this would need platform-specific handling
        Ok("0000".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_get_file_mode() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();

        let mode = get_file_mode(path).unwrap();
        // Should be a valid octal string
        assert!(mode.len() == 4);
        assert!(mode.chars().all(|c| c.is_ascii_digit()));
    }
}
