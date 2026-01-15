#![deny(warnings)]

// Get file permissions (mode)

use crate::error::{FileIoError, Result};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Get file mode (permissions) as octal string
/// Can accept a single path or multiple paths, returns a map of path -> mode
pub fn get_file_mode(paths: &[&str]) -> Result<std::collections::HashMap<String, String>> {
    let mut results = std::collections::HashMap::new();
    let mut errors = Vec::new();
    for path in paths {
        match get_file_mode_single(path) {
            Ok(mode) => {
                results.insert(path.to_string(), mode);
            }
            Err(e) => {
                errors.push(format!("{}: {}", path, e));
            }
        }
    }
    if !errors.is_empty() {
        return Err(crate::error::FileIoMcpError::from(FileIoError::ReadError(
            format!("Some permission queries failed: {}", errors.join("; ")),
        )));
    }
    Ok(results)
}

/// Get file mode (permissions) as octal string for a single path
pub fn get_file_mode_single(path: &str) -> Result<String> {
    let expanded_path = shellexpand::full(path)
        .map_err(|e| {
            crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!(
                "Failed to expand path \'{}\': {}",
                path, e
            )))
        })
        .map(|expanded| expanded.into_owned())?;
    let metadata = fs::metadata(&expanded_path).map_err(|e| {
        crate::error::FileIoMcpError::from(FileIoError::from_io_error(
            "read metadata",
            &expanded_path,
            e,
        ))
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

        let modes = get_file_mode(&[path]).unwrap();
        let mode = modes.get(path).unwrap();
        // Should be a valid octal string
        assert!(mode.len() == 4);
        assert!(mode.chars().all(|c| c.is_ascii_digit()));
    }
}
