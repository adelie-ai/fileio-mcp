#![deny(warnings)]

// Find text in files using grep crate (ripgrep's underlying library)

use crate::error::{FileIoError, Result};
use ignore::WalkBuilder;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Match {
    pub file_path: String,
    pub line_number: u64,
    pub column_start: usize,
    pub column_end: usize,
    pub matched_text: String,
    pub context_before: Option<String>,
    pub context_after: Option<String>,
}

/// Parameters for [`find_in_files`].
///
/// Grouped into a struct to keep the call signature readable (and to satisfy
/// clippy's `too_many_arguments` lint).
#[derive(Debug, Clone)]
pub struct FindInFilesParams<'a> {
    pub pattern: &'a str,
    pub path: &'a str,
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub max_count: Option<u64>,
    pub max_depth: Option<usize>,
    pub include_hidden: bool,
    pub file_glob: Option<&'a str>,
    pub exclude_glob: Option<&'a str>,
    pub whole_word: bool,
    pub multiline: bool,
}

/// Find text in files
pub fn find_in_files(params: &FindInFilesParams<'_>) -> Result<Vec<Match>> {
    let FindInFilesParams {
        pattern,
        path,
        case_sensitive,
        use_regex,
        max_count,
        max_depth,
        include_hidden,
        file_glob,
        exclude_glob,
        whole_word,
        multiline,
    } = *params;

    let expanded_path = shellexpand::full(path)
        .map_err(|e| {
            crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!(
                "Failed to expand path \'{}\': {}",
                path, e
            )))
        })
        .map(|expanded| expanded.into_owned())?;
    let path_obj = Path::new(&expanded_path);

    if !path_obj.exists() {
        return Err(FileIoError::NotFound(expanded_path.to_string()).into());
    }

    // Build regex pattern
    let regex_pattern = if use_regex {
        pattern.to_string()
    } else {
        // Escape special regex characters for literal matching
        regex::escape(pattern)
    };

    // Add word boundaries if whole_word is true
    let regex_pattern = if whole_word {
        format!(r"\b{}\b", regex_pattern)
    } else {
        regex_pattern
    };

    // Build regex with case sensitivity and multiline for matching
    let regex = {
        let mut builder = regex::RegexBuilder::new(&regex_pattern);
        if !case_sensitive {
            builder.case_insensitive(true);
        }
        builder.multi_line(multiline);
        builder.build()
    }
    .map_err(FileIoError::RegexError)?;

    let mut matches = Vec::new();
    let mut file_match_counts: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();

    // Build file walker
    let mut walker = WalkBuilder::new(path_obj);
    walker.hidden(include_hidden);

    if let Some(depth) = max_depth {
        walker.max_depth(Some(depth));
    }

    // Add file glob filters if specified
    if let Some(glob) = file_glob {
        walker.standard_filters(false);
        let glob_pattern = globset::GlobBuilder::new(glob)
            .build()
            .map_err(|e| FileIoError::InvalidPath(format!("Invalid file_glob pattern: {}", e)))?;
        let glob_matcher = glob_pattern.compile_matcher();
        walker.filter_entry(move |entry| {
            entry
                .path()
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| glob_matcher.is_match(name))
        });
    }

    if let Some(glob) = exclude_glob {
        let exclude_pattern = globset::GlobBuilder::new(glob).build().map_err(|e| {
            FileIoError::InvalidPath(format!("Invalid exclude_glob pattern: {}", e))
        })?;
        let exclude_matcher = exclude_pattern.compile_matcher();
        walker.filter_entry(move |entry| {
            !entry
                .path()
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| exclude_matcher.is_match(name))
        });
    }

    for result in walker.build() {
        let entry = result
            .map_err(|e| FileIoError::ReadError(format!("Error walking directory: {}", e)))?;

        let entry_path = entry.path();

        // Only search in files
        if !entry_path.is_file() {
            continue;
        }

        let file_path = entry_path.to_string_lossy().to_string();

        // Check max_count per file
        if let Some(max) = max_count {
            let count = file_match_counts.get(&file_path).copied().unwrap_or(0);
            if count >= max {
                continue;
            }
        }

        // Search in file
        let mut file_matches = Vec::new();

        let content_bytes = std::fs::read(entry_path).map_err(|e| {
            FileIoError::ReadError(format!("Failed to read file {}: {}", file_path, e))
        })?;

        let content = match String::from_utf8(content_bytes) {
            Ok(content) => content,
            Err(_) => continue,
        };

        for (line_number, line) in (1u64..).zip(content.lines()) {
            if let Some(max) = max_count {
                let count = file_match_counts.get(&file_path).copied().unwrap_or(0);
                if count >= max {
                    break;
                }
            }

            for mat in regex.find_iter(line) {
                file_matches.push(Match {
                    file_path: file_path.clone(),
                    line_number,
                    column_start: mat.start(),
                    column_end: mat.end(),
                    matched_text: mat.as_str().to_string(),
                    context_before: None,
                    context_after: None,
                });

                if let Some(max) = max_count {
                    let count = file_match_counts.entry(file_path.clone()).or_insert(0);
                    *count += 1;
                    if *count >= max {
                        break;
                    }
                }
            }
        }

        matches.extend(file_matches);
    }

    Ok(matches)
}

