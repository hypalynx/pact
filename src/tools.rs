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
    vec![json!({
        "type": "function",
        "function": {
            "name": "read",
            "description": "Read the complete contents of a file at an absolute path. Returns the raw text content. Files larger than 64KB are truncated at the limit with a warning. Use this to examine source code, configs, README files, and any other text content.",
            "parameters": {
                "type": "object",
                "properties": {
                    "filePath": {
                        "type": "string",
                        "description": "Absolute path to the file to read (must start with /)"
                    }
                },
                "required": ["filePath"],
                "additionalProperties": false
            }
        }
    })]
}

pub fn execute_tool(tool_call: &ToolCall) -> (String, String) {
    match tool_call.name.as_str() {
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

    // Validate path is absolute
    if !Path::new(path).is_absolute() {
        let error = format!("Error: path must be absolute, got: {}", path);
        return (error.clone(), error);
    }

    // Extract just the filename from the path
    let filename = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);

    // Try to read the file
    match fs::read_to_string(path) {
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
