use crate::tools;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{BufRead, BufReader};
use std::sync::mpsc;
use std::time::{Duration, Instant};

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
}

pub enum LlmEvent {
    Token(String),
    Thinking(String),
    Progress(f32),
    Done,
    Error(String),
    Usage {
        input_tokens: usize,
        output_tokens: usize,
    },
    ToolCall {
        name: String,
        args: serde_json::Map<String, serde_json::Value>,
    },
    ApiLog {
        request_body: String,
        response_body: Option<String>,
        full_response: Option<String>,
        duration_ms: u64,
        error_message: Option<String>,
    },
}

pub fn call_llm(
    messages: Vec<Message>,
    tx: mpsc::Sender<LlmEvent>,
    debug: bool,
    endpoint: &str,
    max_tokens: usize,
    temperature: Option<f32>,
    system_prompt: Option<String>,
) {
    let start_time = Instant::now();

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Failed to create client: {}", e);
            let _ = tx.send(LlmEvent::Error(err_msg.clone()));
            if debug {
                let _ = tx.send(LlmEvent::ApiLog {
                    request_body: String::new(),
                    response_body: None,
                    full_response: None,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    error_message: Some(err_msg),
                });
            }
            return;
        }
    };

    let mut msg_payload: Vec<_> = Vec::new();

    // Add system prompt as proper system message if present
    if let Some(prompt) = system_prompt {
        msg_payload.push(json!({ "role": "system", "content": prompt }));
    }

    // Add conversation messages
    for m in messages {
        // For tool results, send the actual content instead of the summary
        let content = if m.is_tool_result {
            m.tool_result_content.as_deref().unwrap_or(&m.text)
        } else {
            &m.text
        };
        msg_payload.push(json!({ "role": m.role, "content": content }));
    }

    let mut body = json!({
        "model": "local",
        "max_tokens": max_tokens,
        "stream": true,
        "messages": msg_payload,
        "tools": tools::get_tool_definitions(),
        "tool_choice": "auto",
        "stream_options": {
            "include_usage": true
        }
    });

    if let Some(temp) = temperature {
        body["temperature"] = json!(temp);
    }

    let request_body = serde_json::to_string_pretty(&body).unwrap_or_default();

    let response = match client
        .post(format!("{}/v1/chat/completions", endpoint))
        .json(&body)
        .send()
    {
        Ok(r) => r,
        Err(e) => {
            let err_msg = format!("Request failed: {}", e);
            let _ = tx.send(LlmEvent::Error(err_msg.clone()));
            if debug {
                let _ = tx.send(LlmEvent::ApiLog {
                    request_body,
                    response_body: None,
                    full_response: None,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    error_message: Some(err_msg),
                });
            }
            return;
        }
    };

    let mut response_blocks: Vec<String> = Vec::new();

    // Reconstruct complete response by accumulating deltas
    let mut accumulated_text = String::new();
    let mut accumulated_thinking = String::new();
    let mut accumulated_tool_calls: Vec<serde_json::Value> = Vec::new();
    let mut usage_data: Option<serde_json::Value> = None;

    // For accumulating streaming tool call arguments (which come as JSON fragments)
    struct PartialToolCall {
        name: String,
        arguments: String,
    }
    let mut partial_tool_call: Option<PartialToolCall> = None;

    let reader = BufReader::new(response);

    for result in reader.lines() {
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
                // Check if this contains a tool call (for Qwen and text-format models)
                if delta.contains("<tool_call>") {
                    // Parse tool call from text format
                    if let Some(tool_json_start) = delta.find('{')
                        && let Some(tool_json_end) = delta.rfind('}')
                    {
                        let tool_json_str = &delta[tool_json_start..=tool_json_end];
                        if let Ok(tool_json) =
                            serde_json::from_str::<serde_json::Value>(tool_json_str)
                        {
                            accumulated_tool_calls.push(tool_json.clone());
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
                                        // Normalize file parameter to filePath
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
                                let _ = tx.send(LlmEvent::ToolCall { name, args });
                            }
                        }
                    }
                    // Don't send the <tool_call> block as a token
                } else {
                    // Regular text token
                    accumulated_text.push_str(delta);
                    let _ = tx.send(LlmEvent::Token(delta.to_string()));
                }
            }

            // Check for reasoning/thinking tokens
            if let Some(thinking) = delta_obj
                .and_then(|d| d.get("reasoning_content"))
                .and_then(|t| t.as_str())
            {
                accumulated_thinking.push_str(thinking);
                let _ = tx.send(LlmEvent::Thinking(thinking.to_string()));
            }

            // Check for tool calls in delta
            if let Some(tool_calls) = delta_obj
                .and_then(|d| d.get("tool_calls"))
                .and_then(|tc| tc.as_array())
            {
                for tool_call in tool_calls {
                    accumulated_tool_calls.push(tool_call.clone());

                    // Extract tool call components
                    if let Some(function) = tool_call.get("function") {
                        // Get the function name if present
                        if let Some(name) = function.get("name").and_then(|n| n.as_str()) {
                            // Start a new partial tool call if we don't have one
                            if partial_tool_call.is_none() {
                                partial_tool_call = Some(PartialToolCall {
                                    name: name.to_string(),
                                    arguments: String::new(),
                                });
                            }
                        }

                        // Accumulate arguments fragment if present
                        if let Some(args_fragment) =
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
                                    // Successfully parsed! Emit the tool call
                                    let _ = tx.send(LlmEvent::ToolCall {
                                        name: partial.name.clone(),
                                        args: args_obj.clone(),
                                    });
                                    // Clear the partial to avoid re-emitting
                                    partial_tool_call = None;
                                }
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
                    });
                }
            }

            // Check for progress (llama.cpp extension)
            if let Some(progress) = json_val.get("progress").and_then(|p| p.as_f64()) {
                let _ = tx.send(LlmEvent::Progress(progress as f32));
            }
        }
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
        });
    }

    let _ = tx.send(LlmEvent::Done);
}
