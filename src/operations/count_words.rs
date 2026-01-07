#![deny(warnings)]

// Count words in a file

use crate::error::{FileIoError, Result};
use std::fs;
use std::path::Path;

/// Count words in a file (whitespace-separated)
pub fn count_words(path: &str) -> Result<u64> {
    let path_obj = Path::new(path);

    if !path_obj.exists() {
        return Err(FileIoError::NotFound(path.to_string()).into());
    }

    if !path_obj.is_file() {
        return Err(FileIoError::InvalidPath(format!("{} is not a file", path)).into());
    }

    let content = fs::read_to_string(path).map_err(|e| {
        FileIoError::ReadError(format!("Failed to read file {}: {}", path, e))
    })?;

    let word_count = content.split_whitespace().count() as u64;

    Ok(word_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_count_words_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "hello world").unwrap();
        writeln!(file, "foo bar").unwrap();
        let path = file.path().to_str().unwrap();

        let count = count_words(path).unwrap();
        assert_eq!(count, 4); // hello, world, foo, bar
    }

    #[test]
    fn test_count_words_empty() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();

        let count = count_words(path).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_words_multiple_spaces() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "word1    word2   word3").unwrap();
        let path = file.path().to_str().unwrap();

        let count = count_words(path).unwrap();
        assert_eq!(count, 3);
    }
}
