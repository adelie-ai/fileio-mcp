#![deny(warnings)]

// Count words in a file

use crate::error::{FileIoError, Result};
use std::fs;
use std::path::Path;

#[derive(Debug, serde::Serialize)]
pub struct WordCountResult {
    pub path: String,
    pub status: String,
    pub words: Option<u64>,
    pub exists: bool,
}

/// Count words in files (whitespace-separated)
/// Returns a vector of results: { path, status, words }
pub fn count_words(paths: &[&str]) -> Result<Vec<WordCountResult>> {
    let mut results = Vec::new();
    for path in paths {
        match count_words_single(path) {
            Ok(count) => results.push(WordCountResult {
                path: path.to_string(),
                status: "ok".to_string(),
                words: Some(count),
                exists: true,
            }),
            Err(e) => {
                let is_not_found = matches!(e, crate::error::FileIoMcpError::FileIo(crate::error::FileIoError::NotFound(_)));
                let status = if is_not_found {
                    "error: not found".to_string()
                } else {
                    format!("error: {}", e)
                };
                results.push(WordCountResult {
                    path: path.to_string(),
                    status,
                    words: None,
                    exists: !is_not_found,
                });
            }
        }
    }
    Ok(results)
}

/// Count words in a single file (whitespace-separated)
pub fn count_words_single(path: &str) -> Result<u64> {
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

    let content = fs::read_to_string(&expanded_path).map_err(|e| {
        crate::error::FileIoMcpError::from(FileIoError::from_io_error("read file", &expanded_path, e))
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

        let results = count_words(&[path]).unwrap();
        let r = &results[0];
        assert_eq!(r.status, "ok");
        assert_eq!(r.words, Some(4)); // hello, world, foo, bar
    }

    #[test]
    fn test_count_words_empty() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();

        let results = count_words(&[path]).unwrap();
        let r = &results[0];
        assert_eq!(r.status, "ok");
        assert_eq!(r.words, Some(0));
    }

    #[test]
    fn test_count_words_multiple_spaces() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "word1    word2   word3").unwrap();
        let path = file.path().to_str().unwrap();

        let results = count_words(&[path]).unwrap();
        let r = &results[0];
        assert_eq!(r.status, "ok");
        assert_eq!(r.words, Some(3));
    }
}
