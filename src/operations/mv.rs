#![deny(warnings)]

// Move or rename files or directories

use crate::error::{FileIoError, Result};
use globset::{Glob, GlobMatcher};
use std::fs;
use std::path::{Path, PathBuf};

/// Check if a string contains glob patterns
fn is_glob_pattern(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[') || s.contains('{')
}

/// Expand glob pattern to matching paths
fn expand_glob(pattern: &str) -> Result<Vec<PathBuf>> {
    let expanded_pattern = shellexpand::full(pattern)
        .map_err(|e| {
            crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!(
                "Failed to expand path \'{}\': {}",
                pattern, e
            )))
        })
        .map(|expanded| expanded.into_owned())?;
    let path = Path::new(&expanded_pattern);
    let (base_dir, glob_str) = if let Some(parent) = path.parent() {
        if parent.as_os_str().is_empty() {
            (
                Path::new("."),
                path.file_name().and_then(|n| n.to_str()).unwrap_or(pattern),
            )
        } else {
            (
                parent,
                path.file_name().and_then(|n| n.to_str()).unwrap_or(pattern),
            )
        }
    } else {
        (Path::new("."), pattern)
    };

    let glob = Glob::new(glob_str).map_err(|e| {
        FileIoError::InvalidPath(format!("Invalid glob pattern {}: {}", pattern, e))
    })?;
    let matcher: GlobMatcher = glob.compile_matcher();

    let mut matches = Vec::new();
    let entries = fs::read_dir(base_dir).map_err(|e| {
        FileIoError::ReadError(format!(
            "Failed to read directory {}: {}",
            base_dir.display(),
            e
        ))
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            FileIoError::ReadError(format!("Failed to read directory entry: {}", e))
        })?;
        let entry_path = entry.path();
        if let Some(file_name) = entry_path.file_name().and_then(|n| n.to_str()) {
            if matcher.is_match(file_name) {
                matches.push(entry_path);
            }
        }
    }

    Ok(matches)
}

/// Move or rename files or directories (supports glob patterns and arrays of paths)
#[derive(Debug, serde::Serialize)]
pub struct OpResult {
    pub path: String,
    pub status: String,
    pub exists: bool,
}

/// Move or rename files or directories (supports glob patterns and arrays of paths)
/// Returns per-source results and does not fail the whole call for per-file errors.
pub fn mv(sources: &[&str], destination: &str) -> Result<Vec<OpResult>> {
    let expanded_dest = shellexpand::full(destination)
        .map_err(|e| {
            crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!(
                "Failed to expand path \'{}\': {}",
                destination, e
            )))
        })
        .map(|expanded| expanded.into_owned())?;
    let dest_path = Path::new(&expanded_dest);
    let dest_is_dir = dest_path.exists() && dest_path.is_dir();

    let mut all_sources = Vec::new();

    for source in sources {
        // Check if source contains glob patterns
        if is_glob_pattern(source) {
            // Expand glob and add matches
            let matches = expand_glob(source)?;

            if matches.is_empty() {
                return Err(
                    FileIoError::NotFound(format!("No files match pattern: {}", source)).into(),
                );
            }

            for match_path in matches {
                all_sources.push(match_path.to_str().unwrap().to_string());
            }
        } else {
            // Single path
            all_sources.push(source.to_string());
        }
    }

    if all_sources.len() > 1 && !dest_is_dir {
        return Err(FileIoError::InvalidPath(format!(
            "Multiple sources provided but destination '{}' is not a directory",
            destination
        ))
        .into());
    }

    let mut results = Vec::new();
    for source_path in &all_sources {
        let dest = if dest_is_dir {
            let source_path_obj = Path::new(source_path);
            dest_path.join(source_path_obj.file_name().unwrap())
        } else {
            dest_path.to_path_buf()
        };

        match mv_single(source_path, dest.to_str().unwrap()) {
            Ok(()) => results.push(OpResult {
                path: source_path.clone(),
                status: "ok".to_string(),
                exists: true,
            }),
            Err(e) => {
                let is_not_found = matches!(
                    e,
                    crate::error::FileIoMcpError::FileIo(crate::error::FileIoError::NotFound(_))
                );
                results.push(OpResult {
                    path: source_path.clone(),
                    status: format!("error: {}", e),
                    exists: !is_not_found,
                });
            }
        }
    }

    Ok(results)
}

/// Move a single file or directory
fn mv_single(source: &str, destination: &str) -> Result<()> {
    let source_path = Path::new(source);

    if !source_path.exists() {
        return Err(FileIoError::NotFound(source.to_string()).into());
    }

    // Create parent directories if needed
    let dest_path = Path::new(destination);
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            FileIoError::WriteError(format!(
                "Failed to create parent directories for {}: {}",
                destination, e
            ))
        })?;
    }

    fs::rename(source, destination).map_err(|e| {
        use std::io::ErrorKind;
        match e.kind() {
            ErrorKind::PermissionDenied => {
                crate::error::FileIoMcpError::from(FileIoError::PermissionDenied(format!(
                    "Permission denied when moving {} to {}: {}",
                    source, destination, e
                )))
            }
            ErrorKind::NotFound => crate::error::FileIoMcpError::from(FileIoError::NotFound(
                format!("Source not found when moving: {}", source),
            )),
            _ => crate::error::FileIoMcpError::from(FileIoError::from_io_error(
                "move",
                &format!("{} to {}", source, destination),
                e,
            )),
        }
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_mv_file() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("source.txt");
        let dst = dir.path().join("dest.txt");

        fs::write(&src, "content").unwrap();
        let results = mv(&[src.to_str().unwrap()], dst.to_str().unwrap()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, "ok");

        assert!(!src.exists());
        assert!(dst.exists());
        assert_eq!(fs::read_to_string(&dst).unwrap(), "content");
    }

    #[test]
    fn test_mv_glob() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::write(base.join("file1.txt"), "content1").unwrap();
        fs::write(base.join("file2.txt"), "content2").unwrap();
        fs::write(base.join("other.log"), "content3").unwrap();

        let dst_dir = base.join("dest");
        fs::create_dir_all(&dst_dir).unwrap();

        let pattern = base.join("*.txt").to_str().unwrap().to_string();
        let results = mv(&[&pattern], dst_dir.to_str().unwrap()).unwrap();
        assert!(results.iter().all(|r| r.status == "ok"));

        assert!(!base.join("file1.txt").exists());
        assert!(!base.join("file2.txt").exists());
        assert!(base.join("other.log").exists());
        assert!(dst_dir.join("file1.txt").exists());
        assert!(dst_dir.join("file2.txt").exists());
    }
}
