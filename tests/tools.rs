use pact::tools::{ToolCall, execute_tool, get_tool_definitions};
use serde_json::Value;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_get_tool_definitions() {
    let defs = get_tool_definitions();
    assert_eq!(defs.len(), 1);

    let tool = &defs[0];
    assert_eq!(tool.get("type").and_then(|v| v.as_str()), Some("function"));

    let func = tool.get("function").unwrap();
    assert_eq!(func.get("name").and_then(|v| v.as_str()), Some("read"));
    assert!(func.get("description").is_some());
    assert!(func.get("parameters").is_some());
}

#[test]
fn test_read_tool_definition() {
    let defs = get_tool_definitions();
    let tool = &defs[0];
    let func = tool.get("function").unwrap();

    let params = func.get("parameters").unwrap();
    assert_eq!(params.get("type").and_then(|v| v.as_str()), Some("object"));

    let required = params.get("required").unwrap().as_array().unwrap();
    assert!(required.iter().any(|v| v.as_str() == Some("filePath")));
}

#[test]
fn test_execute_read_tool_success() {
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let content = "Test file content";
    temp_file
        .write_all(content.as_bytes())
        .expect("Failed to write to temp file");
    let path = temp_file.path().to_string_lossy().to_string();

    let mut args = serde_json::Map::new();
    args.insert("filePath".to_string(), Value::String(path));

    let tool_call = ToolCall {
        name: "read".to_string(),
        args,
    };

    let (summary, content_result) = execute_tool(&tool_call);
    assert!(summary.contains("Reading"));
    assert_eq!(content_result, content);
}

#[test]
fn test_execute_read_tool_relative_path() {
    let mut args = serde_json::Map::new();
    args.insert(
        "filePath".to_string(),
        Value::String("relative/path".to_string()),
    );

    let tool_call = ToolCall {
        name: "read".to_string(),
        args,
    };

    let (error, _) = execute_tool(&tool_call);
    assert!(error.contains("Error") && error.contains("absolute"));
}

#[test]
fn test_execute_read_tool_missing_filepath() {
    let args = serde_json::Map::new();

    let tool_call = ToolCall {
        name: "read".to_string(),
        args,
    };

    let (error, _) = execute_tool(&tool_call);
    assert!(error.contains("Error") && error.contains("filePath"));
}

#[test]
fn test_execute_read_tool_nonexistent_file() {
    let mut args = serde_json::Map::new();
    args.insert(
        "filePath".to_string(),
        Value::String("/nonexistent/path/to/file.txt".to_string()),
    );

    let tool_call = ToolCall {
        name: "read".to_string(),
        args,
    };

    let (error, _) = execute_tool(&tool_call);
    assert!(error.contains("Error reading file"));
}

#[test]
fn test_execute_read_tool_large_file() {
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    // Create a file larger than MAX_FILE_SIZE (65536 bytes)
    let large_content = "x".repeat(70000);
    temp_file
        .write_all(large_content.as_bytes())
        .expect("Failed to write to temp file");
    let path = temp_file.path().to_string_lossy().to_string();

    let mut args = serde_json::Map::new();
    args.insert("filePath".to_string(), Value::String(path));

    let tool_call = ToolCall {
        name: "read".to_string(),
        args,
    };

    let (summary, content_result) = execute_tool(&tool_call);
    assert!(summary.contains("Reading"));
    assert!(content_result.contains("File too large"));
    assert!(content_result.contains("65536"));
    assert!(content_result.len() < large_content.len());
}

#[test]
fn test_execute_unknown_tool() {
    let args = serde_json::Map::new();

    let tool_call = ToolCall {
        name: "unknown_tool".to_string(),
        args,
    };

    let (error, _) = execute_tool(&tool_call);
    assert!(error.contains("Unknown tool"));
    assert!(error.contains("unknown_tool"));
}

#[test]
fn test_execute_read_tool_wrong_type_filepath() {
    let mut args = serde_json::Map::new();
    args.insert("filePath".to_string(), Value::Number(123.into()));

    let tool_call = ToolCall {
        name: "read".to_string(),
        args,
    };

    let (error, _) = execute_tool(&tool_call);
    assert!(error.contains("Error"));
}

#[test]
fn test_tool_call_clone() {
    let mut args = serde_json::Map::new();
    args.insert("test".to_string(), Value::String("value".to_string()));

    let tool = ToolCall {
        name: "read".to_string(),
        args,
    };

    let cloned = tool.clone();
    assert_eq!(cloned.name, tool.name);
    assert_eq!(
        cloned.args.get("test").and_then(|v| v.as_str()),
        Some("value")
    );
}
