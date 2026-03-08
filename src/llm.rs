use crate::tools;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub arguments: String, // JSON string
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub text: String,
    #[serde(default)]
    pub is_tool_result: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_result_content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallInfo>>,
}

pub enum LlmEvent {
    Token(String, u64),    // (text, call_id)
    Thinking(String, u64), // (text, call_id)
    Done(u64),             // call_id
    Error(String, u64),    // (msg, call_id)
    Usage {
        input_tokens: usize,
        output_tokens: usize,
        call_id: u64,
    },
    ToolCall {
        id: String,
        name: String,
        args: serde_json::Map<String, serde_json::Value>,
        call_id: u64,
    },
    InvalidToolCall {
        raw: String,
        call_id: u64,
    },
    ApiLog {
        request_body: String,
        response_body: Option<String>,
        full_response: Option<String>,
        duration_ms: u64,
        error_message: Option<String>,
        model_name: Option<String>,
        provider: Option<String>,
        call_id: u64,
    },
    ServerInfo {
        model_name: String,
        context_window: usize,
        call_id: u64,
    },
    ToolResult {
        tool_name: String,
        tool_call_id: String,
        summary: String,
        content: String,
        call_id: u64,
    },
    ModelsLoaded {
        models: Vec<String>,
    },
}

/// Parse Qwen XML tool call format: <tool_call><function=NAME><parameter=KEY>VALUE</parameter></function></tool_call>
fn parse_qwen_xml_tool_call(
    raw: &str,
) -> Option<(String, String, serde_json::Map<String, serde_json::Value>)> {
    // Extract function name from <function=NAME>
    let func_start = raw.find("<function=")?;
    let func_end = raw[func_start + 10..].find('>')?;
    let name = raw[func_start + 10..func_start + 10 + func_end].to_string();

    let mut args = serde_json::Map::new();
    let mut remaining = raw;

    // Extract all <parameter=KEY>VALUE</parameter> patterns
    while let Some(param_start) = remaining.find("<parameter=") {
        let after_param = &remaining[param_start + 11..];
        let key_end = after_param.find('>')?;
        let key = after_param[..key_end].to_string();

        let value_start = param_start + 11 + key_end + 1;
        let value_end = remaining[value_start..].find("</parameter>")?;
        let value = remaining[value_start..value_start + value_end].to_string();

        // Clean embedded newlines and trim whitespace from extracted value
        let cleaned_value = value.replace("\n", "").replace("\r", "").trim().to_string();

        // Try to parse value as JSON, otherwise use as string
        let json_value = serde_json::from_str(&cleaned_value)
            .unwrap_or(serde_json::Value::String(cleaned_value));
        args.insert(key, json_value);

        remaining = &remaining[value_start + value_end + 12..];
    }

    // Use a static ID for Qwen XML tool calls
    let id = "qwen_xml_tool_call".to_string();

    if args.is_empty() {
        None
    } else {
        Some((id, name, args))
    }
}
/// Clean tool call arguments by removing embedded newlines and trimming string values
fn clean_tool_args(
    args: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut cleaned = serde_json::Map::new();
    for (key, value) in args {
        let cleaned_value = match value {
            serde_json::Value::String(s) => {
                // Remove embedded newlines and trim whitespace
                let trimmed = s.replace("\\n", "").trim().to_string();
                serde_json::Value::String(trimmed)
            }
            other => other,
        };
        cleaned.insert(key, cleaned_value);
    }
    cleaned
}

