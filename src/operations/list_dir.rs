#![deny(warnings)]

// List directory contents

use crate::error::{FileIoError, Result};
use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub entry_type: String,
    pub size: Option<u64>,
    pub modified: Option<String>,
}

/// List directory contents
pub fn list_directory(path: &str, recursive: bool, include_hidden: bool) -> Result<Vec<DirEntry>> {
    let expanded_path = shellexpand::full(path)
        .map_err(|e| {
            crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!(
                "Failed to expand path \'{}\': {}",
                path, e
            )))
        })
        .map(|expanded| expanded.into_owned())?;
    let path_obj = Path::new(&expanded_path);

    // If the path doesn't exist, treat as non-fatal: return empty entries.
    if !path_obj.exists() {
        return Ok(Vec::new());
    }

    if !path_obj.is_dir() {
        return Err(
            FileIoError::InvalidPath(format!("{} is not a directory", expanded_path)).into(),
        );
    }

    let mut entries = Vec::new();

    if recursive {
        collect_entries_recursive(path_obj, path_obj, &mut entries, include_hidden)?;
    } else {
        collect_entries(path_obj, &mut entries, include_hidden)?;
    }

    Ok(entries)
}

fn collect_entries(dir: &Path, entries: &mut Vec<DirEntry>, include_hidden: bool) -> Result<()> {
    let dir_entries = fs::read_dir(dir).map_err(|e| {
        FileIoError::ReadError(format!("Failed to read directory {}: {}", dir.display(), e))
    })?;

    for entry in dir_entries {
        let entry = entry.map_err(|e| {
            FileIoError::ReadError(format!("Failed to read directory entry: {}", e))
        })?;

        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Skip hidden files if not including them
        if !include_hidden && name.starts_with('.') {
            continue;
        }

        let metadata = entry.metadata().map_err(|e| {
            FileIoError::ReadError(format!(
                "Failed to read metadata for {}: {}",
                path.display(),
                e
            ))
        })?;

        let entry_type = if path.is_dir() {
            "directory"
        } else if path.is_file() {
            "file"
        } else if path.is_symlink() {
            "symlink"
        } else {
            "unknown"
        }
        .to_string();

        let size = if path.is_file() {
            Some(metadata.len())
        } else {
            None
        };

        let modified = metadata.modified().ok().and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs().to_string())
        });

        entries.push(DirEntry {
            name,
            path: path.to_string_lossy().to_string(),
            entry_type,
            size,
            modified,
        });
    }

    Ok(())
}

fn collect_entries_recursive(
    root: &Path,
    dir: &Path,
    entries: &mut Vec<DirEntry>,
    include_hidden: bool,
) -> Result<()> {
    collect_entries(dir, entries, include_hidden)?;

    let dir_entries = fs::read_dir(dir).map_err(|e| {
        FileIoError::ReadError(format!("Failed to read directory {}: {}", dir.display(), e))
    })?;

    for entry in dir_entries {
        let entry = entry.map_err(|e| {
            FileIoError::ReadError(format!("Failed to read directory entry: {}", e))
        })?;

        let path = entry.path();
        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // Skip hidden directories if not including them
            if !include_hidden && name.starts_with('.') {
                continue;
            }

            collect_entries_recursive(root, &path, entries, include_hidden)?;
        }
    }

    Ok(())
}

impl From<DirEntry> for Value {
    fn from(entry: DirEntry) -> Self {
        let mut obj = serde_json::Map::new();
        obj.insert("name".to_string(), Value::String(entry.name));
        obj.insert("path".to_string(), Value::String(entry.path));
        obj.insert("type".to_string(), Value::String(entry.entry_type));
        if let Some(size) = entry.size {
            obj.insert("size".to_string(), Value::Number(size.into()));
        }
        if let Some(modified) = entry.modified {
            obj.insert("modified".to_string(), Value::String(modified));
        }
        Value::Object(obj)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_list_directory() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_str().unwrap();

        // Create some files
        fs::write(dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(dir.path().join("file2.txt"), "content2").unwrap();

        let entries = list_directory(path, false, false).unwrap();
        assert!(entries.len() >= 2);
    }

    #[test]
    fn test_list_directory_recursive() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_str().unwrap();

        // Create nested structure
        let subdir = dir.path().join("subdir");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("file.txt"), "content").unwrap();

        let entries = list_directory(path, true, false).unwrap();
        assert!(entries.iter().any(|e| e.path.contains("subdir")));
        assert!(entries.iter().any(|e| e.path.contains("file.txt")));
    }
}
