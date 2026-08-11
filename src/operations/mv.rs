#![deny(warnings)]

// Move or rename files or directories

use crate::error::{FileIoError, Result};
use crate::operations::path_utils::{expand_glob, is_glob_pattern};
use std::fs;
use std::path::Path;

/// Move or rename files or directories (supports glob patterns and arrays of paths)
#[derive(Debug, serde::Serialize)]
pub struct OpResult {
    pub path: String,
    pub status: String,
    pub exists: bool,
}

/// Move or rename files or directories (supports glob patterns and arrays of paths)
/// Returns per-source results and does not fail the whole call for per-file errors.
#[tracing::instrument(skip_all)]
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
                let s = match_path.to_str().ok_or_else(|| {
                    FileIoError::InvalidPath(format!(
                        "Path is not valid UTF-8: {}",
                        match_path.display()
                    ))
                })?;
                all_sources.push(s.to_string());
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
            let file_name = source_path_obj.file_name().ok_or_else(|| {
                FileIoError::InvalidPath(format!(
                    "Source path has no file name (is it the root?): {}",
                    source_path
                ))
            })?;
            dest_path.join(file_name)
        } else {
            dest_path.to_path_buf()
        };

        let dest_str = dest.to_str().ok_or_else(|| {
            FileIoError::InvalidPath(format!(
                "Destination path is not valid UTF-8: {}",
                dest.display()
            ))
        })?;
        match mv_single(source_path, dest_str) {
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
