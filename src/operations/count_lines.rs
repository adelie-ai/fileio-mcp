#![deny(warnings)]

// Count lines in a file

use crate::error::{FileIoError, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, serde::Serialize)]
pub struct LineCountResult {
    pub path: String,
    pub status: String,
    pub lines: Option<u64>,
    pub exists: bool,
}

/// Count lines in files
/// Returns a vector of results: { path, status, lines }
pub fn count_lines(paths: &[&str]) -> Result<Vec<LineCountResult>> {
    let mut results = Vec::new();
    for path in paths {
        match count_lines_single(path) {
            Ok(count) => results.push(LineCountResult {
                path: path.to_string(),
                status: "ok".to_string(),
                lines: Some(count),
                exists: true,
            }),
            Err(e) => {
                // Map NotFound to a clear status; other errors include message
                let is_not_found = matches!(e, crate::error::FileIoMcpError::FileIo(crate::error::FileIoError::NotFound(_)));
                let status = if is_not_found {
                    "error: not found".to_string()
                } else {
                    format!("error: {}", e)
                };
                results.push(LineCountResult {
                    path: path.to_string(),
                    status,
                    lines: None,
                    exists: !is_not_found, // false if not found
                });
            }
        }
    }
    Ok(results)
}

/// Count lines in a single file
pub fn count_lines_single(path: &str) -> Result<u64> {
    let expanded_path = shellexpand::full(path)
        .map_err(|e| crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!("Failed to expand path \'{}\': {}", path, e))))
        .map(|expanded| expanded.into_owned())?;
    let path_obj = Path::new(&expanded_path);

    if !path_obj.exists() {
        return Err(FileIoError::NotFound(expanded_path.to_string()).into());
    }

    if !path_obj.is_file() {
        return Err(FileIoError::InvalidPath(format!("{} is not a file", expanded_path)).into());
    }

    let file = File::open(&expanded_path)
        .map_err(|e| crate::error::FileIoMcpError::from(FileIoError::from_io_error("open file", &expanded_path, e)))?;

    let reader = BufReader::new(file);
    let line_count = reader.lines().count() as u64;

    Ok(line_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_count_lines_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "line 1").unwrap();
        writeln!(file, "line 2").unwrap();
        writeln!(file, "line 3").unwrap();
        let path = file.path().to_str().unwrap();

        let results = count_lines(&[path]).unwrap();
        let r = &results[0];
        assert_eq!(r.path, path.to_string());
        assert_eq!(r.status, "ok");
        assert_eq!(r.lines, Some(3));
    }

    #[test]
    fn test_count_lines_empty() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();

        let results = count_lines(&[path]).unwrap();
        let r = &results[0];
        assert_eq!(r.status, "ok");
        assert_eq!(r.lines, Some(0));
    }

    #[test]
    fn test_count_lines_single_line_no_newline() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "single line").unwrap();
        let path = file.path().to_str().unwrap();

        let results = count_lines(&[path]).unwrap();
        let r = &results[0];
        assert_eq!(r.status, "ok");
        assert_eq!(r.lines, Some(1));
    }
}
