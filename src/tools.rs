use serde_json::{Value, json};
use similar::TextDiff;
use std::fs;
use std::path::Path;

// Maximum number of lines to return from tool outputs
const MAX_OUTPUT_LINES: usize = 500;
const MAX_OUTPUT_CONTEXT: usize = 50; // Lines to show at end when truncating

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
                "description": "Read the complete contents of a file. Accepts absolute paths (starting with /) or relative paths from current directory. Returns the raw text content. Large files are truncated - use offset to read specific portions.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "filePath": {
                            "type": "string",
                            "description": "Path to the file - either absolute (e.g., /etc/hosts) or relative (e.g., ./README.md)"
                        },
                        "offset": {
                            "type": "integer",
                            "description": "Line number to start reading from (1-indexed). Useful for reading large files in chunks. Defaults to 1."
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
        json!({
            "type": "function",
            "function": {
                "name": "Bash",
                "description": "Execute a shell command and return output. The description field should briefly describe what the command does.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Shell command to execute"
                        },
                        "description": {
                            "type": "string",
                            "description": "Brief description of what this command does (for the UI summary)"
                        }
                    },
                    "required": ["command", "description"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "Write",
                "description": "Write or create a file with the given content. Creates parent directories if needed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to write (absolute or relative)"
                        },
                        "content": {
                            "type": "string",
                            "description": "The complete content to write to the file"
                        }
                    },
                    "required": ["path", "content"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "Edit",
                "description": "Find and replace text in an existing file. Finds old_string and replaces with new_string.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to edit"
                        },
                        "old_string": {
                            "type": "string",
                            "description": "Exact text to find and replace"
                        },
                        "new_string": {
                            "type": "string",
                            "description": "Replacement text"
                        }
                    },
                    "required": ["path", "old_string", "new_string"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "Webfetch",
                "description": "Fetch content from a URL via HTTP GET request. Returns text content with HTML tags stripped.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "URL to fetch (e.g., https://example.com/page)"
                        }
                    },
                    "required": ["url"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "TaskCreate",
                "description": "Create a new task with subject and optional description.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "subject": {
                            "type": "string",
                            "description": "Task title (brief, imperative form)"
                        },
                        "description": {
                            "type": "string",
                            "description": "Detailed description of task requirements"
                        }
                    },
                    "required": ["subject"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "TaskList",
                "description": "List all tasks with their status.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": [],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "TaskGet",
                "description": "Get full details of a specific task by ID.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "integer",
                            "description": "Task ID"
                        }
                    },
                    "required": ["id"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "TaskUpdate",
                "description": "Update task status or other fields.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "integer",
                            "description": "Task ID"
                        },
                        "status": {
                            "type": "string",
                            "description": "New status: pending, in_progress, or completed",
                            "enum": ["pending", "in_progress", "completed"]
                        }
                    },
                    "required": ["id"],
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
        "Bash" => execute_bash(&tool_call.args),
        "Write" => execute_write(&tool_call.args),
        "Edit" => execute_edit(&tool_call.args),
        "Webfetch" => execute_webfetch(&tool_call.args),
        // Support lowercase variants for backwards compatibility
        "bash" => execute_bash(&tool_call.args),
        "write" => execute_write(&tool_call.args),
        "edit" => execute_edit(&tool_call.args),
        "webfetch" => execute_webfetch(&tool_call.args),
        "glob" => execute_glob(&tool_call.args),
        "grep" => execute_grep(&tool_call.args),
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

    // Parse offset parameter (1-indexed, defaults to 1)
    let offset = args
        .get("offset")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(1)
        .saturating_sub(1); // Convert to 0-indexed

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
                        return ("Error: invalid path".to_string(), String::new());
                    }
                }
            }
            Err(e) => return (format!("Error: {}", e), String::new()),
        }
    };

    // Extract just the filename from the path
    let filename = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);

    // Try to read the file
    match fs::read_to_string(&full_path) {
        Ok(content) => {
            let summary = format!("Reading {}", filename);

            // Apply offset and limit output
            let all_lines: Vec<&str> = content.lines().collect();
            let total_lines = all_lines.len();

            // Apply offset
            let lines_from_offset: Vec<&str> = all_lines.iter().skip(offset).cloned().collect();
            let lines_from_offset_count = lines_from_offset.len();

            // Apply line limit with smart truncation
            let result = if lines_from_offset_count > MAX_OUTPUT_LINES {
                let head_lines = MAX_OUTPUT_LINES - MAX_OUTPUT_CONTEXT;
                let head: Vec<&str> = lines_from_offset.iter().take(head_lines).cloned().collect();
                let tail: Vec<&str> = lines_from_offset
                    .iter()
                    .skip(lines_from_offset_count - MAX_OUTPUT_CONTEXT)
                    .cloned()
                    .collect();
                let skipped = lines_from_offset_count - head_lines - MAX_OUTPUT_CONTEXT;
                let offset_note = if offset > 0 {
                    format!(" (starting from line {})", offset + 1)
                } else {
                    String::new()
                };
                format!(
                    "{}\n\n[... {} lines truncated{} ...]\n\n{}",
                    head.join("\n"),
                    skipped,
                    offset_note,
                    tail.join("\n")
                )
            } else {
                let lines_str = lines_from_offset.join("\n");
                if offset > 0 {
                    format!(
                        "{}\n\n[Read from line {} to {} of {} total]",
                        lines_str,
                        offset + 1,
                        offset + lines_from_offset_count,
                        total_lines
                    )
                } else {
                    lines_str
                }
            };

            // Return content for LLM (first element is UI summary, second is LLM result)
            (summary, result)
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
            return (error.clone(), String::new());
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
                let result = format!("No files match pattern: {}", pattern);
                (result.clone(), result)
            } else {
                let summary = format!("Found {} files matching '{}'", matches.len(), pattern);
                let result = matches.join("\n");
                (summary, result)
            }
        }
        Err(e) => {
            let error = format!("Error with glob pattern '{}': {}", pattern, e);
            (error, String::new())
        }
    }
}