#[allow(clippy::too_many_arguments)]
pub fn call_llm(
    messages: Vec<Message>,
    tx: mpsc::Sender<LlmEvent>,
    debug: bool,
    endpoint: &str,
    api_key: Option<&str>,
    max_tokens: usize,
    mode_config: Option<crate::config::Mode>,
    system_prompt: Option<String>,
    model_id: String,
    provider_name: Option<String>,
    cancel_flag: Arc<AtomicBool>,
    call_id: u64,
) {
    let start_time = Instant::now();

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Failed to create client: {}", e);
            let _ = tx.send(LlmEvent::Error(err_msg.clone(), call_id));
            if debug {
                let _ = tx.send(LlmEvent::ApiLog {
                    request_body: String::new(),
                    response_body: None,
                    full_response: None,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    error_message: Some(err_msg),
                    model_name: Some(model_id.clone()),
                    provider: provider_name.clone(),
                    call_id,
                });
            }
            return;
        }
    };

    let mut msg_payload: Vec<_> = Vec::new();

    // Add system prompt as proper system message if present
    if let Some(prompt) = system_prompt {
        msg_payload.push(json!({
            "role": "system",
            "content": [{"type": "text", "text": prompt}]
        }));
    }

    // Add conversation messages
    for m in messages {
        // Skip empty non-tool messages (no text AND no tool_calls) - they confuse the model
        if !m.is_tool_result && m.text.trim().is_empty() && m.tool_calls.is_none() {
            continue;
        }

        let msg = if m.is_tool_result {
            // Tool results: use proper OpenAI format with tool_call_id
            let tool_output = m.tool_result_content.as_deref().unwrap_or(&m.text);
            let mut tool_msg = json!({
                "role": "tool",
                "content": tool_output
            });
            if let Some(tool_call_id) = &m.tool_call_id {
                tool_msg["tool_call_id"] = json!(tool_call_id);
            }
            tool_msg
        } else if let Some(tool_calls) = &m.tool_calls {
            // Assistant message with tool_calls
            let tc_json: Vec<_> = tool_calls
                .iter()
                .map(|tc| {
                    json!({
                        "id": tc.id,
                        "type": "function",
                        "function": {
                            "name": tc.name,
                            "arguments": tc.arguments
                        }
                    })
                })
                .collect();
            let mut obj = json!({
                "role": "assistant",
                "tool_calls": tc_json
            });
            if m.text.trim().is_empty() {
                obj["content"] = serde_json::Value::Null;
            } else {
                obj["content"] = json!([{"type": "text", "text": m.text}]);
            }
            obj
        } else {
            // Regular messages: array format content
            json!({
                "role": m.role,
                "content": [{"type": "text", "text": m.text}]
            })
        };
        msg_payload.push(msg);
    }

    let mut body = json!({
        "model": model_id,
        "max_tokens": max_tokens,
        "stream": true,
        "messages": msg_payload,
        "tools": tools::get_tool_definitions(),
        "tool_choice": "auto",
        "stream_options": {
            "include_usage": true
        }
    });

    // Apply OpenAI-compatible parameters from mode
    if let Some(mode) = &mode_config {
        if let Some(temp) = mode.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(top_p) = mode.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(presence_penalty) = mode.presence_penalty {
            body["presence_penalty"] = json!(presence_penalty);
        }
    }

    // Check if local backend to conditionally apply extensions
    let is_local = endpoint.contains("localhost") || endpoint.contains("127.0.0.1");
    if is_local
        && let Some(mode) = &mode_config
        && !mode.local_extensions.is_empty()
    {
        for (key, value) in &mode.local_extensions {
            body[key] = value.clone();
        }
    }

    let request_body = serde_json::to_string_pretty(&body).unwrap_or_default();

    // Trim trailing /v1 from endpoint to avoid /v1/v1/chat/completions
    // But keep /inference as it's part of Fireworks base URL
    let base_endpoint = endpoint.trim_end_matches("/v1");
    let mut request = client
        .post(format!("{}/v1/chat/completions", base_endpoint))
        .json(&body);

    // Add Authorization header if API key is provided
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    let response = match request.send() {
        Ok(r) => r,
        Err(e) => {
            let err_msg = format!("Request failed: {}", e);
            let _ = tx.send(LlmEvent::Error(err_msg.clone(), call_id));
            if debug {
                let _ = tx.send(LlmEvent::ApiLog {
                    request_body,
                    response_body: None,
                    full_response: None,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    error_message: Some(err_msg),
                    model_name: Some(model_id.clone()),
                    provider: provider_name.clone(),
                    call_id,
                });
            }
            return;
        }
    };

    // Check for error status codes
    let status = response.status();
    if !status.is_success() {
        let err_body = response.text().unwrap_or_default();
        let err_msg = format!(
            "API error {}: {}",
            status,
            &err_body[..err_body.len().min(200)]
        );
        let _ = tx.send(LlmEvent::Error(err_msg.clone(), call_id));
        if debug {
            let _ = tx.send(LlmEvent::ApiLog {
                request_body,
                response_body: Some(err_body),
                full_response: None,
                duration_ms: start_time.elapsed().as_millis() as u64,
                error_message: Some(err_msg),
                model_name: Some(model_id.clone()),
                provider: provider_name.clone(),
                call_id,
            });
        }
        return;
    }

    let mut response_blocks: Vec<String> = Vec::new();

    // Reconstruct complete response by accumulating deltas
    let mut accumulated_text = String::new();
    let mut accumulated_thinking = String::new();
    let mut accumulated_tool_calls: Vec<serde_json::Value> = Vec::new();
    let mut usage_data: Option<serde_json::Value> = None;

    // For accumulating streaming tool call arguments (which come as JSON fragments)
    struct PartialToolCall {
        id: String,
        name: String,
        arguments: String,
    }
    let mut partial_tool_call: Option<PartialToolCall> = None;
    let mut partial_xml: Option<String> = None;

    let reader = BufReader::new(response);

    for result in reader.lines() {
        // Check if cancelled - exit early
        if cancel_flag.load(Ordering::SeqCst) {
            // Send Done to signal clean exit
            let _ = tx.send(LlmEvent::Done(call_id));
            return;
        }

        let Ok(line) = result else { continue };

        if line == "data: [DONE]" {
            break;
        }

        if let Some(data_str) = line.strip_prefix("data: ")
            && let Ok(json_val) = serde_json::from_str::<serde_json::Value>(data_str)
        {
            response_blocks.push(data_str.to_string());
            // Extract delta from OpenAI format: {"choices":[{"delta":{...}}]}
            let delta_obj = json_val
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("delta"));

            // Check for text tokens (content field for OpenAI format)
            if let Some(delta) = delta_obj
                .and_then(|d| d.get("content"))
                .and_then(|t| t.as_str())
            {
                // Handle XML tool call accumulation across deltas
                if delta.contains("<tool_call>") {
                    partial_xml = Some(partial_xml.unwrap_or_default() + delta);
                } else if let Some(ref mut xml) = partial_xml {
                    xml.push_str(delta);
                }

                // Check if XML block is complete
                if let Some(xml) = &partial_xml {
                    if xml.contains("</tool_call>") {
                        let xml_block = partial_xml.take().unwrap();

                        // Try JSON format first (standard tool calls with embedded JSON)
                        let mut tool_call_found = false;
                        if let Some(tool_json_start) = xml_block.find('{')
                            && let Some(tool_json_end) = xml_block.rfind('}')
                        {
                            let tool_json_str = &xml_block[tool_json_start..=tool_json_end];
                            if let Ok(tool_json) =
                                serde_json::from_str::<serde_json::Value>(tool_json_str)
                            {
                                accumulated_tool_calls.push(tool_json.clone());
                                let id = tool_json
                                    .get("id")
                                    .and_then(|i| i.as_str())
                                    .unwrap_or("text_tool_call")
                                    .to_string();
                                let mut name = tool_json
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let mut args = serde_json::Map::new();

                                // Handle arguments - normalize parameter names to filePath
                                if let Some(arguments) =
                                    tool_json.get("arguments").and_then(|a| a.as_object())
                                {
                                    for (key, value) in arguments {
                                        if key == "path" || key == "file" || key == "filePath" {
                                            args.insert("filePath".to_string(), value.clone());
                                        } else {
                                            args.insert(key.clone(), value.clone());
                                        }
                                    }
                                }

                                // If name is empty but we have filePath arg, infer it's a "read" tool
                                if name.is_empty() && args.contains_key("filePath") {
                                    name = "read".to_string();
                                }

                                if !name.is_empty() && !args.is_empty() {
                                    let _ = tx.send(LlmEvent::ToolCall {
                                        id,
                                        name,
                                        args,
                                        call_id,
                                    });
                                    tool_call_found = true;
                                }
                            }
                        }

                        // If JSON parsing failed, try Qwen XML format
                        if !tool_call_found
                            && let Some((id, name, args)) = parse_qwen_xml_tool_call(&xml_block)
                        {
                            let _ = tx.send(LlmEvent::ToolCall {
                                id,
                                name,
                                args,
                                call_id,
                            });
                            tool_call_found = true;
                        }

                        // If both parsers failed, emit InvalidToolCall event and the raw text as a token
                        if !tool_call_found {
                            let _ = tx.send(LlmEvent::InvalidToolCall {
                                raw: xml_block.clone(),
                                call_id,
                            });
                            accumulated_text.push_str(&xml_block);
                            let _ = tx.send(LlmEvent::Token(xml_block, call_id));
                        }
                    }
                } else if !delta.contains("<tool_call>") && !delta.contains("</tool_call>") {
                    // Regular text token (not part of XML block)
                    accumulated_text.push_str(delta);
                    let _ = tx.send(LlmEvent::Token(delta.to_string(), call_id));
                }
            }

            // Check for reasoning/thinking tokens
            if let Some(thinking) = delta_obj
                .and_then(|d| d.get("reasoning_content"))
                .and_then(|t| t.as_str())
            {
                // Handle XML tool call accumulation in thinking (like Qwen3+)
                if thinking.contains("<tool_call>") {
                    partial_xml = Some(partial_xml.unwrap_or_default() + thinking);
                } else if let Some(ref mut xml) = partial_xml {
                    xml.push_str(thinking);
                }

                // Check if XML block is complete
                if let Some(xml) = &partial_xml {
                    if xml.contains("</tool_call>") {
                        let xml_block = partial_xml.take().unwrap();

                        // Try to parse as Qwen XML format
                        if let Some((id, name, args)) = parse_qwen_xml_tool_call(&xml_block) {
                            let _ = tx.send(LlmEvent::ToolCall {
                                id,
                                name,
                                args,
                                call_id,
                            });
                        } else {
                            // If parsing failed, still emit as thinking content
                            let _ = tx.send(LlmEvent::Thinking(xml_block.clone(), call_id));
                            accumulated_thinking.push_str(&xml_block);
                        }
                    }
                } else if !thinking.contains("<tool_call>") && !thinking.contains("</tool_call>") {
                    // Regular thinking token (not part of XML block)
                    accumulated_thinking.push_str(thinking);
                    let _ = tx.send(LlmEvent::Thinking(thinking.to_string(), call_id));
                }
            }

            // Check for tool calls in delta
            if let Some(tool_calls) = delta_obj
                .and_then(|d| d.get("tool_calls"))
                .and_then(|tc| tc.as_array())
            {
                for tool_call in tool_calls {
                    accumulated_tool_calls.push(tool_call.clone());

                    // Extract tool call id and function components
                    // First, if this delta has an id, start a new partial tool call
                    if let Some(id) = tool_call.get("id").and_then(|i| i.as_str())
                        && let Some(function) = tool_call.get("function")
                    {
                        // Get the function name if present
                        if let Some(name) = function.get("name").and_then(|n| n.as_str()) {
                            // Start a new partial tool call if we don't have one
                            if partial_tool_call.is_none() {
                                partial_tool_call = Some(PartialToolCall {
                                    id: id.to_string(),
                                    name: name.to_string(),
                                    arguments: String::new(),
                                });
                            }
                        }
                    }

                    // Accumulate arguments from this delta (regardless of whether it has an id)
                    // This handles fragments that are sent without id but are part of the tool call
                    if let Some(function) = tool_call.get("function")
                        && let Some(args_fragment) =
                            function.get("arguments").and_then(|a| a.as_str())
                        && let Some(ref mut partial) = partial_tool_call
                    {
                        partial.arguments.push_str(args_fragment);

                        // Only try parsing if it might be complete (ends with })
                        if partial.arguments.ends_with('}') {
                            // Try to parse accumulated arguments as JSON
                            if let Ok(args_json) =
                                serde_json::from_str::<serde_json::Value>(&partial.arguments)
                                && let Some(args_obj) = args_json.as_object()
                            {
                                // Successfully parsed! Clean up string values by removing embedded newlines
                                let cleaned_args = clean_tool_args(args_obj.clone());

                                // Emit the tool call
                                let _ = tx.send(LlmEvent::ToolCall {
                                    id: partial.id.clone(),
                                    name: partial.name.clone(),
                                    args: cleaned_args,
                                    call_id,
                                });
                                // Clear the partial to avoid re-emitting
                                partial_tool_call = None;
                            }
                        }
                    }
                }
            }

            // Check for usage (OpenAI format: prompt_tokens, completion_tokens)
            if let Some(usage) = json_val.get("usage") {
                usage_data = Some(usage.clone());
                let input_tokens = usage
                    .get("prompt_tokens")
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0) as usize;
                let output_tokens = usage
                    .get("completion_tokens")
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0) as usize;

                if input_tokens > 0 || output_tokens > 0 {
                    let _ = tx.send(LlmEvent::Usage {
                        input_tokens,
                        output_tokens,
                        call_id,
                    });
                }
            }
        }
    }

    // Handle any remaining incomplete partial tool call
    // If we have a partial tool call but arguments don't parse as JSON,
    // try to extract what we can and emit it
    if let Some(partial) = partial_tool_call {
        // Try to repair/complete the arguments JSON
        let mut fixed_args = partial.arguments.clone();

        // If it doesn't end with }, try to add one
        if !fixed_args.trim().ends_with('}') {
            fixed_args.push('}');
        }

        // Try parsing the fixed arguments
        let args_to_send = if let Ok(args_json) =
            serde_json::from_str::<serde_json::Value>(&fixed_args)
            && let Some(args_obj) = args_json.as_object()
        {
            // Successfully parsed!
            args_obj.clone()
        } else {
            // Parsing failed - try cleaning embedded newlines and retry
            // Remove literal newlines from string values in JSON
            let cleaned = fixed_args.replace("\\n", "");
            if let Ok(args_json) = serde_json::from_str::<serde_json::Value>(&cleaned)
                && let Some(args_obj) = args_json.as_object()
            {
                // Successfully parsed after cleanup!
                args_obj.clone()
            } else {
                // Still can't parse - create a fallback with raw arguments as a string
                let mut fallback = serde_json::Map::new();
                fallback.insert(
                    "_raw_arguments".to_string(),
                    serde_json::Value::String(fixed_args),
                );
                fallback
            }
        };

        // Emit the tool call with whatever we could extract
        let _ = tx.send(LlmEvent::ToolCall {
            id: partial.id,
            name: partial.name,
            args: args_to_send,
            call_id,
        });
    }

    // Reconstruct complete response from accumulated deltas
    let full_response = if !accumulated_thinking.is_empty()
        || !accumulated_text.is_empty()
        || !accumulated_tool_calls.is_empty()
        || usage_data.is_some()
    {
        let mut response_parts = Vec::new();

        if !accumulated_thinking.is_empty() {
            response_parts.push(format!("[THINKING]\n{}\n", accumulated_thinking));
        }

        if !accumulated_text.is_empty() {
            response_parts.push(format!("[TEXT]\n{}\n", accumulated_text));
        }

        if !accumulated_tool_calls.is_empty() {
            response_parts.push("[TOOL_CALLS]".to_string());
            for tool_call in accumulated_tool_calls {
                if let Ok(pretty) = serde_json::to_string_pretty(&tool_call) {
                    response_parts.push(pretty);
                }
            }
            response_parts.push(String::new());
        }

        if let Some(usage) = usage_data {
            response_parts.push(format!("[USAGE]\n{}\n", usage));
        }

        Some(response_parts.join("\n"))
    } else {
        None
    };

    if debug {
        let response_body = if response_blocks.is_empty() {
            None
        } else {
            Some(response_blocks.join("\n"))
        };
        let _ = tx.send(LlmEvent::ApiLog {
            request_body,
            response_body,
            full_response,
            duration_ms: start_time.elapsed().as_millis() as u64,
            error_message: None,
            model_name: Some(model_id),
            provider: provider_name,
            call_id,
        });
    }

    let _ = tx.send(LlmEvent::Done(call_id));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_qwen_xml_tool_call_with_multiline() {
        let xml = r#"<tool_call>
<function=Bash>
<parameter=command>find . -type f \( -name "*.rs" -o -name "*.js" \) 2>/dev/null | head -50</parameter>
<parameter=description>Find Rust and JavaScript source files</parameter>
</function>
</tool_call>"#;

        let result = parse_qwen_xml_tool_call(xml);
        assert!(result.is_some(), "Should parse Qwen XML tool call");

        let (id, name, args) = result.unwrap();
        assert_eq!(name, "Bash");
        assert_eq!(id, "qwen_xml_tool_call");
        assert_eq!(
            args.get("command").and_then(|v| v.as_str()).unwrap(),
            r#"find . -type f \( -name "*.rs" -o -name "*.js" \) 2>/dev/null | head -50"#
        );
        assert_eq!(
            args.get("description").and_then(|v| v.as_str()).unwrap(),
            "Find Rust and JavaScript source files"
        );
    }

    #[test]
    fn test_parse_qwen_xml_tool_call_single_line() {
        let xml = r#"<tool_call><function=Read><parameter=filePath>/etc/passwd</parameter></function></tool_call>"#;

        let result = parse_qwen_xml_tool_call(xml);
        assert!(result.is_some());

        let (_id, name, args) = result.unwrap();
        assert_eq!(name, "Read");
        assert_eq!(
            args.get("filePath").and_then(|v| v.as_str()).unwrap(),
            "/etc/passwd"
        );
    }

    #[test]
    fn test_parse_qwen_xml_tool_call_json_value() {
        let xml = r#"<tool_call><function=Write><parameter=filePath>/test.txt</parameter><parameter=options>{"indent":2}</parameter></function></tool_call>"#;

        let result = parse_qwen_xml_tool_call(xml);
        assert!(result.is_some());

        let (_, name, args) = result.unwrap();
        assert_eq!(name, "Write");

        // Should parse JSON value as object, not string
        let options = args.get("options").unwrap();
        assert!(options.is_object());
        assert_eq!(options.get("indent").and_then(|v| v.as_u64()).unwrap(), 2);
    }

    #[test]
    fn test_tool_call_arguments_repair_incomplete() {
        // When arguments are incomplete/malformed, store as raw
        // Real case: streamed fragments that don't form valid JSON
        let incomplete_args = r#"{"pattern":"**/*.rs"#;

        // This can't be fixed by just adding } - string is unclosed
        let result = serde_json::from_str::<serde_json::Value>(incomplete_args);
        assert!(result.is_err(), "Incomplete JSON should fail to parse");

        // In real code, this would be stored as _raw_arguments
        // For testing, just verify it doesn't parse
    }

    #[test]
    fn test_tool_call_arguments_with_brace_expansion() {
        // Real pattern from logs: **/*.{rs,html,js}
        // This was fragmented as: ** / * . { rs , html , js } }
        let args_json = r#"{"pattern":"**/*.{rs,html,js}"}"#;

        let result = serde_json::from_str::<serde_json::Value>(args_json);
        assert!(result.is_ok());

        let json = result.unwrap();
        assert_eq!(
            json.get("pattern").and_then(|v| v.as_str()).unwrap(),
            "**/*.{rs,html,js}"
        );
    }

    #[test]
    fn test_tool_call_read_file_path_repair() {
        // Real fragmented Read call from logs: ./src/ui/messages.rs
        // Came as fragments: { / filePath / : / " / ./ / src / /ui / /messages / .rs / " / }
        let partial_args = r#"{"filePath":"./src/ui/messages.rs""#;

        let mut fixed = partial_args.to_string();
        if !fixed.trim().ends_with('}') {
            fixed.push('}');
        }

        let result = serde_json::from_str::<serde_json::Value>(&fixed);
        assert!(result.is_ok());

        let json = result.unwrap();
        assert_eq!(
            json.get("filePath").and_then(|v| v.as_str()).unwrap(),
            "./src/ui/messages.rs"
        );
    }

    #[test]
    fn test_tool_call_fallback_with_raw_arguments() {
        // Case where repair still fails - store as raw string
        let unparseable = r#"invalid json {]"#;

        // Try to parse, fallback to raw storage
        let mut map = serde_json::Map::new();
        match serde_json::from_str::<serde_json::Value>(unparseable) {
            Ok(v) if v.is_object() => {
                map = v.as_object().unwrap().clone();
            }
            _ => {
                map.insert(
                    "_raw_arguments".to_string(),
                    serde_json::Value::String(unparseable.to_string()),
                );
            }
        }

        // Should have stored the raw arguments
        assert!(map.contains_key("_raw_arguments"));
        assert_eq!(
            map.get("_raw_arguments").and_then(|v| v.as_str()).unwrap(),
            "invalid json {]"
        );
    }

    #[test]
    fn test_clean_tool_args_removes_embedded_newlines() {
        // Real case from logs: Grep with newlines in files and pattern
        let mut args = serde_json::Map::new();
        args.insert(
            "files".to_string(),
            serde_json::Value::String("\nsrc/app.rs\n".to_string()),
        );
        args.insert(
            "pattern".to_string(),
            serde_json::Value::String("\nKey::Up|Key::Down\n".to_string()),
        );

        let cleaned = clean_tool_args(args);

        assert_eq!(
            cleaned.get("files").and_then(|v| v.as_str()).unwrap(),
            "src/app.rs"
        );
        assert_eq!(
            cleaned.get("pattern").and_then(|v| v.as_str()).unwrap(),
            "Key::Up|Key::Down"
        );
    }

    #[test]
    fn test_clean_tool_args_handles_escaped_newlines() {
        // Tool call with escaped \n sequences
        let mut args = serde_json::Map::new();
        args.insert(
            "filePath".to_string(),
            serde_json::Value::String("src/app.rs\\n".to_string()),
        );

        let cleaned = clean_tool_args(args);

        assert_eq!(
            cleaned.get("filePath").and_then(|v| v.as_str()).unwrap(),
            "src/app.rs"
        );
    }
}
