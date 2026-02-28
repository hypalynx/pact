use serde_json::{Value, json};
use std::fs;
use std::path::Path;

const MAX_FILE_SIZE: usize = 65536; // 64KB limit per read

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub args: serde_json::Map<String, Value>,
}

pub fn get_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "Read",
                "description": "Read the complete contents of a file. Accepts absolute paths (starting with /) or relative paths from current directory. Returns the raw text content. Files larger than 64KB are truncated with a warning.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "filePath": {
                            "type": "string",
                            "description": "Path to the file - either absolute (e.g., /etc/hosts) or relative (e.g., ./README.md)"
                        }
                    },
                    "required": ["filePath"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "Glob",
                "description": "Find files matching a glob pattern from the current directory. Use * to match any characters within a name, ** to match across directories. Returns list of matching file paths.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Glob pattern (e.g., '*.rs', 'src/**/*.ts', 'test_*.py')"
                        }
                    },
                    "required": ["pattern"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "Grep",
                "description": "Search for lines matching a pattern in files. Use regex patterns to find text across one or more files.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Regex pattern to search for"
                        },
                        "files": {
                            "type": "string",
                            "description": "File path or glob pattern (e.g., '*.rs', './src/main.rs', 'src/**/*.py')"
                        }
                    },
                    "required": ["pattern", "files"],
                    "additionalProperties": false
                }
            }
        }),
    ]
}

pub fn execute_tool(tool_call: &ToolCall) -> (String, String) {
    match tool_call.name.as_str() {
        "Read" => execute_read(&tool_call.args),
        "Glob" => execute_glob(&tool_call.args),
        "Grep" => execute_grep(&tool_call.args),
        // Support legacy lowercase "read" for backwards compatibility
        "read" => execute_read(&tool_call.args),
        _ => {
            let error = format!("Unknown tool: {}", tool_call.name);
            (error.clone(), error)
        }
    }
}

fn execute_read(args: &serde_json::Map<String, Value>) -> (String, String) {
    let path = match args.get("filePath").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            let error = "Error: 'filePath' parameter is required and must be a string".to_string();
            return (error.clone(), error);
        }
    };

    // Accept both absolute and relative paths
    let full_path = if Path::new(path).is_absolute() {
        path.to_string()
    } else {
        // Relative path - resolve from current directory
        match std::env::current_dir() {
            Ok(cwd) => {
                let full = cwd.join(path);
                match full.to_str() {
                    Some(p) => p.to_string(),
                    None => {
                        return (
                            "Error: invalid path".to_string(),
                            "Error: path contains invalid UTF-8".to_string(),
                        );
                    }
                }
            }
            Err(e) => return (format!("Error: {}", e), format!("Error: {}", e)),
        }
    };

    // Extract just the filename from the path
    let filename = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);

    // Try to read the file
    match fs::read_to_string(&full_path) {
        Ok(contents) => {
            let summary = format!("Reading {}", filename);
            // Check size
            if contents.len() > MAX_FILE_SIZE {
                let truncated = format!(
                    "File too large ({} bytes, max {}). Showing first {} bytes:\n\n{}",
                    contents.len(),
                    MAX_FILE_SIZE,
                    MAX_FILE_SIZE,
                    &contents[..MAX_FILE_SIZE]
                );
                (summary, truncated)
            } else {
                (summary, contents)
            }
        }
        Err(e) => {
            let error = format!("Error reading file '{}': {}", path, e);
            (error.clone(), error)
        }
    }
}

fn execute_glob(args: &serde_json::Map<String, Value>) -> (String, String) {
    let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            let error = "Error: 'pattern' parameter is required".to_string();
            return (error.clone(), error);
        }
    };

    // Use glob crate to find matching files
    match glob::glob(pattern) {
        Ok(paths) => {
            let mut matches = Vec::new();
            for entry in paths.flatten() {
                if let Some(path_str) = entry.to_str() {
                    matches.push(path_str.trim_start_matches("./").to_string());
                }
            }
            matches.sort();

            if matches.is_empty() {
                (
                    format!("No files match pattern: {}", pattern),
                    String::new(),
                )
            } else {
                let content = matches.join("\n");
                let summary = format!("Found {} files matching '{}'", matches.len(), pattern);
                (summary, content)
            }
        }
        Err(e) => {
            let error = format!("Error with glob pattern '{}': {}", pattern, e);
            (error.clone(), error)
        }
    }
}

fn execute_grep(args: &serde_json::Map<String, Value>) -> (String, String) {
    let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            let error = "Error: 'pattern' parameter is required".to_string();
            return (error.clone(), error);
        }
    };

    let files_pattern = match args.get("files").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            let error = "Error: 'files' parameter is required".to_string();
            return (error.clone(), error);
        }
    };

    // Compile regex pattern
    let regex = match regex::Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => {
            let error = format!("Invalid regex pattern: {}", e);
            return (error.clone(), error);
        }
    };

    // Get matching files from glob pattern
    let file_paths = match glob::glob(files_pattern) {
        Ok(paths) => paths.flatten().collect::<Vec<_>>(),
        Err(e) => {
            let error = format!("Error with glob pattern '{}': {}", files_pattern, e);
            return (error.clone(), error);
        }
    };

    if file_paths.is_empty() {
        return (
            format!("No files match pattern: {}", files_pattern),
            String::new(),
        );
    }

    let mut results = Vec::new();
    let mut match_count = 0;
    let file_count = file_paths.len();

    for file_path in file_paths {
        if !file_path.is_file() {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&file_path) {
            for (line_num, line) in content.lines().enumerate() {
                if regex.is_match(line) {
                    let file_display = file_path
                        .to_string_lossy()
                        .trim_start_matches("./")
                        .to_string();
                    results.push(format!("{}:{}: {}", file_display, line_num + 1, line));
                    match_count += 1;
                }
            }
        }
    }

    if results.is_empty() {
        (
            format!("No matches found for pattern: {}", pattern),
            String::new(),
        )
    } else {
        let content = results.join("\n");
        let summary = format!("Found {} matches in {} files", match_count, file_count);
        (summary, content)
    }
}
