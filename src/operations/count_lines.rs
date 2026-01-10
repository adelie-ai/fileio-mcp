#![deny(warnings)]

// Count lines in a file

use crate::error::{FileIoError, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Count lines in a file
pub fn count_lines(path: &str) -> Result<u64> {
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
        .map_err(|e| FileIoError::ReadError(format!("Failed to open file {}: {}", expanded_path, e)))?;

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

        let count = count_lines(path).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_count_lines_empty() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();

        let count = count_lines(path).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_lines_single_line_no_newline() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "single line").unwrap();
        let path = file.path().to_str().unwrap();

        let count = count_lines(path).unwrap();
        assert_eq!(count, 1);
    }
}
