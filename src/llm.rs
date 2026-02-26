use crate::tools;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub text: String,
    #[serde(default)]
    pub is_tool_result: bool,
}

pub enum LlmEvent {
    Token(String),
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
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(LlmEvent::Error(format!("Failed to create client: {}", e)));
            return;
        }
    };

    let mut msg_payload: Vec<_> = Vec::new();

    // Add tools availability note as first user message with explicit format instruction
    let tools_note = "You have access to a 'read' tool to examine files. When you need to read a file, output ONLY this JSON (no other text):\n\n{\"tool_calls\": [{\"index\": 0, \"id\": \"read\", \"type\": \"function\", \"function\": {\"name\": \"read\", \"arguments\": \"{\\\"path\\\": \\\"/absolute/path/to/file\\\"}\"}}}}\n\nOnce you receive the file contents, respond normally with your analysis or findings. For file paths, always use absolute paths.";
    msg_payload.push(json!({ "role": "user", "content": tools_note }));

    // Prepend mode prompt as second user message if present
    if let Some(prompt) = system_prompt {
        msg_payload.push(json!({ "role": "user", "content": prompt }));
    }

    // Add conversation messages
    for m in messages {
        msg_payload.push(json!({ "role": m.role, "content": m.text }));
    }

    let mut body = json!({
        "model": "local",
        "max_tokens": max_tokens,
        "stream": true,
        "messages": msg_payload,
        "tools": tools::get_tool_definitions(),
    });

    if let Some(temp) = temperature {
        body["temperature"] = json!(temp);
    }

    if debug {
        let _ = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("api.log")
            .and_then(|mut file| {
                writeln!(file, "=== REQUEST {} ===", chrono::Local::now())?;
                writeln!(file, "POST {}/v1/messages", endpoint)?;
                writeln!(
                    file,
                    "{}",
                    serde_json::to_string_pretty(&body).unwrap_or_default()
                )?;
                writeln!(file, "\n=== RESPONSE ===")?;
                Ok(())
            });
    }

    let response = match client
        .post(&format!("{}/v1/messages", endpoint))
        .json(&body)
        .send()
    {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(LlmEvent::Error(format!("Request failed: {}", e)));
            return;
        }
    };

    let mut log_file = if debug {
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("api.log")
            .ok()
    } else {
        None
    };

    let reader = BufReader::new(response);

    for line in reader.lines() {
        if let Ok(line) = line {
            if debug {
                if let Some(ref mut f) = log_file {
                    let _ = writeln!(f, "{}", line);
                }
            }

            if line == "data: [DONE]" {
                break;
            }

            if let Some(data_str) = line.strip_prefix("data: ") {
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(data_str) {
                    // Log all delta events for debugging
                    if debug {
                        if let Some(delta) = json_val.get("delta") {
                            if let Some(ref mut f) = log_file {
                                let _ = writeln!(
                                    f,
                                    "DELTA: {}",
                                    serde_json::to_string_pretty(&delta).unwrap_or_default()
                                );
                            }
                        }
                    }

                    // Check for text tokens
                    if let Some(delta) = json_val
                        .get("delta")
                        .and_then(|d| d.get("text"))
                        .and_then(|t| t.as_str())
                    {
                        // Check if this contains a tool call
                        if delta.contains("<tool_call>") {
                            // Parse tool call from text format
                            if let Some(tool_json_start) = delta.find('{') {
                                if let Some(tool_json_end) = delta.rfind('}') {
                                    let tool_json_str = &delta[tool_json_start..=tool_json_end];
                                    if let Ok(tool_json) =
                                        serde_json::from_str::<serde_json::Value>(tool_json_str)
                                    {
                                        let mut name = tool_json
                                            .get("name")
                                            .and_then(|n| n.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let mut args = serde_json::Map::new();

                                        // Handle both "file" and "path" parameter names
                                        if let Some(file_arg) =
                                            tool_json.get("arguments").and_then(|a| a.get("file"))
                                        {
                                            args.insert("path".to_string(), file_arg.clone());
                                        }
                                        if let Some(path_arg) =
                                            tool_json.get("arguments").and_then(|a| a.get("path"))
                                        {
                                            args.insert("path".to_string(), path_arg.clone());
                                        }

                                        // If name is empty but we have path arg, infer it's a "read" tool
                                        if name.is_empty() && args.contains_key("path") {
                                            name = "read".to_string();
                                        }

                                        if !name.is_empty() && !args.is_empty() {
                                            let _ = tx.send(LlmEvent::ToolCall { name, args });
                                        }
                                    }
                                }
                            }
                            // Don't send the <tool_call> block as a token
                        } else {
                            // Regular text token
                            let _ = tx.send(LlmEvent::Token(delta.to_string()));
                        }
                    }

                    // Check for tool calls in delta
                    if let Some(tool_calls) = json_val
                        .get("delta")
                        .and_then(|d| d.get("tool_calls"))
                        .and_then(|tc| tc.as_array())
                    {
                        if debug {
                            if let Some(ref mut f) = log_file {
                                let _ = writeln!(
                                    f,
                                    "TOOL_CALLS: {}",
                                    serde_json::to_string_pretty(&tool_calls).unwrap_or_default()
                                );
                            }
                        }
                        for tool_call in tool_calls {
                            if let Some(function) = tool_call.get("function") {
                                if let (Some(name), Some(args_str)) = (
                                    function.get("name").and_then(|n| n.as_str()),
                                    function.get("arguments").and_then(|a| a.as_str()),
                                ) {
                                    // Parse arguments JSON
                                    if let Ok(args_json) =
                                        serde_json::from_str::<serde_json::Value>(args_str)
                                    {
                                        if let Some(args_obj) = args_json.as_object() {
                                            let _ = tx.send(LlmEvent::ToolCall {
                                                name: name.to_string(),
                                                args: args_obj.clone(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Check for usage
                    if let Some(usage) = json_val.get("usage") {
                        let input_tokens = usage
                            .get("input_tokens")
                            .and_then(|t| t.as_u64())
                            .unwrap_or(0) as usize;
                        let output_tokens = usage
                            .get("output_tokens")
                            .and_then(|t| t.as_u64())
                            .unwrap_or(0) as usize;

                        if input_tokens > 0 || output_tokens > 0 {
                            let _ = tx.send(LlmEvent::Usage {
                                input_tokens,
                                output_tokens,
                            });
                        }
                    }
                }
            }
        }
    }

    if debug {
        if let Some(mut f) = log_file {
            let _ = writeln!(f, "===\n");
        }
    }

    let _ = tx.send(LlmEvent::Done);
}
