#![deny(warnings)]

// Get current working directory

use crate::error::{FileIoError, Result};
use std::env;

/// Get the current working directory (pwd equivalent)
pub fn pwd() -> Result<String> {
    env::current_dir()
        .map_err(|e| {
            FileIoError::ReadError(format!("Failed to get current working directory: {}", e))
        })?
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| {
            FileIoError::InvalidPath(
                "Current working directory path contains invalid UTF-8".to_string(),
            )
            .into()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pwd() {
        let cwd = pwd().unwrap();
        assert!(!cwd.is_empty());
        // Should be an absolute path
        assert!(cwd.starts_with('/') || cwd.contains(':'));
    }
}
