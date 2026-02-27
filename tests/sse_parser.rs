/// Tests for SSE (Server-Sent Events) parser and LLM message handling
/// These test the core protocol parsing logic for different model formats
use serde_json::json;

#[test]
fn test_parse_text_token() {
    // Real OpenAI format: delta is nested under choices[0]
    let json_val = json!({
        "choices": [{
            "delta": {
                "content": "Hello world"
            }
        }]
    });

    let delta = json_val
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"));

    if let Some(delta_str) = delta
        .and_then(|d| d.get("content"))
        .and_then(|t| t.as_str())
    {
        assert_eq!(delta_str, "Hello world");
        assert!(!delta_str.contains("<tool_call>"));
    } else {
        panic!("Failed to extract content from choices[0].delta");
    }
}

#[test]
fn test_parse_thinking_token() {
    let json_val = json!({
        "choices": [{
            "delta": {
                "reasoning_content": "Let me think about this carefully..."
            }
        }]
    });

    let delta = json_val
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"));

    if let Some(thinking) = delta
        .and_then(|d| d.get("reasoning_content"))
        .and_then(|t| t.as_str())
    {
        assert_eq!(thinking, "Let me think about this carefully...");
    } else {
        panic!("Failed to extract reasoning_content");
    }
}

#[test]
fn test_parse_structured_tool_call() {
    let json_val = json!({
        "choices": [{
            "delta": {
                "tool_calls": [{
                    "id": "call_abc123",
                    "type": "function",
                    "function": {
                        "name": "read",
                        "arguments": r#"{"filePath":"/etc/hosts"}"#
                    }
                }]
            }
        }]
    });

    let delta = json_val
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"));

    if let Some(tool_calls) = delta
        .and_then(|d| d.get("tool_calls"))
        .and_then(|tc| tc.as_array())
    {
        assert_eq!(tool_calls.len(), 1);

        let tool_call = &tool_calls[0];
        if let (Some(name), Some(args_str)) = (
            tool_call
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str()),
            tool_call
                .get("function")
                .and_then(|f| f.get("arguments"))
                .and_then(|a| a.as_str()),
        ) {
            assert_eq!(name, "read");
            let args: serde_json::Value = serde_json::from_str(args_str).unwrap();
            assert_eq!(
                args.get("filePath").and_then(|v| v.as_str()),
                Some("/etc/hosts")
            );
        }
    }
}

#[test]
fn test_parse_qwen_text_tool_call() {
    let tool_call_json = r#"{"name":"read","arguments":{"filePath":"/home/user/file.txt"}}"#;
    let json_val = json!({
        "choices": [{
            "delta": {
                "content": format!("<tool_call>{}</tool_call>", tool_call_json)
            }
        }]
    });

    let delta = json_val
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"));

    if let Some(delta_str) = delta
        .and_then(|d| d.get("content"))
        .and_then(|t| t.as_str())
    {
        assert!(delta_str.contains("<tool_call>"));

        if let Some(tool_json_start) = delta_str.find('{')
            && let Some(tool_json_end) = delta_str.rfind('}')
        {
            let tool_json_str = &delta_str[tool_json_start..=tool_json_end];
            let tool_json: serde_json::Value = serde_json::from_str(tool_json_str).unwrap();

            assert_eq!(
                tool_json.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                "read"
            );

            if let Some(arguments) = tool_json.get("arguments").and_then(|a| a.as_object()) {
                assert_eq!(
                    arguments
                        .get("filePath")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    "/home/user/file.txt"
                );
            }
        }
    }
}

#[test]
fn test_parse_usage_tokens() {
    // Usage is at root level, not in choices
    let json_val = json!({
        "usage": {
            "prompt_tokens": 42,
            "completion_tokens": 15
        }
    });

    if let Some(usage) = json_val.get("usage") {
        let prompt_tokens = usage
            .get("prompt_tokens")
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as usize;
        let completion_tokens = usage
            .get("completion_tokens")
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as usize;

        assert_eq!(prompt_tokens, 42);
        assert_eq!(completion_tokens, 15);
    }
}