fn execute_grep(args: &serde_json::Map<String, Value>) -> (String, String) {
    let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            let error = "Error: 'pattern' parameter is required".to_string();
            return (error, String::new());
        }
    };

    let files_pattern = match args.get("files").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            let error = "Error: 'files' parameter is required".to_string();
            return (error, String::new());
        }
    };

    // Compile regex pattern
    let regex = match regex::Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => {
            let error = format!("Invalid regex pattern: {}", e);
            return (error.clone(), String::new());
        }
    };

    // Get matching files from glob pattern
    let file_paths = match glob::glob(files_pattern) {
        Ok(paths) => paths.flatten().collect::<Vec<_>>(),
        Err(e) => {
            let error = format!("Error with glob pattern '{}': {}", files_pattern, e);
            return (error, String::new());
        }
    };

    if file_paths.is_empty() {
        return (
            format!("No files match pattern: {}", files_pattern),
            String::new(),
        );
    }

    let mut matches = Vec::new();
    let file_count = file_paths.len();

    for file_path in file_paths {
        if !file_path.is_file() {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&file_path) {
            for (line_num, line) in content.lines().enumerate() {
                if regex.is_match(line) {
                    let path_str = file_path.to_string_lossy();
                    matches.push(format!("{}:{}: {}", path_str, line_num + 1, line));
                }
            }
        }
    }

    if matches.is_empty() {
        let result = format!("No matches found for pattern: {}", pattern);
        (result.clone(), result)
    } else {
        let summary = format!("Found {} matches in {} files", matches.len(), file_count);
        let result = matches.join("\n");
        (summary, result)
    }
}

fn execute_bash(args: &serde_json::Map<String, Value>) -> (String, String) {
    let command = match args.get("command").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => {
            let error = "Error: 'command' parameter is required and must be a string".to_string();
            return (error.clone(), error);
        }
    };

    let description = args
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or(command);

    // Validate command for dangerous operations
    match validate_bash_command(command) {
        ValidationResult::HardBlocked(reason) => {
            let error = format!("Command blocked for safety: {}", reason);
            return (error.clone(), error);
        }
        ValidationResult::SoftBlocked(reason) => {
            let warning = format!(
                "⚠️  WARNING: This command is potentially dangerous: {}. Confirm execution to proceed.",
                reason
            );
            return (warning.clone(), warning);
        }
        ValidationResult::Safe => {}
    }

    execute_bash_unchecked(command, Some(description))
}

