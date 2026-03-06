use pact::tools::{ToolCall, execute_tool, get_tool_definitions};
use serde_json::Value;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_get_tool_definitions() {
    let defs = get_tool_definitions();
    assert_eq!(defs.len(), 11);

    let tool = &defs[0];
    assert_eq!(tool.get("type").and_then(|v| v.as_str()), Some("function"));

    let func = tool.get("function").unwrap();
    assert_eq!(func.get("name").and_then(|v| v.as_str()), Some("Read"));
    assert!(func.get("description").is_some());
    assert!(func.get("parameters").is_some());
}

#[test]
fn test_read_tool_definition() {
    let defs = get_tool_definitions();
    let tool = defs
        .iter()
        .find(|t| {
            t.get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .map(|n| n == "Read")
                .unwrap_or(false)
        })
        .expect("Read tool not found");

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
        .expect("Failed to write");
    let path = temp_file.path().to_string_lossy().to_string();

    let mut args = serde_json::Map::new();
    args.insert("filePath".to_string(), Value::String(path));

    let tool_call = ToolCall {
        name: "Read".to_string(),
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
        Value::String("./nonexistent_test_file.txt".to_string()),
    );

    let tool_call = ToolCall {
        name: "Read".to_string(),
        args,
    };

    let (error, _) = execute_tool(&tool_call);
    assert!(error.contains("Error reading file"));
}

#[test]
fn test_execute_read_tool_missing_filepath() {
    let args = serde_json::Map::new();

    let tool_call = ToolCall {
        name: "Read".to_string(),
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
        name: "Read".to_string(),
        args,
    };

    let (error, _) = execute_tool(&tool_call);
    assert!(error.contains("Error reading file"));
}

#[test]
fn test_execute_read_tool_large_file() {
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let large_content = "x".repeat(70000);
    temp_file
        .write_all(large_content.as_bytes())
        .expect("Failed to write");
    let path = temp_file.path().to_string_lossy().to_string();

    let mut args = serde_json::Map::new();
    args.insert("filePath".to_string(), Value::String(path));

    let tool_call = ToolCall {
        name: "Read".to_string(),
        args,
    };

    let (summary, content_result) = execute_tool(&tool_call);
    assert!(summary.contains("Reading"));
    assert_eq!(content_result, large_content);
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
        name: "Read".to_string(),
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
        name: "Read".to_string(),
        args,
    };

    let cloned = tool.clone();
    assert_eq!(cloned.name, tool.name);
    assert_eq!(
        cloned.args.get("test").and_then(|v| v.as_str()),
        Some("value")
    );
}

#[test]
fn test_execute_glob_tool_no_matches() {
    let mut args = serde_json::Map::new();
    args.insert(
        "pattern".to_string(),
        Value::String("*.nonexistent".to_string()),
    );

    let tool_call = ToolCall {
        name: "Glob".to_string(),
        args,
    };

    let (summary, result) = execute_tool(&tool_call);

    assert!(summary.contains("No files"));
    assert_eq!(result, summary);
}

#[test]
fn test_execute_glob_tool_missing_pattern() {
    let args = serde_json::Map::new();

    let tool_call = ToolCall {
        name: "Glob".to_string(),
        args,
    };

    let (error, result) = execute_tool(&tool_call);

    assert!(error.contains("Error"));
    assert!(error.contains("pattern"));
    assert!(result.is_empty());
}

#[test]
fn test_execute_bash_tool_echo() {
    let mut args = serde_json::Map::new();
    args.insert(
        "command".to_string(),
        Value::String("echo hello".to_string()),
    );
    args.insert(
        "description".to_string(),
        Value::String("Echo test".to_string()),
    );

    let tool_call = ToolCall {
        name: "Bash".to_string(),
        args,
    };

    let (summary, result) = execute_tool(&tool_call);

    assert_eq!(summary, "Echo test");
    assert!(result.contains("hello"));
}

#[test]
fn test_execute_bash_tool_missing_command() {
    let args = serde_json::Map::new();

    let tool_call = ToolCall {
        name: "Bash".to_string(),
        args,
    };

    let (error, result) = execute_tool(&tool_call);

    assert!(error.contains("Error"));
    assert!(error.contains("command"));
    assert_eq!(error, result);
}

#[test]
fn test_execute_bash_tool_blocked_command() {
    let mut args = serde_json::Map::new();
    args.insert("command".to_string(), Value::String("rm -rf /".to_string()));
    args.insert(
        "description".to_string(),
        Value::String("Dangerous".to_string()),
    );

    let tool_call = ToolCall {
        name: "Bash".to_string(),
        args,
    };

    let (summary, _result) = execute_tool(&tool_call);

    assert!(summary.contains("blocked") || summary.contains("WARNING") || summary.contains('⚠'));
}

#[test]
fn test_execute_write_tool_missing_path() {
    let mut args = serde_json::Map::new();
    args.insert("content".to_string(), Value::String("test".to_string()));

    let tool_call = ToolCall {
        name: "Write".to_string(),
        args,
    };

    let (error, result) = execute_tool(&tool_call);

    assert!(error.contains("Error"));
    assert!(error.contains("path"));
    assert_eq!(error, result);
}

#[test]
fn test_execute_edit_tool_old_string_not_found() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test_edit.txt");
    std::fs::write(&file_path, "Hello, World!").expect("Failed to write file");

    let mut args = serde_json::Map::new();
    args.insert(
        "path".to_string(),
        Value::String(file_path.to_string_lossy().to_string()),
    );
    args.insert(
        "old_string".to_string(),
        Value::String("NonExistent".to_string()),
    );
    args.insert("new_string".to_string(), Value::String("Rust".to_string()));

    let tool_call = ToolCall {
        name: "Edit".to_string(),
        args,
    };

    let (error, result) = execute_tool(&tool_call);

    assert!(error.contains("not found"));
    assert_eq!(error, result);
}

#[test]
fn test_execute_edit_tool_duplicate_old_string() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test_edit.txt");
    std::fs::write(&file_path, "Hello World World!").expect("Failed to write file");

    let mut args = serde_json::Map::new();
    args.insert(
        "path".to_string(),
        Value::String(file_path.to_string_lossy().to_string()),
    );
    args.insert("old_string".to_string(), Value::String("World".to_string()));
    args.insert("new_string".to_string(), Value::String("Rust".to_string()));

    let tool_call = ToolCall {
        name: "Edit".to_string(),
        args,
    };

    let (error, result) = execute_tool(&tool_call);

    assert!(error.contains("appears"));
    assert!(error.contains("times"));
    assert_eq!(error, result);
}

#[test]
fn test_execute_webfetch_tool_missing_url() {
    let args = serde_json::Map::new();

    let tool_call = ToolCall {
        name: "Webfetch".to_string(),
        args,
    };

    let (error, result) = execute_tool(&tool_call);

    assert!(error.contains("Error"));
    assert!(error.contains("url"));
    assert_eq!(error, result);
}

#[test]
fn test_execute_read_tool_with_offset() {
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let content: String = (1..=10).map(|i| format!("Line {}\n", i)).collect();
    temp_file
        .write_all(content.as_bytes())
        .expect("Failed to write");
    let path = temp_file.path().to_string_lossy().to_string();

    let mut args = serde_json::Map::new();
    args.insert("filePath".to_string(), Value::String(path));
    args.insert("offset".to_string(), Value::Number(5.into()));

    let tool_call = ToolCall {
        name: "Read".to_string(),
        args,
    };

    let (summary, result) = execute_tool(&tool_call);

    assert!(summary.contains("Reading"));
    assert!(result.contains("Line 5") || result.contains("Line 6"));
}