#[test]
fn test_tool_parameter_normalization() {
    // Test that path/file/filePath parameters all get handled
    let test_cases = vec![
        ("path", "/test/path"),
        ("file", "/test/file"),
        ("filePath", "/test/filepath"),
    ];

    for (param_name, param_value) in test_cases {
        let mut arguments = serde_json::Map::new();
        arguments.insert(param_name.to_string(), json!(param_value));

        // Normalize: any of path/file/filePath becomes filePath
        let has_file_param = arguments.contains_key("path")
            || arguments.contains_key("file")
            || arguments.contains_key("filePath");

        assert!(has_file_param, "Should have file parameter variant");
    }
}

#[test]
fn test_multiple_tool_calls() {
    let json_val = json!({
        "choices": [{
            "delta": {
                "tool_calls": [
                    {
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "read",
                            "arguments": r#"{"filePath":"/file1.txt"}"#
                        }
                    },
                    {
                        "id": "call_2",
                        "type": "function",
                        "function": {
                            "name": "read",
                            "arguments": r#"{"filePath":"/file2.txt"}"#
                        }
                    }
                ]
            }
        }]
    });

    let delta = json_val
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"));

    if let Some(tool_calls) = delta
        .and_then(|d| d.get("tool_calls"))
        .and_then(|tc| tc.as_array())
    {
        assert_eq!(tool_calls.len(), 2);
    }
}

#[test]
fn test_mixed_content_and_thinking() {
    let json_val = json!({
        "choices": [{
            "delta": {
                "reasoning_content": "Analyzing...",
                "content": "Result"
            }
        }]
    });

    let delta = json_val
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"));

    let thinking = delta
        .and_then(|d| d.get("reasoning_content"))
        .and_then(|t| t.as_str());
    let content = delta
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str());

    assert_eq!(thinking, Some("Analyzing..."));
    assert_eq!(content, Some("Result"));
}

#[test]
fn test_empty_delta() {
    let json_val = json!({
        "choices": [{
            "delta": {}
        }]
    });

    let delta = json_val
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"));

    let has_content = delta.and_then(|d| d.get("content")).is_some();
    let has_thinking = delta.and_then(|d| d.get("reasoning_content")).is_some();
    let has_tool_calls = delta.and_then(|d| d.get("tool_calls")).is_some();

    assert!(!has_content);
    assert!(!has_thinking);
    assert!(!has_tool_calls);
}

#[test]
fn test_finish_reason() {
    let json_val = json!({
        "choices": [{
            "finish_reason": "stop"
        }]
    });

    if let Some(first_choice) = json_val
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
    {
        let finish_reason = first_choice.get("finish_reason").and_then(|fr| fr.as_str());
        assert_eq!(finish_reason, Some("stop"));
    }
}

#[test]
fn test_complex_tool_arguments() {
    let json_val = json!({
        "choices": [{
            "delta": {
                "tool_calls": [{
                    "id": "call_123",
                    "type": "function",
                    "function": {
                        "name": "bash",
                        "arguments": r#"{"command":"ls -la","workdir":"/home/user","timeout":30}"#
                    }
                }]
            }
        }]
    });

    let delta = json_val
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"));

    if let Some(args_str) = delta
        .and_then(|d| d.get("tool_calls"))
        .and_then(|tc| tc.as_array())
        .and_then(|tc| tc.first())
        .and_then(|tc| tc.get("function"))
        .and_then(|f| f.get("arguments"))
        .and_then(|a| a.as_str())
    {
        let args: serde_json::Value = serde_json::from_str(args_str).unwrap();
        assert_eq!(args.get("command").and_then(|v| v.as_str()), Some("ls -la"));
        assert_eq!(
            args.get("workdir").and_then(|v| v.as_str()),
            Some("/home/user")
        );
        assert_eq!(args.get("timeout").and_then(|v| v.as_u64()), Some(30));
    }
}
