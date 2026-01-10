#![deny(warnings)]

// Read lines from a file with windowing support

use crate::error::{FileIoError, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};

/// Read lines from a file with optional windowing
pub fn read_lines(
    path: &str,
    start_line: Option<u64>,
    end_line: Option<u64>,
    line_count: Option<u64>,
    start_offset: Option<u64>,
) -> Result<Vec<String>> {
    let expanded_path = shellexpand::full(path)
        .map_err(|e| crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!("Failed to expand path \'{}\': {}", path, e))))
        .map(|expanded| expanded.into_owned())?;
    let file = File::open(&expanded_path)
        .map_err(|e| FileIoError::ReadError(format!("Failed to open file {}: {}", expanded_path, e)))?;

    let reader = BufReader::new(file);
    let lines: Vec<String> = reader
        .lines()
        .enumerate()
        .map(|(i, line)| {
            line.map_err(|e| {
                FileIoError::ReadError(format!("Failed to read line {}: {}", i + 1, e))
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // Determine the range of lines to return
    let start = if let Some(start) = start_line {
        if start == 0 {
            return Err(FileIoError::InvalidLineNumbers(
                "Line numbers start at 1".to_string(),
            )
            .into());
        }
        (start - 1) as usize
    } else if let Some(offset) = start_offset {
        offset as usize
    } else {
        0
    };

    let end = if let Some(end) = end_line {
        if end == 0 {
            return Err(FileIoError::InvalidLineNumbers(
                "Line numbers start at 1".to_string(),
            )
            .into());
        }
        if end < start_line.unwrap_or(1) {
            return Err(FileIoError::InvalidLineNumbers(
                "end_line must be >= start_line".to_string(),
            )
            .into());
        }
        end as usize
    } else if let Some(count) = line_count {
        start + count as usize
    } else {
        lines.len()
    };

    // Validate bounds
    if start > lines.len() {
        return Err(FileIoError::InvalidLineNumbers(format!(
            "start_line {} exceeds file length {}",
            start + 1,
            lines.len()
        ))
        .into());
    }

    let end = end.min(lines.len());

    if start > end {
        return Err(FileIoError::InvalidLineNumbers(
            "start_line must be <= end_line".to_string(),
        )
        .into());
    }

    Ok(lines[start..end].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_read_all_lines() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "line 1").unwrap();
        writeln!(file, "line 2").unwrap();
        writeln!(file, "line 3").unwrap();
        let path = file.path().to_str().unwrap();

        let lines = read_lines(path, None, None, None, None).unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line 1");
        assert_eq!(lines[2], "line 3");
    }

    #[test]
    fn test_read_lines_with_range() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "line 1").unwrap();
        writeln!(file, "line 2").unwrap();
        writeln!(file, "line 3").unwrap();
        writeln!(file, "line 4").unwrap();
        let path = file.path().to_str().unwrap();

        let lines = read_lines(path, Some(2), Some(3), None, None).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "line 2");
        assert_eq!(lines[1], "line 3");
    }

    #[test]
    fn test_read_lines_with_count() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "line 1").unwrap();
        writeln!(file, "line 2").unwrap();
        writeln!(file, "line 3").unwrap();
        let path = file.path().to_str().unwrap();

        let lines = read_lines(path, Some(1), None, Some(2), None).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "line 1");
        assert_eq!(lines[1], "line 2");
    }

    #[test]
    fn test_read_lines_with_offset() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "line 1").unwrap();
        writeln!(file, "line 2").unwrap();
        writeln!(file, "line 3").unwrap();
        let path = file.path().to_str().unwrap();

        let lines = read_lines(path, None, None, Some(2), Some(1)).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "line 2");
        assert_eq!(lines[1], "line 3");
    }
}