/// Execute bash command without validation - used only after user confirmation
pub fn execute_bash_unchecked(command: &str, description_opt: Option<&str>) -> (String, String) {
    const MAX_OUTPUT_BYTES: usize = 65536; // 64KB

    let description = description_opt.unwrap_or(command);

    // Execute command with timeout
    match std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
    {
        Ok(output) => {
            let mut result = String::new();
            result.push_str(&String::from_utf8_lossy(&output.stdout));
            if !output.stderr.is_empty() {
                result.push_str(&String::from_utf8_lossy(&output.stderr));
            }

            // First apply line-based truncation for very long outputs
            let lines: Vec<&str> = result.lines().collect();
            let line_count = lines.len();
            let result = if line_count > MAX_OUTPUT_LINES {
                let head_lines = MAX_OUTPUT_LINES - MAX_OUTPUT_CONTEXT;
                let head: Vec<&str> = lines.iter().take(head_lines).cloned().collect();
                let tail: Vec<&str> = lines
                    .iter()
                    .skip(line_count - MAX_OUTPUT_CONTEXT)
                    .cloned()
                    .collect();
                let skipped = line_count - head_lines - MAX_OUTPUT_CONTEXT;
                format!(
                    "{}\n\n[... {} lines truncated ...]\n\n{}",
                    head.join("\n"),
                    skipped,
                    tail.join("\n")
                )
            } else {
                result
            };

            // Then apply byte limit as safety net
            let mut result = result;
            if result.len() > MAX_OUTPUT_BYTES {
                result.truncate(MAX_OUTPUT_BYTES);
                result.push_str("\n[Output truncated at 64KB]");
            }

            (description.to_string(), result)
        }
        Err(e) => {
            let error = format!("Failed to execute command: {}", e);
            (error.clone(), error)
        }
    }
}

#[derive(Debug)]
pub enum ValidationResult {
    Safe,
    HardBlocked(String),
    SoftBlocked(String),
}

pub fn validate_bash_command(command: &str) -> ValidationResult {
    let cmd_lower = command.to_lowercase();

    // Hard blocklist - always reject these
    let hard_blocks = [
        ("dd", "disk write operations (data destruction risk)"),
        ("mkfs", "filesystem formatting (irreversible)"),
        ("reboot", "system reboot (would interrupt session)"),
        ("shutdown", "system shutdown (would interrupt session)"),
    ];

    for (pattern, reason) in &hard_blocks {
        if is_command_match(&cmd_lower, pattern) {
            return ValidationResult::HardBlocked(reason.to_string());
        }
    }

    // Soft blocklist - warn and require confirmation
    let soft_blocks = [
        ("rm ", "file deletion (rm) - data loss risk"),
        ("rm\t", "file deletion (rm) - data loss risk"),
        ("mv ", "file move/rename - could overwrite data"),
        ("truncate", "file truncation (destructive)"),
        ("git push --force", "force git push (overwrites history)"),
        ("git push -f", "force git push (overwrites history)"),
        (" | bash", "pipe to bash (code injection risk)"),
        (" | sh", "pipe to shell (code injection risk)"),
    ];

    for (pattern, reason) in &soft_blocks {
        if is_command_match(&cmd_lower, pattern) {
            return ValidationResult::SoftBlocked(reason.to_string());
        }
    }

    ValidationResult::Safe
}

fn is_command_match(command: &str, pattern: &str) -> bool {
    // Check if pattern appears as a command (beginning of string or after operators)
    if command.starts_with(pattern) {
        return true;
    }

    // Also check after common command separators
    for sep in &[" && ", " ; ", " | ", "\n", "\t"] {
        if command.contains(&format!("{}{}", sep, pattern)) {
            return true;
        }
    }

    false
}

fn execute_write(args: &serde_json::Map<String, Value>) -> (String, String) {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            let error = "Error: 'path' parameter is required".to_string();
            return (error.clone(), error);
        }
    };

    let content = match args.get("content").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => {
            let error = "Error: 'content' parameter is required".to_string();
            return (error.clone(), error);
        }
    };

    // Create parent directories if needed
    let file_path = Path::new(path);
    if let Some(parent) = file_path.parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = fs::create_dir_all(parent)
    {
        let error = format!("Error creating directories: {}", e);
        return (error.clone(), error);
    }

    // Get old content if file exists (for diff)
    let old_content = fs::read_to_string(path).unwrap_or_default();

    // Write file
    match fs::write(path, content) {
        Ok(_) => {
            let filename = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path);
            let summary = format!("Writing {}", filename);

            // Generate diff
            let diff = generate_diff(&old_content, content, filename);
            let result = format!("Written {} bytes to {}\n\n{}", content.len(), path, diff);
            (summary, result)
        }
        Err(e) => {
            let error = format!("Error writing file: {}", e);
            (error.clone(), error)
        }
    }
}

