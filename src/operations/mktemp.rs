#![deny(warnings)]

// Create temporary files or directories

use crate::error::{FileIoError, Result};
use std::path::Path;
use tempfile::{NamedTempFile, TempDir};

/// Create a temporary file
pub fn mktemp_file(template: Option<&str>) -> Result<String> {
    if let Some(tmpl) = template {
        // If template provided, create in specified directory
        let path = Path::new(tmpl);
        let parent = path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent).map_err(|e| {
            FileIoError::WriteError(format!(
                "Failed to create parent directory for template {}: {}",
                tmpl, e
            ))
        })?;
        let file = NamedTempFile::new_in(parent).map_err(|e| {
            FileIoError::WriteError(format!("Failed to create temporary file: {}", e))
        })?;
        let path_str = file.path().to_string_lossy().to_string();
        file.keep().map_err(|e| {
            FileIoError::WriteError(format!("Failed to persist temporary file: {}", e))
        })?;
        Ok(path_str)
    } else {
        let file = NamedTempFile::new().map_err(|e| {
            FileIoError::WriteError(format!("Failed to create temporary file: {}", e))
        })?;
        let path_str = file.path().to_string_lossy().to_string();
        file.keep().map_err(|e| {
            FileIoError::WriteError(format!("Failed to persist temporary file: {}", e))
        })?;
        Ok(path_str)
    }
}

/// Create a temporary directory
pub fn mktemp_dir(template: Option<&str>) -> Result<String> {
    if let Some(tmpl) = template {
        // If template provided, create in specified directory
        let path = Path::new(tmpl);
        let parent = path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent).map_err(|e| {
            FileIoError::WriteError(format!(
                "Failed to create parent directory for template {}: {}",
                tmpl, e
            ))
        })?;
        let dir = TempDir::new_in(parent).map_err(|e| {
            FileIoError::WriteError(format!("Failed to create temporary directory: {}", e))
        })?;
        let path_str = dir.path().to_string_lossy().to_string();
        let _ = dir.keep(); // Keep the directory
        Ok(path_str)
    } else {
        let dir = TempDir::new().map_err(|e| {
            FileIoError::WriteError(format!("Failed to create temporary directory: {}", e))
        })?;
        let path_str = dir.path().to_string_lossy().to_string();
        let _ = dir.keep(); // Keep the directory
        Ok(path_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mktemp_file() {
        let path = mktemp_file(None).unwrap();
        assert!(Path::new(&path).exists());
        assert!(Path::new(&path).is_file());
    }

    #[test]
    fn test_mktemp_dir() {
        let path = mktemp_dir(None).unwrap();
        assert!(Path::new(&path).exists());
        assert!(Path::new(&path).is_dir());
    }
}