impl From<Match> for serde_json::Value {
    fn from(m: Match) -> Self {
        let mut obj = serde_json::Map::new();
        obj.insert(
            "file_path".to_string(),
            serde_json::Value::String(m.file_path),
        );
        obj.insert(
            "line_number".to_string(),
            serde_json::Value::Number(m.line_number.into()),
        );
        obj.insert(
            "column_start".to_string(),
            serde_json::Value::Number(m.column_start.into()),
        );
        obj.insert(
            "column_end".to_string(),
            serde_json::Value::Number(m.column_end.into()),
        );
        obj.insert(
            "matched_text".to_string(),
            serde_json::Value::String(m.matched_text),
        );
        if let Some(ctx) = m.context_before {
            obj.insert("context_before".to_string(), serde_json::Value::String(ctx));
        }
        if let Some(ctx) = m.context_after {
            obj.insert("context_after".to_string(), serde_json::Value::String(ctx));
        }
        serde_json::Value::Object(obj)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Build a `FindInFilesParams` with non-search fields defaulted, so tests
    /// only specify what they care about.
    fn params<'a>(pattern: &'a str, path: &'a str) -> FindInFilesParams<'a> {
        FindInFilesParams {
            pattern,
            path,
            case_sensitive: true,
            use_regex: false,
            max_count: None,
            max_depth: None,
            include_hidden: false,
            file_glob: None,
            exclude_glob: None,
            whole_word: false,
            multiline: false,
        }
    }

    #[test]
    fn test_find_in_files_literal() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_str().unwrap();

        fs::write(dir.path().join("test.txt"), "hello world\nfoo bar").unwrap();

        let matches = find_in_files(&params("hello", root)).unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].matched_text, "hello");
        assert_eq!(matches[0].line_number, 1);
    }

    #[test]
    fn test_find_in_files_regex() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_str().unwrap();

        fs::write(dir.path().join("test.txt"), "hello123\nworld456").unwrap();

        let matches = find_in_files(&FindInFilesParams {
            use_regex: true,
            ..params(r"\d+", root)
        })
        .unwrap();

        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_find_in_files_case_insensitive() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_str().unwrap();

        fs::write(dir.path().join("test.txt"), "Hello World").unwrap();

        let matches = find_in_files(&FindInFilesParams {
            case_sensitive: false,
            ..params("hello", root)
        })
        .unwrap();

        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_find_in_files_max_count() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_str().unwrap();

        fs::write(dir.path().join("test.txt"), "hello hello hello").unwrap();

        let matches = find_in_files(&FindInFilesParams {
            max_count: Some(2),
            ..params("hello", root)
        })
        .unwrap();

        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_find_in_files_skips_non_utf8_files() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_str().unwrap();

        fs::write(dir.path().join("text.txt"), "needle in text\n").unwrap();
        fs::write(dir.path().join("binary.bin"), [0xFFu8, 0x00, 0x80, 0xFE]).unwrap();

        let matches = find_in_files(&params("needle", root)).unwrap();

        assert_eq!(matches.len(), 1);
        assert!(matches[0].file_path.ends_with("text.txt"));
    }
}
