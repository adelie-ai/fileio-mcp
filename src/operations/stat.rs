#![deny(warnings)]

// Get file or directory statistics

use crate::error::{FileIoError, Result};
use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FileStat {
    pub path: String,
    pub entry_type: String,
    pub size: u64,
    pub mode: Option<String>,
    pub modified: Option<String>,
    pub accessed: Option<String>,
    pub created: Option<String>,
    pub is_file: bool,
    pub is_dir: bool,
    pub is_symlink: bool,
}

/// Get file or directory statistics
pub fn stat(path: &str) -> Result<FileStat> {
    let expanded_path = shellexpand::full(path)
        .map_err(|e| crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!("Failed to expand path \'{}\': {}", path, e))))
        .map(|expanded| expanded.into_owned())?;
    let path_obj = Path::new(&expanded_path);

    if !path_obj.exists() {
        return Err(FileIoError::NotFound(expanded_path.to_string()).into());
    }

    let metadata = fs::metadata(&expanded_path).map_err(|e| {
        crate::error::FileIoMcpError::from(FileIoError::from_io_error("read metadata", &expanded_path, e))
    })?;

    let entry_type = if path_obj.is_dir() {
        "directory"
    } else if path_obj.is_file() {
        "file"
    } else if path_obj.is_symlink() {
        "symlink"
    } else {
        "unknown"
    }
    .to_string();

    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        let permissions = metadata.permissions();
        let mode_value = permissions.mode();
        Some(format!("{:04o}", mode_value & 0o7777))
    };

    #[cfg(not(unix))]
    let mode: Option<String> = None;

    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs().to_string())
        });

    let accessed = metadata
        .accessed()
        .ok()
        .and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs().to_string())
        });

    #[cfg(unix)]
    let created = metadata
        .created()
        .ok()
        .and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs().to_string())
        });

    #[cfg(not(unix))]
    let created = metadata
        .created()
        .ok()
        .and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs().to_string())
        });

    Ok(FileStat {
        path: expanded_path.clone(),
        entry_type,
        size: metadata.len(),
        mode,
        modified,
        accessed,
        created,
        is_file: path_obj.is_file(),
        is_dir: path_obj.is_dir(),
        is_symlink: path_obj.is_symlink(),
    })
}

impl From<FileStat> for Value {
    fn from(stat: FileStat) -> Self {
        let mut obj = serde_json::Map::new();
        obj.insert("path".to_string(), Value::String(stat.path));
        obj.insert("type".to_string(), Value::String(stat.entry_type));
        obj.insert("size".to_string(), Value::Number(stat.size.into()));
        if let Some(mode) = stat.mode {
            obj.insert("mode".to_string(), Value::String(mode));
        }
        if let Some(modified) = stat.modified {
            obj.insert("modified".to_string(), Value::String(modified));
        }
        if let Some(accessed) = stat.accessed {
            obj.insert("accessed".to_string(), Value::String(accessed));
        }
        if let Some(created) = stat.created {
            obj.insert("created".to_string(), Value::String(created));
        }
        obj.insert("is_file".to_string(), Value::Bool(stat.is_file));
        obj.insert("is_dir".to_string(), Value::Bool(stat.is_dir));
        obj.insert("is_symlink".to_string(), Value::Bool(stat.is_symlink));
        Value::Object(obj)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn test_stat_file() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();

        let stat_result = stat(path).unwrap();
        assert!(stat_result.is_file);
        assert!(!stat_result.is_dir);
        assert_eq!(stat_result.entry_type, "file");
    }

    #[test]
    fn test_stat_directory() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_str().unwrap();

        let stat_result = stat(path).unwrap();
        assert!(!stat_result.is_file);
        assert!(stat_result.is_dir);
        assert_eq!(stat_result.entry_type, "directory");
    }

    #[test]
    fn test_stat_not_found() {
        let result = stat("/nonexistent/path/that/does/not/exist");
        assert!(result.is_err());
    }
}
