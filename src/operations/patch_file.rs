#![deny(warnings)]

// Apply patches to files (unified diff and add/remove lines formats)

use crate::error::{FileIoError, Result};
use std::fs;

/// Apply a patch to a file
pub fn patch_file(path: &str, patch: &str, format: Option<&str>) -> Result<()> {
    let format = format.unwrap_or("unified_diff");

    match format {
        "unified_diff" => apply_unified_diff(path, patch),
        "add_remove_lines" => apply_add_remove_lines(path, patch),
        _ => Err(FileIoError::PatchError(format!(
            "Unknown patch format: {}",
            format
        ))
        .into()),
    }
}

fn apply_unified_diff(path: &str, diff: &str) -> Result<()> {
    // Read current file content
    let current_content = fs::read_to_string(path)
        .map_err(|e| FileIoError::ReadError(format!("Failed to read file {}: {}", path, e)))?;

    let lines: Vec<&str> = current_content.lines().collect();
    let mut new_lines = Vec::new();
    let mut line_index = 0;

    // Simple unified diff parser
    // Format: @@ -start,count +start,count @@
    // Lines starting with - are removed, + are added, space are context
    for line in diff.lines() {
        if line.starts_with("@@") {
            // Parse hunk header
            // For simplicity, we'll just track line numbers
            continue;
        } else if line.starts_with('-') && !line.starts_with("---") {
            // Line to remove - skip it in original
            if line_index < lines.len() {
                line_index += 1;
            }
        } else if line.starts_with('+') && !line.starts_with("+++") {
            // Line to add
            new_lines.push(&line[1..]);
        } else if line.starts_with(' ') || line.is_empty() {
            // Context line - keep from original
            if line_index < lines.len() {
                new_lines.push(lines[line_index]);
                line_index += 1;
            }
        }
    }

    // Add remaining lines
    while line_index < lines.len() {
        new_lines.push(lines[line_index]);
        line_index += 1;
    }

    // Write patched content
    let new_content = new_lines.join("\n");
    fs::write(path, new_content).map_err(|e| {
        FileIoError::WriteError(format!("Failed to write patched file {}: {}", path, e))
    })?;

    Ok(())
}

fn apply_add_remove_lines(path: &str, patch_json: &str) -> Result<()> {
    // Parse JSON patch format
    // Expected format: { "operations": [{"type": "add|remove", "line": number, "content": "..."}] }
    let patch_data: serde_json::Value = serde_json::from_str(patch_json).map_err(|e| {
        FileIoError::PatchError(format!("Failed to parse patch JSON: {}", e))
    })?;

    // Read current file content
    let current_content = fs::read_to_string(path)
        .map_err(|e| FileIoError::ReadError(format!("Failed to read file {}: {}", path, e)))?;

    let mut lines: Vec<String> = current_content.lines().map(|s| s.to_string()).collect();

    // Get operations array
    let operations = patch_data
        .get("operations")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            FileIoError::PatchError("Patch JSON must contain 'operations' array".to_string())
        })?;

    // Sort operations by line number (descending) so removals don't affect indices
    let mut sorted_ops: Vec<_> = operations
        .iter()
        .filter_map(|op| {
            let op_type = op.get("type")?.as_str()?;
            let line = op.get("line")?.as_u64()? as usize;
            Some((op_type, line, op))
        })
        .collect();
    sorted_ops.sort_by(|a, b| b.1.cmp(&a.1)); // Sort descending by line number

    // Apply operations
    for (op_type, line_num, op) in sorted_ops {
        if line_num == 0 || line_num > lines.len() + 1 {
            return Err(FileIoError::PatchError(format!(
                "Invalid line number: {} (file has {} lines)",
                line_num,
                lines.len()
            ))
            .into());
        }

        match op_type {
            "add" => {
                let content = op
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        FileIoError::PatchError("Add operation must have 'content' field".to_string())
                    })?;
                lines.insert(line_num - 1, content.to_string());
            }
            "remove" => {
                if line_num > lines.len() {
                    return Err(FileIoError::PatchError(format!(
                        "Cannot remove line {} (file has {} lines)",
                        line_num,
                        lines.len()
                    ))
                    .into());
                }
                lines.remove(line_num - 1);
            }
            _ => {
                return Err(FileIoError::PatchError(format!(
                    "Unknown operation type: {}",
                    op_type
                ))
                .into());
            }
        }
    }

    // Write patched content
    let new_content = lines.join("\n");
    fs::write(path, new_content).map_err(|e| {
        FileIoError::WriteError(format!("Failed to write patched file {}: {}", path, e))
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_apply_add_remove_lines() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "line 1").unwrap();
        writeln!(file, "line 2").unwrap();
        writeln!(file, "line 3").unwrap();
        let path = file.path().to_str().unwrap();

        let patch = r#"{
            "operations": [
                {"type": "add", "line": 2, "content": "line 1.5"},
                {"type": "remove", "line": 3}
            ]
        }"#;

        patch_file(path, patch, Some("add_remove_lines")).unwrap();

        let content = fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1], "line 1.5");
    }
}