fn execute_edit(args: &serde_json::Map<String, Value>) -> (String, String) {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            let error = "Error: 'path' parameter is required".to_string();
            return (error.clone(), error);
        }
    };

    let old_string = match args.get("old_string").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            let error = "Error: 'old_string' parameter is required".to_string();
            return (error.clone(), error);
        }
    };

    let new_string = match args.get("new_string").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            let error = "Error: 'new_string' parameter is required".to_string();
            return (error.clone(), error);
        }
    };

    // Read file
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            let error = format!("Error reading file: {}", e);
            return (error.clone(), error);
        }
    };

    // Find and replace
    if !content.contains(old_string) {
        let error = "Error: old_string not found in file".to_string();
        return (error.clone(), error);
    }

    let count = content.matches(old_string).count();
    if count > 1 {
        let error = format!("Error: old_string appears {} times (must be unique)", count);
        return (error.clone(), error);
    }

    let new_content = content.replacen(old_string, new_string, 1);

    // Write back
    match fs::write(path, &new_content) {
        Ok(_) => {
            let filename = Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path);
            let summary = format!("Editing {}", filename);

            // Generate diff
            let diff = generate_diff(&content, &new_content, filename);
            let result = format!("Successfully edited {}\n\n{}", path, diff);
            (summary, result)
        }
        Err(e) => {
            let error = format!("Error writing file: {}", e);
            (error.clone(), error)
        }
    }
}

fn execute_webfetch(args: &serde_json::Map<String, Value>) -> (String, String) {
    const MAX_CONTENT: usize = 32768; // 32KB

    let url = match args.get("url").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => {
            let error = "Error: 'url' parameter is required".to_string();
            return (error.clone(), error);
        }
    };

    // Extract domain for summary
    let domain = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(url);

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let error = format!("Failed to create HTTP client: {}", e);
            return (error.clone(), error);
        }
    };

    match client.get(url).send() {
        Ok(response) => match response.text() {
            Ok(content) => {
                // Strip HTML tags
                let text = strip_html_tags(&content);

                // Truncate if needed
                let result = if text.len() > MAX_CONTENT {
                    format!("{}\n[Content truncated at 32KB]", &text[..MAX_CONTENT])
                } else {
                    text
                };

                let summary = format!("Fetching {}", domain);
                (summary, result)
            }
            Err(e) => {
                let error = format!("Error reading response: {}", e);
                (error.clone(), error)
            }
        },
        Err(e) => {
            let error = format!("HTTP request failed: {}", e);
            (error.clone(), error)
        }
    }
}

fn strip_html_tags(html: &str) -> String {
    // Simple HTML tag stripping - remove anything between < and >
    let mut result = String::new();
    let mut in_tag = false;

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }

    // Clean up excessive whitespace
    let lines: Vec<&str> = result.lines().filter(|l| !l.trim().is_empty()).collect();
    lines.join("\n")
}

fn generate_diff(old: &str, new: &str, filename: &str) -> String {
    // New file case
    if old.is_empty() && !new.is_empty() {
        let mut result = format!(
            "```diff\n--- /dev/null\n+++ {}\n@@ -0,0 +1,{} @@\n",
            filename,
            new.lines().count()
        );
        for line in new.lines() {
            result.push_str(&format!("+{}\n", line));
        }
        result.push_str("```");
        return result;
    }

    let diff = TextDiff::from_lines(old, new);
    let unified = diff
        .unified_diff()
        .context_radius(5)
        .header(&format!("--- {}", filename), &format!("+++ {}", filename))
        .to_string();

    if unified.trim().is_empty() {
        return "(no changes)".to_string();
    }

    // Limit to 3000 lines to avoid overwhelming output
    let lines: Vec<&str> = unified.lines().collect();
    let capped = if lines.len() > 3000 {
        format!(
            "{}\n\n... (diff truncated, too many changes)",
            lines[..3000].join("\n")
        )
    } else {
        unified
    };

    format!("```diff\n{}\n```", capped)
}
