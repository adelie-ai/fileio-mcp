#![deny(warnings)]

// Structured, deterministic file edits (LLM-friendly)

use crate::error::{FileIoError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct EditFileRequest {
    pub path: String,
    pub edits: Vec<EditOperation>,

    #[serde(default)]
    pub create_if_missing: bool,

    #[serde(default)]
    pub dry_run: bool,

    #[serde(default)]
    pub return_content: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EditOperation {
    InsertAfter {
        search: String,
        text: String,
        #[serde(default)]
        use_regex: bool,
        #[serde(default = "default_occurrence")]
        occurrence: u32,
        #[serde(default = "default_require_match")]
        require_match: bool,
    },
    InsertBefore {
        search: String,
        text: String,
        #[serde(default)]
        use_regex: bool,
        #[serde(default = "default_occurrence")]
        occurrence: u32,
        #[serde(default = "default_require_match")]
        require_match: bool,
    },
    Replace {
        search: String,
        text: String,
        #[serde(default)]
        use_regex: bool,
        #[serde(default = "default_occurrence")]
        occurrence: u32,
        #[serde(default = "default_require_match")]
        require_match: bool,
    },
    Delete {
        search: String,
        #[serde(default)]
        use_regex: bool,
        #[serde(default = "default_occurrence")]
        occurrence: u32,
        #[serde(default = "default_require_match")]
        require_match: bool,
    },

    InsertAtLine {
        line: u64,
        text: String,
    },
    ReplaceLines {
        start_line: u64,
        end_line: u64,
        text: String,
    },
    DeleteLines {
        start_line: u64,
        end_line: u64,
    },
}

#[derive(Debug, Serialize)]
pub struct EditFileResult {
    pub path: String,
    pub changed: bool,
    pub applied_edits: usize,
    pub dry_run: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

fn default_occurrence() -> u32 {
    1
}

fn default_require_match() -> bool {
    true
}

pub fn edit_file(req: EditFileRequest) -> Result<EditFileResult> {
    let expanded_path = shellexpand::full(&req.path)
        .map_err(|e| {
            crate::error::FileIoMcpError::from(crate::error::FileIoError::InvalidPath(format!(
                "Failed to expand path '{}'': {}",
                req.path, e
            )))
        })
        .map(|expanded| expanded.into_owned())?;

    let path_obj = Path::new(&expanded_path);

    // Load file content (or create empty if allowed)
    let original_content = match fs::read_to_string(&expanded_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && req.create_if_missing => String::new(),
        Err(e) => {
            return Err(crate::error::FileIoMcpError::from(FileIoError::from_io_error(
                "read file",
                &expanded_path,
                e,
            )))
        }
    };

    if !req.create_if_missing && !path_obj.exists() {
        return Err(FileIoError::NotFound(expanded_path).into());
    }

    let mut content = original_content.clone();
    let mut applied = 0usize;

    for edit in req.edits {
        let before = content.clone();

        match edit {
            EditOperation::InsertAfter {
                search,
                text,
                use_regex,
                occurrence,
                require_match,
            } => {
                let Some((_, end)) = find_nth_span(&content, &search, use_regex, occurrence)? else {
                    if require_match {
                        return Err(FileIoError::InvalidPath(format!(
                            "Edit failed: search pattern not found (insert_after): {}",
                            search
                        ))
                        .into());
                    }
                    continue;
                };
                content.insert_str(end, &text);
            }
            EditOperation::InsertBefore {
                search,
                text,
                use_regex,
                occurrence,
                require_match,
            } => {
                let Some((start, _)) = find_nth_span(&content, &search, use_regex, occurrence)?
                else {
                    if require_match {
                        return Err(FileIoError::InvalidPath(format!(
                            "Edit failed: search pattern not found (insert_before): {}",
                            search
                        ))
                        .into());
                    }
                    continue;
                };
                content.insert_str(start, &text);
            }
            EditOperation::Replace {
                search,
                text,
                use_regex,
                occurrence,
                require_match,
            } => {
                let Some((start, end)) = find_nth_span(&content, &search, use_regex, occurrence)?
                else {
                    if require_match {
                        return Err(FileIoError::InvalidPath(format!(
                            "Edit failed: search pattern not found (replace): {}",
                            search
                        ))
                        .into());
                    }
                    continue;
                };
                content.replace_range(start..end, &text);
            }
            EditOperation::Delete {
                search,
                use_regex,
                occurrence,
                require_match,
            } => {
                let Some((start, end)) = find_nth_span(&content, &search, use_regex, occurrence)?
                else {
                    if require_match {
                        return Err(FileIoError::InvalidPath(format!(
                            "Edit failed: search pattern not found (delete): {}",
                            search
                        ))
                        .into());
                    }
                    continue;
                };
                content.replace_range(start..end, "");
            }
            EditOperation::InsertAtLine { line, text } => {
                let line_usize = u64_to_usize(line, "line")?;
                let insert_at = line_start_offset(&content, line_usize, true)?;
                content.insert_str(insert_at, &text);
            }
            EditOperation::ReplaceLines {
                start_line,
                end_line,
                text,
            } => {
                let start = u64_to_usize(start_line, "start_line")?;
                let end = u64_to_usize(end_line, "end_line")?;
                let (start_off, end_off) = line_range_offsets(&content, start, end, true)?;
                let removed = &content[start_off..end_off];

                let mut replacement = text;
                if removed.ends_with('\n') && !replacement.ends_with('\n') {
                    replacement.push('\n');
                }
                content.replace_range(start_off..end_off, &replacement);
            }
            EditOperation::DeleteLines {
                start_line,
                end_line,
            } => {
                let start = u64_to_usize(start_line, "start_line")?;
                let end = u64_to_usize(end_line, "end_line")?;
                let (start_off, end_off) = line_range_offsets(&content, start, end, true)?;
                content.replace_range(start_off..end_off, "");
            }
        }

        if content != before {
            applied += 1;
        }
    }

    let changed = content != original_content;

    if changed && !req.dry_run {
        // Reuse existing atomic writer (it also creates parent dirs if needed)
        crate::operations::write_file::write_file(&expanded_path, &content, false)?;
    }

    Ok(EditFileResult {
        path: expanded_path,
        changed,
        applied_edits: applied,
        dry_run: req.dry_run,
        content: if req.return_content || req.dry_run {
            Some(content)
        } else {
            None
        },
    })
}

fn u64_to_usize(v: u64, field: &str) -> Result<usize> {
    usize::try_from(v).map_err(|_| {
        FileIoError::InvalidLineNumbers(format!("{} is too large: {}", field, v)).into()
    })
}

fn find_nth_span(
    haystack: &str,
    needle: &str,
    use_regex: bool,
    occurrence: u32,
) -> Result<Option<(usize, usize)>> {
    if occurrence == 0 {
        return Err(FileIoError::InvalidLineNumbers(
            "occurrence must be >= 1".to_string(),
        )
        .into());
    }

    if needle.is_empty() {
        return Err(FileIoError::InvalidPath("search must not be empty".to_string()).into());
    }

    if use_regex {
        let re = regex::Regex::new(needle).map_err(FileIoError::from)?;
        let mut i = 0u32;
        for m in re.find_iter(haystack) {
            i += 1;
            if i == occurrence {
                return Ok(Some((m.start(), m.end())));
            }
        }
        Ok(None)
    } else {
        let mut start_from = 0usize;
        let mut i = 0u32;
        while let Some(pos) = haystack[start_from..].find(needle) {
            let abs = start_from + pos;
            i += 1;
            if i == occurrence {
                return Ok(Some((abs, abs + needle.len())));
            }
            start_from = abs + needle.len();
            if start_from > haystack.len() {
                break;
            }
        }
        Ok(None)
    }
}

fn compute_line_starts(content: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (idx, b) in content.bytes().enumerate() {
        if b == b'\n' {
            starts.push(idx + 1);
        }
    }
    starts
}

fn effective_line_count(content: &str) -> usize {
    if content.is_empty() {
        1
    } else {
        compute_line_starts(content).len()
    }
}

fn line_start_offset(content: &str, line: usize, allow_past_end: bool) -> Result<usize> {
    if line == 0 {
        return Err(FileIoError::InvalidLineNumbers("line must be >= 1".to_string()).into());
    }

    let count = effective_line_count(content);
    if allow_past_end {
        if line > count + 1 {
            return Err(FileIoError::InvalidLineNumbers(format!(
                "Invalid line number: {} (file has {} lines)",
                line, count
            ))
            .into());
        }
        if line == count + 1 {
            return Ok(content.len());
        }
    } else if line > count {
        return Err(FileIoError::InvalidLineNumbers(format!(
            "Invalid line number: {} (file has {} lines)",
            line, count
        ))
        .into());
    }

    if content.is_empty() {
        return Ok(0);
    }

    let starts = compute_line_starts(content);
    starts
        .get(line - 1)
        .copied()
        .ok_or_else(|| FileIoError::InvalidLineNumbers(format!(
            "Invalid line number: {} (file has {} lines)",
            line, starts.len()
        )))
        .map_err(|e| e.into())
}

fn line_range_offsets(
    content: &str,
    start_line: usize,
    end_line: usize,
    allow_empty_file_as_one_line: bool,
) -> Result<(usize, usize)> {
    if start_line == 0 || end_line == 0 {
        return Err(
            FileIoError::InvalidLineNumbers("line numbers must be >= 1".to_string()).into(),
        );
    }
    if start_line > end_line {
        return Err(FileIoError::InvalidLineNumbers(format!(
            "start_line ({}) must be <= end_line ({})",
            start_line, end_line
        ))
        .into());
    }

    let count = if allow_empty_file_as_one_line {
        effective_line_count(content)
    } else if content.is_empty() {
        0
    } else {
        compute_line_starts(content).len()
    };

    if start_line > count || end_line > count {
        return Err(FileIoError::InvalidLineNumbers(format!(
            "Invalid line range: {}..{} (file has {} lines)",
            start_line, end_line, count
        ))
        .into());
    }

    if content.is_empty() {
        // empty file is treated as 1 empty line
        return Ok((0, 0));
    }

    let starts = compute_line_starts(content);
    let start_off = *starts.get(start_line - 1).ok_or_else(|| {
        FileIoError::InvalidLineNumbers(format!(
            "Invalid start_line: {} (file has {} lines)",
            start_line,
            starts.len()
        ))
    })?;

    let end_off = if end_line < starts.len() {
        *starts.get(end_line).unwrap_or(&content.len())
    } else {
        content.len()
    };

    Ok((start_off, end_off))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn insert_after_anchor_string() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "[deps]\nanyhow=\"1\"\n").unwrap();

        let res = edit_file(EditFileRequest {
            path: path.to_string_lossy().to_string(),
            edits: vec![EditOperation::InsertAfter {
                search: "[deps]\n".to_string(),
                text: "rusqlite=\"0.31\"\n".to_string(),
                use_regex: false,
                occurrence: 1,
                require_match: true,
            }],
            create_if_missing: false,
            dry_run: false,
            return_content: true,
        })
        .unwrap();

        assert!(res.changed);
        assert!(res.content.unwrap().contains("rusqlite"));
    }

    #[test]
    fn replace_lines_preserves_newline_when_replacing_full_line() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("b.txt");
        fs::write(&path, "a\nb\nc\n").unwrap();

        let res = edit_file(EditFileRequest {
            path: path.to_string_lossy().to_string(),
            edits: vec![EditOperation::ReplaceLines {
                start_line: 2,
                end_line: 2,
                text: "B".to_string(),
            }],
            create_if_missing: false,
            dry_run: false,
            return_content: true,
        })
        .unwrap();

        assert_eq!(res.content.unwrap(), "a\nB\nc\n");
    }
}
