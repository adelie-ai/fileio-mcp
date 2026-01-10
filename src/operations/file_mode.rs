#![deny(warnings)]

// Set file permissions

use crate::error::{FileIoError, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;

/// Set file mode (permissions)
pub fn set_file_mode(path: &str, mode: &str) -> Result<()> {
    // Parse mode string - support both octal and symbolic
    let mode_value = parse_mode(mode)?;
    let expanded_path = shellexpand::full(path)
        .map_err(|e| crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!("Failed to expand path \'{}\': {}", path, e))))
        .map(|expanded| expanded.into_owned())?;

    let metadata = fs::metadata(&expanded_path).map_err(|e| {
        FileIoError::ReadError(format!("Failed to read metadata for {}: {}", expanded_path, e))
    })?;

    let mut permissions = metadata.permissions();
    permissions.set_mode(mode_value);
    fs::set_permissions(&expanded_path, permissions).map_err(|e| {
        FileIoError::InvalidMode(format!("Failed to set permissions for {}: {}", expanded_path, e))
    })?;

    Ok(())
}

fn parse_mode(mode_str: &str) -> Result<u32> {
    // Try octal first (e.g., "755", "0644")
    if let Ok(mode) = u32::from_str_radix(mode_str.trim_start_matches('0'), 8) {
        return Ok(mode);
    }

    // Try decimal
    if let Ok(mode) = mode_str.parse::<u32>() {
        // If it looks like octal (starts with 0), parse as octal
        if mode_str.starts_with('0') {
            return Ok(u32::from_str_radix(mode_str.trim_start_matches('0'), 8)
                .map_err(|_| FileIoError::InvalidMode(format!("Invalid octal mode: {}", mode_str)))?);
        }
        return Ok(mode);
    }

    Err(FileIoError::InvalidMode(format!(
        "Invalid mode format: {} (expected octal like 755 or 0644)",
        mode_str
    ))
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_set_file_mode_octal() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();

        set_file_mode(path, "0644").unwrap();

        let metadata = fs::metadata(path).unwrap();
        let permissions = metadata.permissions();
        let mode = permissions.mode();
        assert_eq!(mode & 0o777, 0o644);
    }

    #[test]
    fn test_set_file_mode_octal_no_leading_zero() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();

        set_file_mode(path, "755").unwrap();

        let metadata = fs::metadata(path).unwrap();
        let permissions = metadata.permissions();
        let mode = permissions.mode();
        assert_eq!(mode & 0o777, 0o755);
    }
}
