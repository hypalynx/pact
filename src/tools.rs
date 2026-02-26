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
            "description": "Read file contents from disk. Use this to examine code, configuration, documentation, and understand the context you're working with.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute file path to read"
                    }
                },
                "required": ["path"]
            }
        }
    })]
}

pub fn execute_tool(tool_call: &ToolCall) -> String {
    match tool_call.name.as_str() {
        "read" => execute_read(&tool_call.args),
        _ => format!("Unknown tool: {}", tool_call.name),
    }
}

fn execute_read(args: &serde_json::Map<String, Value>) -> String {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return "Error: 'path' parameter is required and must be a string".to_string(),
    };

    // Validate path is absolute
    if !Path::new(path).is_absolute() {
        return format!("Error: path must be absolute, got: {}", path);
    }

    // Try to read the file
    match fs::read_to_string(path) {
        Ok(contents) => {
            // Check size
            if contents.len() > MAX_FILE_SIZE {
                format!(
                    "File too large ({} bytes, max {}). Showing first {} bytes:\n\n{}",
                    contents.len(),
                    MAX_FILE_SIZE,
                    MAX_FILE_SIZE,
                    &contents[..MAX_FILE_SIZE]
                )
            } else {
                contents
            }
        }
        Err(e) => format!("Error reading file '{}': {}", path, e),
    }
}
